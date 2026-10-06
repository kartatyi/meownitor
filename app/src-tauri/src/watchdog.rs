// The page's watchdog. The card calls `poll` every 70 ms; when that stops while the page is visible,
// the page is stuck. Seen 2026-10-06: Windows ran out of memory, WebView2's crash reporter died while
// dumping our WebView2 browser process and left it suspended — every invoke hung, the card froze and
// the window stayed click-through, so clicks went to whatever was under it. After this long without
// a poll:
//   4 s  the window takes the mouse again, so the widget can at least be clicked and dragged;
//   8 s  our WebView2 processes are resumed (a no-op for those that aren't suspended);
//   30 s they are killed and the widget starts over — not while a round window is open (its answer
//        would be lost), and not again within 10 minutes of the last such start.
// A long gap in the watchdog's own ticks means the PC slept, which starts the count over.
// widget.log in the data folder says when any of it happened.
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, WebviewWindow};

const TAKE_MOUSE: Duration = Duration::from_secs(4);
const RESUME: Duration = Duration::from_secs(8);
const RESTART: Duration = Duration::from_secs(30);
const AGAIN_AFTER_SECS: u64 = 10 * 60;

/// When the watchdog last started the widget over (unix seconds, `restarted-at` in the data folder:
/// a widget that msix.rs has Explorer start again doesn't inherit our environment).
fn restarted_at() -> Option<u64> {
    let mark = crate::sessions::data_dir()?.join("restarted-at");
    std::fs::read_to_string(mark).ok()?.trim().parse().ok()
}

#[derive(Default)]
pub struct Beat(Mutex<State>);

#[derive(Default)]
struct State {
    /// The last poll; None until the page's first.
    last: Option<Instant>,
    /// The page says it is hidden: its timers are throttled, so a quiet spell means nothing.
    hidden: bool,
    /// Click-through as the page last asked for it.
    ignore: bool,
    /// The window was shown. Before that GTK has no window to make click-through (tao unwraps the
    /// GdkWindow that isn't there yet), so click-through waits for `show`.
    shown: bool,
    /// How far the watchdog has gone since the last poll (0 = not at all).
    step: u8,
}

/// `poll`: the page is alive. If the watchdog had stepped in, the page's click-through goes back.
pub fn polled(w: &WebviewWindow, beat: &Beat) {
    let back = {
        let mut s = beat.0.lock().unwrap();
        s.last = Some(Instant::now());
        let back = (s.step > 0).then_some((s.step, s.ignore && s.shown));
        s.step = 0;
        back
    };
    if let Some((step, ignore)) = back {
        log(&format!("the page is back (after step {step})"));
        let _ = w.set_ignore_cursor_events(ignore);
    }
}

/// `ignore`: remembered, so it can be put back after the watchdog took the mouse, and applied once
/// the window is shown.
pub fn ignoring(w: &WebviewWindow, beat: &Beat, on: bool) {
    let shown = {
        let mut s = beat.0.lock().unwrap();
        s.ignore = on;
        s.shown
    };
    if shown {
        let _ = w.set_ignore_cursor_events(on);
    }
}

/// `show`: the window comes up with the click-through the page asked for meanwhile.
pub fn shown(w: &WebviewWindow, beat: &Beat) {
    let ignore = {
        let mut s = beat.0.lock().unwrap();
        s.shown = true;
        s.ignore
    };
    let _ = w.show();
    let _ = w.set_ignore_cursor_events(ignore);
}

/// `page_hidden`: visibility changes start the count over.
pub fn hidden(beat: &Beat, hidden: bool) {
    let mut s = beat.0.lock().unwrap();
    s.hidden = hidden;
    if s.last.is_some() {
        s.last = Some(Instant::now());
    }
}

/// Moves on to step `to` if the page is still quiet `after` the last poll — the check and the
/// move under one lock, since a poll may come in at any moment.
fn advance(beat: &Beat, to: u8, after: Duration) -> bool {
    let mut s = beat.0.lock().unwrap();
    let due = s.step + 1 == to && !s.hidden && s.last.is_some_and(|t| t.elapsed() >= after);
    if due {
        s.step = to;
    }
    due
}

pub fn spawn(app: AppHandle) {
    let healed = restarted_at();
    log(
        if healed.is_some_and(|t| now_secs().saturating_sub(t) < 60) {
            "started (by the watchdog)"
        } else {
            "started"
        },
    );
    std::thread::spawn(move || {
        let mut tick = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let beat = app.state::<Beat>();
            if tick.elapsed() > Duration::from_secs(5) {
                let mut s = beat.0.lock().unwrap();
                if s.last.is_some() {
                    s.last = Some(Instant::now());
                }
            }
            tick = Instant::now();
            if advance(&beat, 1, TAKE_MOUSE) {
                log("no poll for 4 s: the window takes the mouse");
                let a = app.clone();
                // On the main thread, where `poll` runs too: if one came in meanwhile, its
                // click-through stands.
                let _ = app.run_on_main_thread(move || {
                    if a.state::<Beat>().0.lock().unwrap().step >= 1 {
                        if let Some(w) = a.get_webview_window("main") {
                            let _ = w.set_ignore_cursor_events(false);
                        }
                    }
                });
            } else if advance(&beat, 2, RESUME) {
                log(&format!(
                    "no poll for 8 s: resumed {} WebView2 processes",
                    procs::resume()
                ));
            } else if !app.webview_windows().keys().any(|l| l != "main")
                && advance(&beat, 3, RESTART)
            {
                if healed.is_some_and(|t| now_secs().saturating_sub(t) < AGAIN_AFTER_SECS) {
                    log("no poll for 30 s; no restart, the last one was under 10 minutes ago");
                } else {
                    restart(&app);
                }
            }
        }
    });
}

/// WebView2 goes first: a new widget with the same profile would otherwise join the stuck browser.
fn restart(app: &AppHandle) {
    let killed = procs::kill();
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if let Some(mark) = crate::sessions::data_dir().map(|d| d.join("restarted-at")) {
        let _ = std::fs::write(mark, now_secs().to_string());
    }
    match std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .spawn()
    {
        Ok(_) => {
            log(&format!(
                "no poll for 30 s: killed {killed} WebView2 processes, started a new widget"
            ));
            app.exit(0);
            // Should the event loop not get to it, leave anyway.
            std::thread::sleep(Duration::from_secs(5));
            std::process::exit(0);
        }
        Err(e) => log(&format!(
            "no poll for 30 s: killed {killed} WebView2 processes, no new widget: {e}"
        )),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// One line in widget.log in the data folder, which starts over once it passes 256 KB.
pub fn log(msg: &str) {
    use std::io::Write;
    let Some(dir) = crate::sessions::data_dir() else {
        return;
    };
    let path = dir.join("widget.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(format!("{ms} {msg}\n").as_bytes());
    }
}

#[cfg(windows)]
mod procs {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetProcessTimes, OpenProcess, TerminateProcess,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SUSPEND_RESUME, PROCESS_TERMINATE,
    };

    #[link(name = "ntdll")]
    extern "system" {
        fn NtResumeProcess(process: HANDLE) -> i32;
    }

    fn started(h: HANDLE) -> Option<u64> {
        let z = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        (unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) } != 0)
            .then_some((c.dwHighDateTime as u64) << 32 | c.dwLowDateTime as u64)
    }

    /// Our WebView2 processes: msedgewebview2.exe descended from this process through
    /// msedgewebview2.exe (anything else we started, say Claude by a claude:// link, is left alone),
    /// each started after its parent, so a reused process id can't pass for one of them.
    fn ours() -> Vec<HANDLE> {
        let mut all = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return Vec::new();
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snap, &mut e) != 0;
            while more {
                let n = e
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(e.szExeFile.len());
                if String::from_utf16_lossy(&e.szExeFile[..n])
                    .eq_ignore_ascii_case("msedgewebview2.exe")
                {
                    all.push((e.th32ProcessID, e.th32ParentProcessID));
                }
                more = Process32NextW(snap, &mut e) != 0;
            }
            CloseHandle(snap);
        }
        let mut found = Vec::new();
        let Some(me) = started(unsafe { GetCurrentProcess() }) else {
            return found;
        };
        let mut parents = vec![(std::process::id(), me)];
        while let Some((parent, born)) = parents.pop() {
            for &(pid, _) in all
                .iter()
                .filter(|&&(pid, ppid)| ppid == parent && pid != parent)
            {
                let h = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION
                            | PROCESS_SUSPEND_RESUME
                            | PROCESS_TERMINATE,
                        0,
                        pid,
                    )
                };
                if h.is_null() {
                    continue;
                }
                match started(h) {
                    Some(t) if t >= born => {
                        found.push(h);
                        parents.push((pid, t));
                    }
                    _ => unsafe {
                        CloseHandle(h);
                    },
                }
            }
        }
        found
    }

    /// Resumes each of our WebView2 processes once; returns how many took it.
    pub fn resume() -> usize {
        ours()
            .into_iter()
            .filter(|&h| unsafe { (NtResumeProcess(h) >= 0, CloseHandle(h)).0 })
            .count()
    }

    /// Kills our WebView2 processes; returns how many.
    pub fn kill() -> usize {
        ours()
            .into_iter()
            .filter(|&h| unsafe { (TerminateProcess(h, 1) != 0, CloseHandle(h)).0 })
            .count()
    }
}

#[cfg(not(windows))]
mod procs {
    pub fn resume() -> usize {
        0
    }
    pub fn kill() -> usize {
        0
    }
}
