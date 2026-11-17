// Hiding the widget and bringing it back. Two reasons hide it: the user sent it away for a while
// from the tray, and — on Windows — a full-screen window is in front on the widget's monitor: a
// film, a slide show, a game (ontop.rs looks). It comes back when neither holds. Only the window
// goes: sounds and notifications carry on as they were.
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct Away(Mutex<State>);

#[derive(Default)]
struct State {
    /// Sent away from the tray until then.
    until: Option<Instant>,
    /// A full-screen window is in front on our monitor.
    covered: bool,
}

/// Hides or shows the window as the two reasons stand now, and re-words the tray menu, whose item
/// says «Hide for an hour» or «Show the widget».
pub fn apply(app: &AppHandle) {
    let hide = {
        let away = app.state::<Away>();
        let mut s = away.0.lock().unwrap();
        if s.until.is_some_and(|t| t <= Instant::now()) {
            s.until = None;
        }
        s.until.is_some() || s.covered
    };
    let beat = app.state::<crate::watchdog::Beat>();
    if let Some(w) = app.get_webview_window("main") {
        if hide {
            crate::watchdog::away(&beat, true);
            let _ = w.hide();
        } else if crate::watchdog::away(&beat, false) {
            crate::watchdog::shown(&w, &beat);
        }
    }
    crate::refresh_tray(app);
}

/// Whether the user sent the widget away and that time isn't over.
pub fn sent_away(app: &AppHandle) -> bool {
    let away = app.state::<Away>();
    let s = away.0.lock().unwrap();
    s.until.is_some_and(|t| t > Instant::now())
}

pub fn hide_for(app: &AppHandle, time: Duration) {
    let until = Instant::now() + time;
    app.state::<Away>().0.lock().unwrap().until = Some(until);
    apply(app);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(time + Duration::from_millis(50));
        // Unless it was shown, or sent away anew, meanwhile.
        if app.state::<Away>().0.lock().unwrap().until == Some(until) {
            apply(&app);
        }
    });
}

/// Back from the tray, whatever time was left.
pub fn show(app: &AppHandle) {
    app.state::<Away>().0.lock().unwrap().until = None;
    apply(app);
}

/// A full-screen window came in front on our monitor, or left.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn cover(app: &AppHandle, on: bool) {
    let changed = {
        let away = app.state::<Away>();
        let mut s = away.0.lock().unwrap();
        let changed = s.covered != on;
        s.covered = on;
        changed
    };
    if changed {
        apply(app);
    }
}
