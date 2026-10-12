// Keeping the widget on top. Seen 2026-10-05 and again 2026-10-12: some time into a run the window
// ends up below ordinary windows (a maximized one covers it), its WS_EX_TOPMOST bit still set. What
// demotes it isn't known, and tao asks Windows for HWND_TOPMOST only when always-on-top *changes*,
// so nothing puts it back. Every 2 s, on the window's thread: if a visible window that isn't topmost
// sits above ours, ours goes back to the top of the topmost band — without taking the focus. Each
// time widget.log says what was above it, so the culprit can be found.
use std::time::Duration;
use tauri::{AppHandle, Manager};

const EVERY: Duration = Duration::from_secs(2);

#[cfg(windows)]
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(EVERY);
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = a.get_webview_window("main") {
                if let Ok(hwnd) = w.hwnd() {
                    win::keep(hwnd.0 as _, &w);
                }
            }
        });
    });
}

#[cfg(not(windows))]
pub fn spawn(_: AppHandle) {}

#[cfg(windows)]
mod win {
    use crate::watchdog::log;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tauri::WebviewWindow;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetWindow, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible,
        SetWindowPos, GWL_EXSTYLE, GW_HWNDPREV, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, WS_EX_TOPMOST,
    };

    fn topmost(h: HWND) -> bool {
        unsafe { GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 }
    }

    fn cloaked(h: HWND) -> bool {
        let mut c = 0u32;
        unsafe {
            DwmGetWindowAttribute(h, DWMWA_CLOAKED as _, &mut c as *mut u32 as _, 4) == 0 && c != 0
        }
    }

    /// The first window above ours that shows on screen and isn't topmost, if any.
    fn over(ours: HWND) -> Option<HWND> {
        let mut h = ours;
        loop {
            h = unsafe { GetWindow(h, GW_HWNDPREV) };
            if h.is_null() {
                return None;
            }
            if unsafe { IsWindowVisible(h) } != 0 && !topmost(h) && !cloaked(h) {
                return Some(h);
            }
        }
    }

    /// "explorer.exe [CabinetWClass]", for the log.
    fn describe(h: HWND) -> String {
        let mut class = [0u16; 128];
        let n = unsafe { GetClassNameW(h, class.as_mut_ptr(), class.len() as i32) }.max(0);
        let class = String::from_utf16_lossy(&class[..n as usize]);
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(h, &mut pid) };
        let mut exe = format!("pid {pid}");
        unsafe {
            let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if !p.is_null() {
                let mut buf = [0u16; 260];
                let mut len = buf.len() as u32;
                if QueryFullProcessImageNameW(p, 0, buf.as_mut_ptr(), &mut len) != 0 {
                    let path = String::from_utf16_lossy(&buf[..len as usize]);
                    exe = path.rsplit('\\').next().unwrap_or(&path).to_string();
                }
                CloseHandle(p);
            }
        }
        format!("{exe} [{class}]")
    }

    fn raise(h: HWND) {
        unsafe {
            SetWindowPos(
                h,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }

    pub fn keep(ours: HWND, w: &WebviewWindow) {
        if unsafe { IsWindowVisible(ours) } == 0 {
            return;
        }
        let Some(above) = over(ours) else {
            STUCK.store(false, Ordering::Relaxed);
            return;
        };
        let bit = if topmost(ours) { "set" } else { "cleared" };
        let what = describe(above);
        raise(ours);
        if over(ours).is_none() {
            log(&format!("buried under {what} (topmost bit {bit}): raised"));
            return;
        }
        // Still under it: always-on-top off and on again, which tao turns into the same call
        // after dropping the window out of the topmost band first. The next check tells whether
        // that took; should nothing help, the log says so once, not every 2 s.
        if !STUCK.swap(true, Ordering::Relaxed) {
            log(&format!(
                "buried under {what} (topmost bit {bit}): always-on-top off and on"
            ));
        }
        let _ = w.set_always_on_top(false);
        let _ = w.set_always_on_top(true);
    }

    static STUCK: AtomicBool = AtomicBool::new(false);
}
