// Questions from Claude. A session writes one HTML page — a description and one question or a
// group of them — into rounds/<session id>/<name>.html (`meownitor-hook where`) and waits for
// rounds/<session id>/<name>.answer.md (`meownitor-hook wait <name>`). The widget lists pending
// rounds, opens one in a window centred on the widget's monitor and serves it through the `round`
// scheme, which also takes the page's POST /answer — the same contract as the design pages — and
// POST /size, the page's own height, so the window fits its content. A round the session withdraws
// (`meownitor-hook drop <name>` leaves <name>.dropped) leaves the list and closes its window.
// The page kit (round-kit.css, round-kit.js) is built into the widget and served next to every page,
// so a page links it by its bare name and always gets the kit of the widget that shows it.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tauri::http::{Request, Response};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, UriSchemeContext, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

#[derive(Clone)]
pub struct Pending {
    pub sid: String,
    pub name: String,
    pub title: String,
    pub since: u64,
}

const KIT_CSS: &[u8] = include_bytes!("../../../skill/meownitor-round/round-kit.css");
const KIT_JS: &[u8] = include_bytes!("../../../skill/meownitor-round/round-kit.js");

pub fn dir() -> Option<PathBuf> {
    crate::sessions::data_dir().map(|d| d.join("rounds"))
}

fn safe(seg: &str) -> bool {
    !seg.is_empty()
        && seg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn title_of(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let a = lower.find("<title>")? + 7;
    let b = a + lower[a..].find("</title>")?;
    let t = html.get(a..b)?.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn modified_of(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// A pending round's page as it was last read.
struct Page {
    path: PathBuf,
    modified: Option<SystemTime>,
    found: Pending,
}

fn page_of(sid: &str, name: &str, path: &Path) -> Page {
    let modified = modified_of(path);
    let html = std::fs::read_to_string(path).unwrap_or_default();
    Page {
        path: path.to_path_buf(),
        modified,
        found: Pending {
            sid: sid.to_string(),
            name: name.to_string(),
            title: title_of(&html).unwrap_or_else(|| name.to_string()),
            since: modified
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        },
    }
}

/// The rounds in one session's folder that have no answer yet and that the session hasn't
/// withdrawn (`meownitor-hook drop`).
fn pages_in(dir: &Path, sid: &str) -> Vec<Page> {
    let mut out = Vec::new();
    let Ok(files) = std::fs::read_dir(dir) else {
        return out;
    };
    for f in files.flatten() {
        let p = f.path();
        if p.extension().is_none_or(|x| x != "html") {
            continue;
        }
        let name = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !safe(&name)
            || ["answer.md", "dropped"]
                .iter()
                .any(|x| p.with_file_name(format!("{name}.{x}")).exists())
        {
            continue;
        }
        out.push(page_of(sid, &name, &p));
    }
    out
}

/// The pending rounds, asked for three times a second (sessions.rs) while the folders hold every
/// page and picture of the last month — so they are not all read every time. A session's folder is
/// listed again only when its own modification time changed: a page, an answer, a withdrawal are
/// each a file added to it. A page still pending is read again only when it was rewritten. And
/// every `ANEW` everything is read anew, should a change ever not show in a folder's time.
pub struct Watch {
    dirs: HashMap<String, (Option<SystemTime>, Vec<Page>)>,
    anew: Option<Instant>,
}

const ANEW: Duration = Duration::from_secs(10);

impl Watch {
    pub fn new() -> Self {
        Watch {
            dirs: HashMap::new(),
            anew: None,
        }
    }

    pub fn pending(&mut self) -> Vec<Pending> {
        match dir() {
            Some(root) => self.pending_in(&root),
            None => Vec::new(),
        }
    }

    fn pending_in(&mut self, root: &Path) -> Vec<Pending> {
        let anew = self.anew.is_none_or(|t| t.elapsed() >= ANEW);
        if anew {
            self.anew = Some(Instant::now());
        }
        let mut seen = std::mem::take(&mut self.dirs);
        let mut out = Vec::new();
        let Ok(sessions) = std::fs::read_dir(root) else {
            return out;
        };
        for s in sessions.flatten() {
            let sid = s.file_name().to_string_lossy().to_string();
            if !safe(&sid) {
                continue;
            }
            let path = s.path();
            let modified = modified_of(&path);
            let mut pages = match seen.remove(&sid) {
                Some((was, pages)) if !anew && modified.is_some() && was == modified => pages,
                _ => pages_in(&path, &sid),
            };
            for p in &mut pages {
                if modified_of(&p.path) != p.modified {
                    *p = page_of(&sid, &p.found.name, &p.path);
                }
            }
            out.extend(pages.iter().map(|p| p.found.clone()));
            self.dirs.insert(sid, (modified, pages));
        }
        out
    }
}

/// How long a session's rounds are kept after the last thing happened in its folder.
const KEEP: Duration = Duration::from_secs(30 * 24 * 3600);

/// Removes the round folders of sessions where every round is answered or withdrawn and nothing
/// has changed for `KEEP` — pages, pictures and answers alike. A folder with a round still waiting
/// for its answer stays for as long as it waits. Returns how many folders went.
pub fn prune() -> usize {
    dir()
        .map(|root| prune_in(&root, SystemTime::now()))
        .unwrap_or(0)
}

fn prune_in(root: &Path, now: SystemTime) -> usize {
    let Ok(sessions) = std::fs::read_dir(root) else {
        return 0;
    };
    let mut gone = 0;
    for s in sessions.flatten() {
        let sid = s.file_name().to_string_lossy().to_string();
        // file_type() doesn't follow a link: a folder linked from elsewhere is not ours to empty.
        if !safe(&sid) || !s.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = s.path();
        if !pages_in(&path, &sid).is_empty() {
            continue;
        }
        let newest = std::fs::read_dir(&path)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|f| f.metadata().ok()?.modified().ok())
            .max()
            .or_else(|| modified_of(&path));
        if newest.is_some_and(|t| now.duration_since(t).is_ok_and(|age| age > KEEP))
            && std::fs::remove_dir_all(&path).is_ok()
        {
            gone += 1;
        }
    }
    gone
}

fn page_url(sid: &str, name: &str) -> String {
    #[cfg(windows)]
    return format!("http://round.localhost/{sid}/{name}.html");
    #[cfg(not(windows))]
    return format!("round://localhost/{sid}/{name}.html");
}

/// Tells the main window what the round page measured, so it grows to fit — never past 90% of the monitor.
const SIZE_SCRIPT: &str = "window.addEventListener('load',function(){setTimeout(function(){try{var d=document.documentElement,h=Math.max(d.scrollHeight,document.body?document.body.scrollHeight:0);fetch('/size',{method:'POST',body:JSON.stringify({h:h})});}catch(e){}},200);});";

fn work_area<R: Runtime>(w: &WebviewWindow<R>) -> Option<(f64, f64, f64, f64)> {
    let m = w
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten())?;
    let s = m.scale_factor();
    let a = m.work_area();
    Some((
        a.position.x as f64 / s,
        a.position.y as f64 / s,
        a.size.width as f64 / s,
        a.size.height as f64 / s,
    ))
}

/// Opens (or brings forward) the window for one round, centred on the monitor the widget is on.
pub fn open<R: Runtime>(
    app: &AppHandle<R>,
    from: &WebviewWindow<R>,
    sid: &str,
    name: &str,
    title: &str,
) -> Result<(), String> {
    if !safe(sid) || !safe(name) {
        return Err("bad round".into());
    }
    let label = format!("round-{}-{}", &sid[..sid.len().min(8)], name);
    let label = label.as_str();
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    let (x, y, ww, wh) = work_area(from).unwrap_or((0.0, 0.0, 1920.0, 1080.0));
    let (w, h) = ((ww * 0.9).min(1280.0), (wh * 0.9).min(980.0));
    let url = page_url(sid, name)
        .parse()
        .map_err(|_| "bad url".to_string())?;
    let lang = crate::i18n::lang();
    WebviewWindowBuilder::new(app, label, WebviewUrl::CustomProtocol(url))
        .title(lang.tr("round").replace("{title}", title))
        .inner_size(w, h)
        .position(x + (ww - w) / 2.0, y + (wh - h) / 2.0)
        .focused(true)
        // The kit words its buttons in the widget's language (skill/meownitor-round/round-kit.js).
        .initialization_script(format!("window.__ROUND_LANG='{}';", lang.code()))
        .initialization_script(SIZE_SCRIPT)
        .build()
        .map_err(|e| e.to_string())?;
    // The widget never takes focus, so Windows would leave a window it opens behind the others,
    // blinking in the taskbar: hold it on top for a moment to bring it forward.
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
        let w2 = w.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            let _ = w2.set_always_on_top(false);
        });
        follow_layout(app, &w, label);
        // A round its session withdrew while the window is open closes with it.
        if let Some(dropped) = dir().map(|d| d.join(sid).join(format!("{name}.dropped"))) {
            let (app, label) = (app.clone(), label.to_string());
            std::thread::spawn(move || {
                while let Some(w) = app.get_webview_window(&label) {
                    if dropped.exists() {
                        let _ = w.close();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
            });
        }
    }
    Ok(())
}

/// The page takes keys in WebView2's own process, and Windows gives a layout to one thread at a
/// time: to this app's when the window comes forward (the page kept an old one), to the page's when
/// the language is switched in it (the language bar, which follows this app's, stayed put while the
/// page typed in the other). While the round window is in front, whichever of the two changed
/// hands its layout to the other.
#[cfg(windows)]
fn follow_layout<R: Runtime>(app: &AppHandle<R>, w: &WebviewWindow<R>, label: &str) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        ActivateKeyboardLayout, GetKeyboardLayout,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, PostMessageW,
        GUITHREADINFO, WM_INPUTLANGCHANGEREQUEST,
    };
    let Ok(hwnd) = w.hwnd() else { return };
    let top = hwnd.0 as isize;
    let (app, label) = (app.clone(), label.to_string());
    std::thread::spawn(move || {
        let tid = unsafe { GetWindowThreadProcessId(top as _, std::ptr::null_mut()) };
        // The page's focused window and its thread, while the focus is in the page.
        let page = || unsafe {
            let mut gi: GUITHREADINFO = std::mem::zeroed();
            gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            let mut pid = 0;
            if GetGUIThreadInfo(tid, &mut gi) == 0 || gi.hwndFocus.is_null() {
                return None;
            }
            let ptid = GetWindowThreadProcessId(gi.hwndFocus, &mut pid);
            (pid != std::process::id()).then_some((gi.hwndFocus as isize, ptid))
        };
        // (window, page) layouts when they last agreed; None until the window is in front.
        let mut agreed: Option<(isize, isize)> = None;
        while app.get_webview_window(&label).is_some() {
            std::thread::sleep(Duration::from_millis(50));
            let Some((focus, ptid)) =
                page().filter(|_| unsafe { GetForegroundWindow() } as isize == top)
            else {
                agreed = None;
                continue;
            };
            let (win, pg) = unsafe {
                (
                    GetKeyboardLayout(tid) as isize,
                    GetKeyboardLayout(ptid) as isize,
                )
            };
            if win == pg {
                agreed = Some((win, pg));
            } else if agreed.is_some_and(|(_, was)| was != pg) {
                // Switched in the page: the window (and with it the language bar) follows.
                let _ = app.run_on_main_thread(move || unsafe {
                    ActivateKeyboardLayout(pg as _, 0);
                });
            } else {
                // Came forward with the layout Windows gave the window: the page follows.
                unsafe { PostMessageW(focus as _, WM_INPUTLANGCHANGEREQUEST, 0, win) };
            }
        }
    });
}

#[cfg(not(windows))]
fn follow_layout<R: Runtime>(_: &AppHandle<R>, _: &WebviewWindow<R>, _: &str) {}

fn reply(status: u16, mime: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .body(body)
        .unwrap()
}

/// The `round` scheme: GET serves the round's files, POST /answer records the answer and closes
/// the window, POST /size fits the window's height to the page.
pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    req: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let path = req.uri().path().to_string();
    let app = ctx.app_handle().clone();
    let label = ctx.webview_label().to_string();
    if req.method() == "POST" {
        let body: serde_json::Value = serde_json::from_slice(req.body()).unwrap_or_default();
        if path == "/answer" {
            let page = body.get("page").and_then(|v| v.as_str()).unwrap_or("");
            let text = body.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let parts: Vec<&str> = page.trim_start_matches('/').split('/').collect();
            let (Some(root), [sid, file]) = (dir(), parts.as_slice()) else {
                return reply(400, "application/json", br#"{"ok":false}"#.to_vec());
            };
            let name = file.trim_end_matches(".html");
            if !safe(sid) || !safe(name) || text.trim().is_empty() {
                return reply(400, "application/json", br#"{"ok":false}"#.to_vec());
            }
            let answer = root.join(sid).join(format!("{name}.answer.md"));
            if std::fs::write(&answer, text).is_err() {
                return reply(500, "application/json", br#"{"ok":false}"#.to_vec());
            }
            for old in crate::msix::legacy_answers(sid, name) {
                let _ = std::fs::write(old, text);
            }
            // Leave the page a moment to say "sent", then close it.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1400));
                if let Some(w) = app.get_webview_window(&label) {
                    let _ = w.close();
                }
            });
            return reply(200, "application/json", br#"{"ok":true}"#.to_vec());
        }
        if path == "/size" {
            if let (Some(w), Some(h)) = (
                app.get_webview_window(&label),
                body.get("h").and_then(|v| v.as_f64()),
            ) {
                if let (Some((x, y, ww, wh)), Ok(inner), Ok(scale)) =
                    (work_area(&w), w.inner_size(), w.scale_factor())
                {
                    let width = inner.width as f64 / scale;
                    let height = (h + 4.0).min(wh * 0.9).max(240.0);
                    let _ = w.set_size(LogicalSize::new(width, height));
                    let _ = w.set_position(LogicalPosition::new(
                        x + (ww - width) / 2.0,
                        y + (wh - height) / 2.0,
                    ));
                }
            }
            return reply(200, "application/json", br#"{"ok":true}"#.to_vec());
        }
        return reply(404, "text/plain", b"not found".to_vec());
    }
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let (Some(root), [sid, file]) = (dir(), parts.as_slice()) else {
        return reply(404, "text/plain", b"not found".to_vec());
    };
    match *file {
        "round-kit.css" => return reply(200, "text/css", KIT_CSS.to_vec()),
        "round-kit.js" => return reply(200, "text/javascript", KIT_JS.to_vec()),
        _ => {}
    }
    let (stem, ext) = file.rsplit_once('.').unwrap_or((file, ""));
    let mime = match ext {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" => "text/javascript",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return reply(404, "text/plain", b"not found".to_vec()),
    };
    if !safe(sid)
        || !stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        || stem.contains("..")
    {
        return reply(404, "text/plain", b"not found".to_vec());
    }
    match std::fs::read(root.join(sid).join(file)) {
        Ok(bytes) => reply(200, mime, bytes),
        Err(_) => reply(404, "text/plain", b"not found".to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answered_and_withdrawn_rounds_are_not_pending() {
        let root = std::env::temp_dir().join(format!("cw-pending-{}", std::process::id()));
        let dir = root.join("s1");
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["open", "answered", "dropped"] {
            std::fs::write(
                dir.join(format!("{n}.html")),
                format!("<title>T {n}</title>"),
            )
            .unwrap();
        }
        std::fs::write(dir.join("answered.answer.md"), "1 — A").unwrap();
        std::fs::write(dir.join("dropped.dropped"), "").unwrap();
        let mut watch = Watch::new();
        let list = watch.pending_in(&root);
        assert_eq!(
            list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["open"]
        );
        assert_eq!(list[0].title, "T open");

        // What changes between two passes shows in the next one: a page rewritten in place, a new
        // page, an answer.
        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(dir.join("open.html"), "<title>T open, again</title>").unwrap();
        assert_eq!(watch.pending_in(&root)[0].title, "T open, again");
        std::fs::write(dir.join("next.html"), "<title>T next</title>").unwrap();
        std::fs::write(dir.join("open.answer.md"), "1 — B").unwrap();
        let list = watch.pending_in(&root);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["next"]
        );
    }

    /// Only a folder where every round is done and nothing changed for a month goes; one with a
    /// round still waiting stays however old it is.
    #[test]
    fn old_answered_rounds_go_and_waiting_ones_stay() {
        let root = std::env::temp_dir().join(format!("cw-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (sid, answered) in [("done", true), ("waits", false)] {
            let dir = root.join(sid);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("r.html"), "<title>T</title>").unwrap();
            std::fs::write(dir.join("r-dark.png"), b"png").unwrap();
            if answered {
                std::fs::write(dir.join("r.answer.md"), "1 — A").unwrap();
                std::fs::write(dir.join("r.taken"), "").unwrap();
            }
        }
        let now = SystemTime::now();
        assert_eq!(prune_in(&root, now), 0, "nothing here is a month old");
        assert_eq!(prune_in(&root, now + KEEP - Duration::from_secs(60)), 0);
        assert_eq!(prune_in(&root, now + KEEP + Duration::from_secs(60)), 1);
        assert!(!root.join("done").exists());
        assert!(
            root.join("waits").join("r.html").exists(),
            "an unanswered round is there to answer whenever the user gets to it"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
