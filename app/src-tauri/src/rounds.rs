// Questions from Claude. A session writes one HTML page — a description and one question or a
// group of them — into rounds/<session id>/<name>.html (`meownitor-hook where`) and waits for
// rounds/<session id>/<name>.answer.md (`meownitor-hook wait <name>`). The widget lists pending
// rounds, opens one in a window centred on the widget's monitor and serves it through the `round`
// scheme, which also takes the page's POST /answer — the same contract as the design pages — and
// POST /size, the page's own height, so the window fits its content. A round the session withdraws
// (`meownitor-hook drop <name>` leaves <name>.dropped) leaves the list and closes its window.
// The page kit (round-kit.css, round-kit.js) is built into the widget and served next to every page,
// so a page links it by its bare name and always gets the kit of the widget that shows it.
use std::path::PathBuf;
use std::time::Duration;
use tauri::http::{Request, Response};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, UriSchemeContext, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

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

/// Rounds that have no answer yet and that their session hasn't withdrawn (`meownitor-hook drop`).
pub fn pending() -> Vec<Pending> {
    dir().map(|root| pending_in(&root)).unwrap_or_default()
}

fn pending_in(root: &std::path::Path) -> Vec<Pending> {
    let mut out = Vec::new();
    let Ok(sessions) = std::fs::read_dir(root) else {
        return out;
    };
    for s in sessions.flatten() {
        let sid = s.file_name().to_string_lossy().to_string();
        if !safe(&sid) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(s.path()) else {
            continue;
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
            let html = std::fs::read_to_string(&p).unwrap_or_default();
            let since = f
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push(Pending {
                sid: sid.clone(),
                title: title_of(&html).unwrap_or_else(|| name.clone()),
                name,
                since,
            });
        }
    }
    out
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
        let list = pending_in(&root);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["open"]
        );
        assert_eq!(list[0].title, "T open");
    }
}
