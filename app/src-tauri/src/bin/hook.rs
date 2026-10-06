// Claude Code hook relay: records what a session is doing into a small per-session JSON file that
// the widget reads (sessions/<session_id>.json under the widget's data folder). It must never slow
// Claude down or fail it — on any problem it just exits 0 without output. Its only output is a
// question round's answer that no waiter took, passed on with the user's next message.
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The widget's data folder (sessions.rs `data_dir`): ~/.meownitor, on Windows where Claude
/// Desktop's package doesn't redirect our writes as it does under AppData.
fn state_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(|h| PathBuf::from(h).join(".meownitor").join("sessions"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// First line, trimmed to `n` characters.
fn short(s: &str, n: usize) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    let mut out: String = line.chars().take(n).collect();
    if line.chars().count() > n {
        out.push('…');
    }
    out
}

fn base_name(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// The tool as the widget names it, and the one detail worth a glance: the command's own
/// description, the file, the pattern, the host.
fn describe(tool: &str, input: &Value) -> (String, String) {
    let s = |k: &str| input.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let name = match tool.strip_prefix("mcp__") {
        Some(rest) => rest.rsplit("__").next().unwrap_or(rest).to_string(),
        None => tool.to_string(),
    };
    let detail = match tool {
        "Bash" | "PowerShell" => short(
            if s("description").is_empty() {
                s("command")
            } else {
                s("description")
            },
            80,
        ),
        "Read" | "Edit" | "Write" | "MultiEdit" => base_name(s("file_path")),
        "NotebookEdit" => base_name(s("notebook_path")),
        "Grep" | "Glob" => short(s("pattern"), 60),
        "WebFetch" => s("url").split('/').nth(2).unwrap_or("").to_string(),
        "WebSearch" => short(s("query"), 60),
        "Task" | "Agent" => short(s("description"), 60),
        "Skill" => short(s("skill"), 40),
        "AskUserQuestion" => input
            .pointer("/questions/0/question")
            .and_then(|v| v.as_str())
            .map(|q| short(q, 80))
            .unwrap_or_default(),
        _ => String::new(),
    };
    (name, detail)
}

/// One line per event in events.log in the data folder, so what Claude actually sends can be checked;
/// the log starts over once it passes 512 KB.
fn log_event(dir: &std::path::Path, now: u64, sid: &str, event: &str, tool: &str) {
    use std::io::Write;
    let Some(base) = dir.parent() else { return };
    let path = base.join("events.log");
    if std::fs::metadata(&path)
        .map(|m| m.len() > 512 * 1024)
        .unwrap_or(false)
    {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{now} {} {event} {tool}", &sid[..sid.len().min(8)]);
    }
}

/// An answer reaches the session once: whoever makes <name>.taken first hands it over — the waiter,
/// or the hook with the user's next message when no waiter was left to see it.
fn take(dir: &Path, name: &str) -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{name}.taken")))
        .is_ok()
}

/// Answers sent after the session stopped waiting — the waiter gave up, was stopped or went with a
/// restart — in the waiter's words, to go to the session with the user's next message. Only answers
/// newer than rounds/.since: before it the waiters took every answer and marked none.
fn late_answers(rounds: &Path, sid: &str) -> String {
    let stamp = rounds.join(".since");
    if !stamp.exists() {
        let _ = std::fs::create_dir_all(rounds);
        let _ = std::fs::write(&stamp, "");
    }
    let modified = |p: &Path| p.metadata().and_then(|m| m.modified()).ok();
    let Some(since) = modified(&stamp) else {
        return String::new();
    };
    let dir = rounds.join(sid);
    let Ok(files) = std::fs::read_dir(&dir) else {
        return String::new();
    };
    let mut out = String::new();
    for f in files.flatten() {
        let p = f.path();
        let Some(name) = p
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".answer.md"))
        else {
            continue;
        };
        if modified(&p).is_none_or(|t| t < since) || dir.join(format!("{name}.dropped")).exists() {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&p) {
            if take(&dir, name) {
                out += &format!("ANSWER to {name}:\n{text}\n");
            }
        }
    }
    if out.is_empty() {
        return out;
    }
    format!("The user answered a widget round after its waiter had stopped:\n{out}")
}

/// `where`, `wait` and `drop`: the calls a Claude session makes to ask through the widget.
/// `where` prints (and creates) this session's round folder; the session writes <name>.html there.
/// `wait <name>` blocks until the user sends the answer, prints it and exits — run it in the
/// background, its exit is what brings the session back. It gives up after a day (or the hours
/// given after the name); an answer sent later comes with the user's next message (`late_answers`).
/// `drop <name>` withdraws a round the user answered in the chat: the widget stops showing it and
/// closes its window, its waiter exits.
/// The session id comes from the CLAUDE_CODE_SESSION_ID that Claude Code gives every command it runs.
fn round_command(args: &[String]) -> Option<i32> {
    let cmd = args.get(1)?.as_str();
    if !["where", "wait", "drop"].contains(&cmd) {
        return None;
    }
    let sid = std::env::var("CLAUDE_CODE_SESSION_ID").unwrap_or_default();
    if sid.is_empty()
        || !sid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        eprintln!("CLAUDE_CODE_SESSION_ID is not set — run this from a Claude Code session");
        return Some(2);
    }
    let dir = state_dir()?.parent()?.join("rounds").join(&sid);
    let _ = std::fs::create_dir_all(&dir);
    if cmd == "where" {
        println!("{}", dir.display());
        return Some(0);
    }
    let name = args
        .get(2)
        .map(|n| n.trim_end_matches(".html").to_string())
        .unwrap_or_default();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        eprintln!("usage: meownitor-hook wait|drop <round-name>   (letters, digits, - and _)");
        return Some(2);
    }
    if !dir.join(format!("{name}.html")).exists() {
        eprintln!("no round {name}.html in {}", dir.display());
        return Some(2);
    }
    let dropped = dir.join(format!("{name}.dropped"));
    if cmd == "drop" {
        if std::fs::write(&dropped, "").is_err() {
            eprintln!("could not withdraw {name} in {}", dir.display());
            return Some(1);
        }
        println!("Round {name} withdrawn from the widget.");
        return Some(0);
    }
    let answer = dir.join(format!("{name}.answer.md"));
    let hours: u64 = args.get(3).and_then(|h| h.parse().ok()).unwrap_or(24);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(hours * 3600);
    while std::time::Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&answer) {
            if take(&dir, &name) {
                println!(
                    "ANSWER to {name}:
{text}"
                );
            } else {
                println!("The answer to {name} already came with the user's message.");
            }
            return Some(0);
        }
        if dropped.exists() {
            println!("Round {name} was withdrawn — no answer to wait for.");
            return Some(0);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    println!("No answer to {name} — gave up waiting. If the user answers later, it comes with their next message.");
    Some(1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = round_command(&args) {
        std::process::exit(code);
    }
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return;
    }
    let Ok(ev) = serde_json::from_str::<Value>(&buf) else {
        return;
    };
    let str_of = |k: &str| ev.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let sid = str_of("session_id");
    if sid.is_empty()
        || !sid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return;
    }
    let Some(dir) = state_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(format!("{sid}.json"));
    let mut st: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let prev = |k: &str| st.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let (prev_state, prev_what, prev_detail) = (prev("state"), prev("what"), prev("detail"));

    let event = str_of("hook_event_name");
    let tool = str_of("tool_name");
    let input = ev.get("tool_input").cloned().unwrap_or(Value::Null);
    // `what` is the tool's name, or a code the widget words in its own language: @think, @ask,
    // @agent, @perm, @done, @failed.
    let (state, what, detail): (String, String, String) = match event.as_str() {
        "SessionStart" => ("idle".into(), String::new(), String::new()),
        "UserPromptSubmit" => ("run".into(), "@think".into(), String::new()),
        "PreToolUse" if tool == "AskUserQuestion" => {
            ("wait".into(), "@ask".into(), describe(&tool, &input).1)
        }
        "PreToolUse" => {
            let (n, d) = describe(&tool, &input);
            ("run".into(), n, d)
        }
        // A background agent can finish after the turn has ended: that must not wake a finished session.
        "SubagentStop" if prev_state == "done" || prev_state == "idle" => {
            (prev_state.clone(), prev_what.clone(), prev_detail.clone())
        }
        "PostToolUse" | "PostToolUseFailure" | "SubagentStop" => {
            ("run".into(), "@think".into(), String::new())
        }
        "SubagentStart" => ("run".into(), "@agent".into(), str_of("agent_type")),
        "PermissionRequest" => {
            let (n, d) = describe(&tool, &input);
            (
                "wait".into(),
                "@perm".into(),
                if d.is_empty() {
                    n
                } else {
                    format!("{n} · {d}")
                },
            )
        }
        "Notification" if str_of("message").contains("permission") => {
            ("wait".into(), "@perm".into(), short(&str_of("message"), 80))
        }
        // "Claude is waiting for your input" and the like change nothing.
        "Notification" => (prev_state.clone(), prev_what.clone(), prev_detail.clone()),
        "Stop" => ("done".into(), "@done".into(), String::new()),
        "StopFailure" => ("done".into(), "@failed".into(), String::new()),
        "SessionEnd" => ("ended".into(), String::new(), String::new()),
        _ => return,
    };

    let now = now_ms();
    // The clock shows how long the session has been in its state — working since your message,
    // waiting since it asked — not how long the current tool has been running.
    // A new message starts a new turn even if the previous turn's Stop never arrived.
    let changed = state != prev_state || event == "UserPromptSubmit";
    st["sid"] = json!(sid);
    st["cwd"] = json!(str_of("cwd"));
    // The widget reads its tail: stopping a turn fires no hook and shows only there.
    st["transcript"] = json!(str_of("transcript_path"));
    st["event"] = json!(event);
    st["updated"] = json!(now);
    if changed || st.get("since").is_none() {
        st["since"] = json!(now);
    }
    st["state"] = json!(state);
    st["what"] = json!(what);
    st["detail"] = json!(detail);

    log_event(&dir, now, &sid, &event, &tool);

    let tmp = dir.join(format!("{sid}.{}.tmp", std::process::id()));
    if std::fs::write(&tmp, st.to_string()).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }

    // What this hook prints on UserPromptSubmit, Claude reads along with the message.
    if event == "UserPromptSubmit" {
        if let Some(rounds) = dir.parent().map(|d| d.join("rounds")) {
            print!("{}", late_answers(&rounds, &sid));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_answer_comes_once_and_an_old_one_never() {
        let rounds = std::env::temp_dir().join(format!("cw-rounds-{}", std::process::id()));
        let dir = rounds.join("s1");
        std::fs::create_dir_all(&dir).unwrap();
        // Answered before the hook kept count: a waiter of the time took it.
        std::fs::write(dir.join("old.answer.md"), "1 — A").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(
            late_answers(&rounds, "s1"),
            "",
            "the first call only sets the stamp"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));

        std::fs::write(dir.join("r1.answer.md"), "1 — B").unwrap();
        std::fs::write(dir.join("r2.answer.md"), "1 — C").unwrap();
        assert!(take(&dir, "r2"), "the waiter of r2 took its answer");
        let out = late_answers(&rounds, "s1");
        assert!(out.contains("ANSWER to r1:\n1 — B"), "{out}");
        assert!(!out.contains("r2") && !out.contains("old"), "{out}");
        assert_eq!(
            late_answers(&rounds, "s1"),
            "",
            "an answer goes to the session once"
        );
        assert!(!take(&dir, "r1"), "a waiter started now finds it taken");
        let _ = std::fs::remove_dir_all(&rounds);
    }
}
