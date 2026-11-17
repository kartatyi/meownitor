// The session list the card shows. Two sources, merged by Claude Code's session id:
// - what each session is doing, written by our hook (bin/hook.rs) into sessions/<id>.json;
// - Claude Desktop's own record of each session (title, its local id for the claude:// link,
//   archived or not), one JSON file per session under claude-code-sessions.
// A background thread re-reads both and emits "sessions" to the window whenever the list changes.
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// A session counts as open while it was active within this window; Desktop never forgets a
/// session that was not archived, so "open" has to mean "recent".
const OPEN_FOR_MS: u64 = 24 * 3600 * 1000;
/// A finished turn nobody came back to drops into the idle group after this long.
const DONE_TO_IDLE_MS: u64 = 30 * 60 * 1000;
/// A working session that has been silent this long is assumed gone. A waiting one is not: it
/// sends nothing while it waits for you, so it stays until it is answered, stopped or a day old.
const STALE_MS: u64 = 30 * 60 * 1000;
/// A session's state file is kept this long after its last event: the list shows a day, the rest
/// is margin. Old files and old answered rounds (rounds.rs `prune`) go once in `CLEAN_EVERY`.
const KEEP_STATE: Duration = Duration::from_secs(7 * 24 * 3600);
const CLEAN_EVERY: Duration = Duration::from_secs(3600);

#[derive(Serialize, Clone, PartialEq, Default)]
pub struct Session {
    sid: String,
    local: Option<String>,
    title: String,
    state: String,
    what: String,
    detail: String,
    since: u64,
    cwd: String,
    /// A question round waiting for an answer (rounds.rs), by name.
    round: Option<String>,
    /// The beginning of Claude's last reply, while the session's turn is over («Your turn»).
    said: String,
    /// What a permission request asks to allow, in full, while the session waits on it.
    more: String,
    /// The request can be answered on the card: its hook is waiting for the click (perms.rs).
    perm: bool,
}

pub struct Latest(pub Mutex<Vec<Session>>);

struct Hooked {
    cwd: String,
    transcript: String,
    event: String,
    state: String,
    what: String,
    detail: String,
    said: String,
    more: String,
    since: u64,
    updated: u64,
}

struct Desk {
    local: String,
    title: String,
    archived: bool,
    last: u64,
    cwd: String,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The user's home folder: %USERPROFILE% on Windows (a shell's $HOME there may be anywhere), $HOME elsewhere.
pub fn home() -> Option<PathBuf> {
    #[cfg(windows)]
    return std::env::var_os("USERPROFILE").map(PathBuf::from);
    #[cfg(not(windows))]
    return std::env::var_os("HOME").map(PathBuf::from);
}

/// ~/.meownitor on every system — on Windows not under AppData, which Claude Desktop's package
/// redirects (msix.rs) — so the hook and the round skill find it by one path.
pub fn data_dir() -> Option<PathBuf> {
    home().map(|h| h.join(".meownitor"))
}

/// Where Claude Desktop keeps its records: %APPDATA%\Claude — or the MSIX package's copy of it —
/// on Windows, ~/Library/Application Support/Claude on macOS, and on Linux, which has no Desktop
/// of its own, where the community builds of it keep theirs: $XDG_CONFIG_HOME/Claude.
fn desktop_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = home().map(|h| h.join("Library").join("Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|h| h.join(".config")));
    let plain = base.map(|b| b.join("Claude").join("claude-code-sessions"));
    let packaged = crate::msix::claude_packages().into_iter().map(|p| {
        p.join("Roaming")
            .join("Claude")
            .join("claude-code-sessions")
    });
    packaged.chain(plain.clone()).find(|p| p.is_dir()).or(plain)
}

fn str_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Files changed since the last pass, re-read; files gone, forgotten.
struct Cache<T> {
    seen: HashMap<PathBuf, (SystemTime, Option<(String, T)>)>,
}

impl<T> Cache<T> {
    fn new() -> Self {
        Cache {
            seen: HashMap::new(),
        }
    }
    fn refresh(&mut self, files: Vec<PathBuf>, parse: impl Fn(&Path) -> Option<(String, T)>) {
        let mut next = HashMap::new();
        for f in files {
            let Some(m) = mtime(&f) else { continue };
            match self.seen.remove(&f) {
                Some((old, v)) if old == m => {
                    next.insert(f, (old, v));
                }
                _ => {
                    let v = parse(&f);
                    next.insert(f, (m, v));
                }
            }
        }
        self.seen = next;
    }
    fn values(&self) -> impl Iterator<Item = &(String, T)> {
        self.seen.values().filter_map(|(_, v)| v.as_ref())
    }
}

fn list_json(dir: &Path, depth: u32, prefix: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth > 0 {
                list_json(&p, depth - 1, prefix, out);
            }
        } else if p.extension().is_some_and(|x| x == "json")
            && p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix))
        {
            out.push(p);
        }
    }
}

fn parse_hooked(p: &Path) -> Option<(String, Hooked)> {
    let v = read_json(p)?;
    let num = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    Some((
        str_of(&v, "sid"),
        Hooked {
            cwd: str_of(&v, "cwd"),
            transcript: str_of(&v, "transcript"),
            event: str_of(&v, "event"),
            state: str_of(&v, "state"),
            what: str_of(&v, "what"),
            detail: str_of(&v, "detail"),
            said: str_of(&v, "said"),
            more: str_of(&v, "more"),
            since: num("since"),
            updated: num("updated"),
        },
    ))
}

fn parse_desk(p: &Path) -> Option<(String, Desk)> {
    let v = read_json(p)?;
    let cli = str_of(&v, "cliSessionId");
    if cli.is_empty() {
        return None;
    }
    Some((
        cli,
        Desk {
            local: str_of(&v, "sessionId"),
            title: str_of(&v, "title"),
            archived: v
                .get("isArchived")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            last: v
                .get("lastActivityAt")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            cwd: str_of(&v, "cwd"),
        },
    ))
}

/// Where Claude Code keeps a session's transcript. The hook passes the path on; state files written
/// before it did fall back to Claude Code's layout: ~/.claude/projects/<cwd with everything but
/// letters and digits turned into '-'>/<id>.jsonl — right only if the session never changed folder.
fn transcript_of(sid: &str, h: &Hooked) -> Option<PathBuf> {
    if !h.transcript.is_empty() {
        return Some(PathBuf::from(&h.transcript));
    }
    let project: String = h
        .cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Some(
        home()?
            .join(".claude")
            .join("projects")
            .join(project)
            .join(format!("{sid}.jsonl")),
    )
}

/// Stopping a turn fires no hook: Claude Code only writes "[Request interrupted by user]" (or
/// "… for tool use]") into the transcript as the turn's last message. A session whose transcript
/// ends that way was stopped, whatever the hook said last.
fn ends_interrupted(path: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut tail = Vec::new();
    if f.seek(SeekFrom::Start(len.saturating_sub(64 * 1024)))
        .is_err()
        || f.read_to_end(&mut tail).is_err()
    {
        return false;
    }
    for line in String::from_utf8_lossy(&tail).lines().rev() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = str_of(&v, "type");
        if (kind != "user" && kind != "assistant")
            || v.get("isSidechain").and_then(|x| x.as_bool()) == Some(true)
        {
            continue;
        }
        let texts: Vec<&str> = match v.pointer("/message/content") {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect(),
            _ => Vec::new(),
        };
        return kind == "user"
            && texts
                .iter()
                .any(|t| t.starts_with("[Request interrupted by user"));
    }
    false
}

/// Removes the state files (and a hook's leftover .tmp) untouched for `KEEP_STATE` — except those
/// of the sessions in `keep`, which have a round still waiting: its row is named from the file.
/// Returns how many went.
fn prune(dir: &Path, keep: &[&str], now: SystemTime) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut gone = 0;
    for e in rd.flatten() {
        let p = e.path();
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
        if !e.file_type().is_ok_and(|t| t.is_file()) || (ext != "json" && ext != "tmp") {
            continue;
        }
        let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("");
        if ext == "json" && keep.contains(&stem) {
            continue;
        }
        let old = mtime(&p).is_some_and(|t| now.duration_since(t).is_ok_and(|a| a > KEEP_STATE));
        if old && std::fs::remove_file(&p).is_ok() {
            gone += 1;
        }
    }
    gone
}

fn folder_name(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(cwd)
        .to_string()
}

fn compose(
    hooks: &Cache<Hooked>,
    desk: &Cache<Desk>,
    stopped: &[String],
    rounds: &[crate::rounds::Pending],
    perms: &[crate::perms::Waiting],
    now: u64,
) -> Vec<Session> {
    let mut map: HashMap<String, Session> = HashMap::new();
    let mut archived: Vec<String> = Vec::new();
    for (cli, d) in desk.values() {
        if d.archived {
            archived.push(cli.clone());
            continue;
        }
        if now.saturating_sub(d.last) > OPEN_FOR_MS {
            continue;
        }
        map.insert(
            cli.clone(),
            Session {
                sid: cli.clone(),
                local: Some(d.local.clone()),
                title: if d.title.is_empty() {
                    folder_name(&d.cwd)
                } else {
                    d.title.clone()
                },
                state: "idle".into(),
                what: String::new(),
                detail: String::new(),
                since: d.last,
                cwd: d.cwd.clone(),
                round: None,
                ..Default::default()
            },
        );
    }
    for (sid, h) in hooks.values() {
        if archived.contains(sid) || now.saturating_sub(h.updated) > OPEN_FOR_MS {
            continue;
        }
        if h.state == "ended" {
            map.remove(sid);
            continue;
        }
        let mut state = h.state.clone();
        let over = match state.as_str() {
            "done" => now.saturating_sub(h.since) > DONE_TO_IDLE_MS,
            "run" => now.saturating_sub(h.updated) > STALE_MS || stopped.contains(sid),
            "wait" => stopped.contains(sid),
            _ => false,
        };
        if over {
            state = "idle".into();
        }
        let e = map.entry(sid.clone()).or_insert_with(|| Session {
            sid: sid.clone(),
            local: None,
            // Named by the widget in its own language: "<folder> · terminal".
            title: String::new(),
            state: String::new(),
            what: String::new(),
            detail: String::new(),
            since: 0,
            cwd: h.cwd.clone(),
            round: None,
            ..Default::default()
        });
        e.since = if state == "idle" {
            e.since.max(h.updated)
        } else {
            h.since
        };
        e.what = if state == "idle" {
            String::new()
        } else {
            h.what.clone()
        };
        e.detail = if state == "idle" {
            String::new()
        } else {
            h.detail.clone()
        };
        e.said = if state == "done" {
            h.said.clone()
        } else {
            String::new()
        };
        e.more = if state == "wait" {
            h.more.clone()
        } else {
            String::new()
        };
        e.state = state;
    }
    // A permission request whose hook waits for a click on the card: the session waits for you
    // whatever its other agents have reported since, and the row says what is asked.
    for p in perms {
        let e = map.entry(p.sid.clone()).or_insert_with(|| Session {
            sid: p.sid.clone(),
            ..Default::default()
        });
        e.state = "wait".into();
        e.what = "@perm".into();
        e.detail = p.detail.clone();
        e.more = p.more.clone();
        e.since = p.since;
        e.perm = true;
    }
    // A pending round outranks whatever the hook last said: the session is waiting for you — for as
    // long as the round is unanswered, days after the session itself left the list. Such a row is
    // still named by what Desktop or the hook knew of the session.
    for r in rounds {
        let e = map.entry(r.sid.clone()).or_insert_with(|| {
            let d = desk
                .values()
                .find(|(cli, d)| *cli == r.sid && !d.archived)
                .map(|(_, d)| d);
            let h = hooks
                .values()
                .find(|(sid, _)| *sid == r.sid)
                .map(|(_, h)| h);
            Session {
                sid: r.sid.clone(),
                local: d.map(|d| d.local.clone()),
                title: d
                    .map(|d| {
                        if d.title.is_empty() {
                            folder_name(&d.cwd)
                        } else {
                            d.title.clone()
                        }
                    })
                    .unwrap_or_default(),
                state: String::new(),
                what: String::new(),
                detail: String::new(),
                since: 0,
                cwd: d
                    .map(|d| d.cwd.clone())
                    .or_else(|| h.map(|h| h.cwd.clone()))
                    .unwrap_or_default(),
                round: None,
                ..Default::default()
            }
        });
        e.state = "wait".into();
        e.what = "@ask".into();
        e.detail = r.title.clone();
        e.since = r.since;
        e.round = Some(r.name.clone());
    }
    let mut list: Vec<Session> = map.into_values().collect();
    list.sort_by_key(|s| std::cmp::Reverse(s.since));
    list
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let (Some(data), desk_dir) = (data_dir(), desktop_dir()) else {
            return;
        };
        let hook_dir = data.join("sessions");
        let mut hooks: Cache<Hooked> = Cache::new();
        let mut desk: Cache<Desk> = Cache::new();
        // Whether each working or waiting session's transcript ends in a stop, by session id.
        let mut transcripts: Cache<bool> = Cache::new();
        let mut last_desk = Instant::now() - Duration::from_secs(60);
        let mut last_sent: Option<Vec<Session>> = None;
        let mut rounds = crate::rounds::Watch::new();
        let mut cleaned: Option<Instant> = None;
        loop {
            let mut files = Vec::new();
            list_json(&hook_dir, 0, "", &mut files);
            hooks.refresh(files, parse_hooked);
            let now = now_ms();
            let busy: Vec<PathBuf> = hooks
                .values()
                .filter(|(_, h)| {
                    let quiet = now.saturating_sub(h.updated);
                    (h.state == "run" && quiet <= STALE_MS)
                        || (h.state == "wait" && quiet <= OPEN_FOR_MS)
                })
                .filter_map(|(sid, h)| transcript_of(sid, h))
                .collect();
            transcripts.refresh(busy, |p| {
                Some((
                    p.file_stem()?.to_string_lossy().into_owned(),
                    ends_interrupted(p),
                ))
            });
            let stopped: Vec<String> = transcripts
                .values()
                .filter(|(_, hit)| *hit)
                .map(|(sid, _)| sid.clone())
                .collect();
            if last_desk.elapsed() >= Duration::from_secs(5) {
                if let Some(d) = &desk_dir {
                    let mut files = Vec::new();
                    list_json(d, 2, "local_", &mut files);
                    desk.refresh(files, parse_desk);
                }
                last_desk = Instant::now();
            }
            let pending = rounds.pending();
            // A permission request waiting for a click on the card is let go once its session's
            // turn is over or a new one began: there is nothing left to allow.
            let mut perms = crate::perms::waiting();
            perms.retain(|p| {
                let over = hooks.values().any(|(sid, h)| {
                    *sid == p.sid
                        && h.updated > p.since
                        && ["Stop", "StopFailure", "SessionEnd", "UserPromptSubmit"]
                            .contains(&h.event.as_str())
                });
                if over {
                    crate::perms::answer(&p.sid, "dialog");
                }
                !over
            });
            // Release builds only: a build under work deletes nothing from the data folder.
            if !cfg!(debug_assertions) && cleaned.is_none_or(|t| t.elapsed() >= CLEAN_EVERY) {
                let keep: Vec<&str> = pending.iter().map(|r| r.sid.as_str()).collect();
                let states = prune(&hook_dir, &keep, SystemTime::now());
                let folders = crate::rounds::prune();
                if states + folders > 0 {
                    crate::watchdog::log(&format!(
                        "cleaned up: {states} session files untouched for a week, {folders} round folders untouched for a month"
                    ));
                }
                cleaned = Some(Instant::now());
            }
            let list = compose(&hooks, &desk, &stopped, &pending, &perms, now);
            if last_sent.as_ref() != Some(&list) {
                if let Some(latest) = app.try_state::<Latest>() {
                    *latest.0.lock().unwrap() = list.clone();
                }
                let _ = app.emit("sessions", &list);
                last_sent = Some(list);
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    });
}

/// Opens a Desktop session by its local id through the claude:// link Desktop registers.
pub fn open_in_desktop(local: &str) {
    if !local.starts_with("local_")
        || !local[6..]
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        return;
    }
    let url = format!("claude://claude.ai/epitaxy/{local}");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &url])
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hooked(state: &str, quiet_ms: u64, now: u64) -> Hooked {
        Hooked {
            cwd: "C:/work/api-server".into(),
            transcript: String::new(),
            event: String::new(),
            state: state.into(),
            what: "@perm".into(),
            detail: String::new(),
            said: "All 13 tests pass.".into(),
            more: "cargo test".into(),
            since: now - quiet_ms,
            updated: now - quiet_ms,
        }
    }

    fn cache<T>(entries: Vec<(&str, T)>) -> Cache<T> {
        let mut c = Cache::new();
        for (sid, v) in entries {
            c.seen
                .insert(PathBuf::from(sid), (UNIX_EPOCH, Some((sid.to_string(), v))));
        }
        c
    }

    /// Silence ends a working session, not one that waits for you: that one stays until it is
    /// stopped or a day old, and a round stays for as long as it is unanswered.
    #[test]
    fn a_waiting_session_stays_waiting() {
        let now = 10 * OPEN_FOR_MS;
        let hour = 3600 * 1000;
        let hooks = cache(vec![
            ("waits", hooked("wait", 2 * hour, now)),
            ("works", hooked("run", 2 * hour, now)),
            ("stopped", hooked("wait", 2 * hour, now)),
            ("old", hooked("wait", 25 * hour, now)),
            ("asked", hooked("done", 72 * hour, now)),
        ]);
        let rounds = [crate::rounds::Pending {
            sid: "asked".into(),
            name: "pick".into(),
            title: "Pick one".into(),
            since: now - 72 * hour,
        }];
        let list = compose(
            &hooks,
            &Cache::new(),
            &["stopped".to_string()],
            &rounds,
            &[],
            now,
        );
        let state = |sid: &str| list.iter().find(|s| s.sid == sid).map(|s| s.state.as_str());
        assert_eq!(state("waits"), Some("wait"));
        assert_eq!(state("works"), Some("idle"));
        assert_eq!(state("stopped"), Some("idle"));
        assert_eq!(
            state("old"),
            None,
            "a day of silence drops it from the list"
        );
        let asked = list.iter().find(|s| s.sid == "asked").unwrap();
        assert_eq!(
            (
                asked.state.as_str(),
                asked.round.as_deref(),
                asked.cwd.as_str()
            ),
            ("wait", Some("pick"), "C:/work/api-server"),
            "a round asked three days ago is still there to answer, named by its folder"
        );
    }

    /// What was said shows only while the turn is over, what is asked only while the session
    /// waits; a request whose hook waits for the card makes the session wait whatever it said last.
    #[test]
    fn the_reply_shows_when_done_and_the_request_when_waiting() {
        let now = 10 * OPEN_FOR_MS;
        let hooks = cache(vec![
            ("done", hooked("done", 60_000, now)),
            ("waits", hooked("wait", 60_000, now)),
            ("busy", hooked("run", 60_000, now)),
        ]);
        let perms = [crate::perms::Waiting {
            sid: "busy".into(),
            tool: "Bash".into(),
            detail: "Bash · Deploy".into(),
            more: "./deploy.sh production".into(),
            since: now - 5_000,
        }];
        let list = compose(&hooks, &Cache::new(), &[], &[], &perms, now);
        let of = |sid: &str| list.iter().find(|s| s.sid == sid).unwrap();
        assert_eq!(
            (of("done").said.as_str(), of("done").more.as_str()),
            ("All 13 tests pass.", "")
        );
        assert_eq!(
            (
                of("waits").said.as_str(),
                of("waits").more.as_str(),
                of("waits").perm
            ),
            ("", "cargo test", false)
        );
        let busy = of("busy");
        assert_eq!(
            (
                busy.state.as_str(),
                busy.what.as_str(),
                busy.more.as_str(),
                busy.perm
            ),
            ("wait", "@perm", "./deploy.sh production", true)
        );
    }

    /// A week-old state file goes, a fresh one stays, and so does an old one whose session still
    /// has a round waiting; nothing but the hook's own files is touched.
    #[test]
    fn old_state_files_go_except_those_with_a_round_waiting() {
        let dir = std::env::temp_dir().join(format!("cw-states-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["gone.json", "asked.json", "gone.123.tmp", "notes.txt"] {
            std::fs::write(dir.join(f), "{}").unwrap();
        }
        let now = SystemTime::now();
        assert_eq!(
            prune(&dir, &["asked"], now),
            0,
            "nothing here is a week old"
        );
        let later = now + KEEP_STATE + Duration::from_secs(60);
        assert_eq!(prune(&dir, &["asked"], later), 2);
        let left = |f: &str| dir.join(f).exists();
        assert!(!left("gone.json") && !left("gone.123.tmp"));
        assert!(left("asked.json") && left("notes.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stop is seen only while it is the transcript's last message — plain or mid-tool; a new
    /// prompt after it, or a reply, means the session went on.
    #[test]
    fn a_stop_shows_only_as_the_last_message() {
        let p =
            std::env::temp_dir().join(format!("cw-transcript-test-{}.jsonl", std::process::id()));
        let user = |t: &str| {
            format!(
                r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"{t}"}}]}}}}"#
            )
        };
        let lines = |ls: &[String]| std::fs::write(&p, ls.join("\n") + "\n").unwrap();
        let attachment = r#"{"type":"attachment","attachment":{"type":"date"}}"#.to_string();
        let title = r#"{"type":"custom-title","customTitle":"x"}"#.to_string();
        let reply = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}"#.to_string();

        lines(&[
            user("hi"),
            attachment.clone(),
            user("[Request interrupted by user]"),
            title.clone(),
        ]);
        assert!(ends_interrupted(&p));
        lines(&[
            user("[Request interrupted by user for tool use]"),
            title.clone(),
        ]);
        assert!(ends_interrupted(&p));
        lines(&[
            user("[Request interrupted by user]"),
            user("go on"),
            attachment.clone(),
        ]);
        assert!(!ends_interrupted(&p));
        lines(&[user("hi"), reply]);
        assert!(!ends_interrupted(&p));
        let _ = std::fs::remove_file(&p);
        assert!(!ends_interrupted(&p));
    }
}
