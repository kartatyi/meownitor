// Claude session widget: a transparent always-on-top window whose geometry the web side drives
// through a few commands in physical pixels, so dragging, snapping to a screen edge and the
// cursor-following eyes behave the same on any monitor and any DPI.
#![windows_subsystem = "windows"]

mod config;
mod hooks;
mod i18n;
mod instance;
mod lifecycle;
mod limits;
mod msix;
mod ontop;
mod perms;
mod rounds;
mod sessions;
mod skill;
mod visible;
mod watchdog;

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, PhysicalPosition, PhysicalSize, State, WebviewWindow, Wry};

/// Cursor-to-window offset captured when a drag starts.
#[derive(Default)]
struct DragOffset(Mutex<Option<(f64, f64)>>);

#[derive(Serialize, Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Serialize)]
struct Poll {
    cursor: (f64, f64),
    win: Rect,
    work: Rect,
    scale: f64,
}

fn snapshot(w: &WebviewWindow) -> Poll {
    let cursor = w
        .cursor_position()
        .map(|p| (p.x, p.y))
        .unwrap_or((-1e9, -1e9));
    let pos = w.outer_position().unwrap_or_default();
    let size = w.outer_size().unwrap_or_default();
    let monitor = w
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());
    let (work, scale) = match monitor {
        Some(m) => {
            let a = m.work_area();
            (
                Rect {
                    x: a.position.x as f64,
                    y: a.position.y as f64,
                    w: a.size.width as f64,
                    h: a.size.height as f64,
                },
                m.scale_factor(),
            )
        }
        None => (
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0,
            },
            1.0,
        ),
    };
    Poll {
        cursor,
        win: Rect {
            x: pos.x as f64,
            y: pos.y as f64,
            w: size.width as f64,
            h: size.height as f64,
        },
        work,
        scale,
    }
}

#[tauri::command]
fn poll(window: WebviewWindow, beat: State<watchdog::Beat>) -> Poll {
    watchdog::polled(&window, &beat);
    snapshot(&window)
}

/// Moves and sizes the window in one step. On Windows tao's set_size and set_position are two
/// SetWindowPos calls, and a frame shown between them has the window at its new size but its old
/// place — at the right edge the content jumped right and back.
#[tauri::command]
fn place(window: WebviewWindow, x: i32, y: i32, w: u32, h: u32) {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER,
        };
        let flags = SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE;
        unsafe {
            SetWindowPos(
                hwnd.0 as _,
                std::ptr::null_mut(),
                x,
                y,
                w as i32,
                h as i32,
                flags,
            )
        };
        return;
    }
    let _ = window.set_size(PhysicalSize::new(w, h));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// How to start: `MEOWNITOR_START` (card, settings, dock-l, dock-r, dock-t, `-open` to start slid out,
/// `:strip` / `:peek` / `:line` for the edge look, e.g. dock-t-open:peek), `MEOWNITOR_MOOD` and
/// `MEOWNITOR_ROUND` (<session id>/<round name>, opened right away) —
/// so every mode can be opened and captured without touching the mouse.
#[tauri::command]
fn start_hint() -> (Option<String>, Option<String>, Option<String>) {
    (
        std::env::var("MEOWNITOR_START").ok(),
        std::env::var("MEOWNITOR_MOOD").ok(),
        std::env::var("MEOWNITOR_ROUND").ok(),
    )
}

#[tauri::command]
fn sessions(latest: State<sessions::Latest>) -> Vec<sessions::Session> {
    latest.0.lock().unwrap().clone()
}

#[tauri::command]
fn limits(latest: State<limits::Latest>) -> limits::Limits {
    latest.0.lock().unwrap().clone()
}

// Async on purpose: creating a window from a synchronous command deadlocks on Windows.
#[tauri::command]
async fn open_round(
    app: tauri::AppHandle,
    window: WebviewWindow,
    sid: String,
    name: String,
    title: String,
) -> Result<(), String> {
    rounds::open(&app, &window, &sid, &name, &title)
}

#[tauri::command]
fn get_config() -> serde_json::Value {
    config::read()
}

#[tauri::command]
fn set_config(patch: serde_json::Value) -> serde_json::Value {
    if let Some(on) = patch.get("hideOnFullscreen").and_then(|v| v.as_bool()) {
        ontop::hide_on_fullscreen(on);
    }
    config::merge(&patch)
}

/// The card switched language: remember it and re-word the tray menu.
#[tauri::command]
fn set_lang(app: AppHandle, lang: String) -> Result<(), String> {
    config::merge(&serde_json::json!({ "lang": if lang == "uk" { "uk" } else { "en" } }));
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(tray_menu(&app).map_err(|e| e.to_string())?))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// «Do not disturb» as the card last reported it (`tray_state`): the tray item's tick.
static QUIET: AtomicBool = AtomicBool::new(false);

/// The tray menu: opening the card and the settings, sending the widget away for an hour or
/// bringing it back, an hour of quiet, the data folder, the mood and character switches (for
/// trying them out) and Quit.
fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let l = i18n::lang();
    let item = |id: &str| MenuItem::with_id(app, id, l.tr(id), true, None::<&str>);
    let hide = item(if visible::sent_away(app) {
        "unhide"
    } else {
        "hide"
    })?;
    let quiet = CheckMenuItem::with_id(
        app,
        "quiet",
        l.tr("quiet"),
        true,
        QUIET.load(Ordering::Relaxed),
        None::<&str>,
    )?;
    let moods = Submenu::with_items(
        app,
        l.tr("mood"),
        true,
        &[
            &item("mood:auto")?,
            &item("mood:idle")?,
            &item("mood:work")?,
            &item("mood:ask")?,
            &item("mood:done")?,
            &item("mood:sleep")?,
            &item("mood:tired")?,
        ],
    )?;
    let kinds = Submenu::with_items(
        app,
        l.tr("character"),
        true,
        &[
            &item("kind:cat")?,
            &item("kind:blob")?,
            &item("kind:ghost")?,
        ],
    )?;
    Menu::with_items(
        app,
        &[
            &item("open")?,
            &item("settings")?,
            &PredefinedMenuItem::separator(app)?,
            &hide,
            &quiet,
            &item("data")?,
            &PredefinedMenuItem::separator(app)?,
            &moods,
            &kinds,
            &PredefinedMenuItem::separator(app)?,
            &item("quit")?,
        ],
    )
}

/// The tray menu again, after something it words or ticks has changed.
pub(crate) fn refresh_tray(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

/// What the tray icon says on hover — how many sessions wait and work, the nearest limit — and
/// whether «Do not disturb» is on, as the card has them.
#[tauri::command]
fn tray_state(app: AppHandle, tip: String, quiet: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(tip));
    }
    if QUIET.swap(quiet, Ordering::Relaxed) != quiet {
        refresh_tray(&app);
    }
}

/// A click in the tray menu. What only the card can do goes on to it as a "tray" event.
fn tray_clicked(app: &AppHandle, id: &str) {
    match id {
        "quit" => {
            watchdog::log("quit from the tray");
            app.exit(0);
        }
        "hide" => visible::hide_for(app, Duration::from_secs(3600)),
        "unhide" => visible::show(app),
        "data" => {
            if let Some(dir) = sessions::data_dir() {
                open_path(&dir);
            }
        }
        _ => {
            // The card and its settings are no use hidden.
            if id == "open" || id == "settings" {
                visible::show(app);
            }
            let _ = app.emit("tray", id.to_string());
        }
    }
}

/// Opens a folder in the system's file manager.
fn open_path(dir: &std::path::Path) -> bool {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(dir).spawn().is_ok()
}

/// A terminal session's folder, from its row.
#[tauri::command]
fn open_folder(path: String) -> bool {
    let dir = std::path::Path::new(&path);
    dir.is_dir() && open_path(dir)
}

/// The clipboard, kept for as long as the widget runs: under X11 what was copied goes with the
/// one who copied it.
static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

/// `claude --resume …`, from a terminal session's row.
#[tauri::command]
fn copy_text(text: String) -> bool {
    let mut held = CLIPBOARD.lock().unwrap();
    if held.is_none() {
        *held = arboard::Clipboard::new().ok();
    }
    held.as_mut().is_some_and(|c| c.set_text(text).is_ok())
}

/// A system notification: a session started waiting for you.
#[tauri::command]
fn notify(app: AppHandle, title: String, body: String) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

/// The click on a permission request's row: "allow", "deny", or "dialog" to leave it to Claude
/// Code's own dialog. Taken from the widget's own window only — a round's page is written by a
/// session, and a session must not answer its own request. widget.log keeps what was answered.
#[tauri::command]
fn permit(window: WebviewWindow, sid: String, answer: String) -> bool {
    if window.label() != "main" || !["allow", "deny", "dialog"].contains(&answer.as_str()) {
        return false;
    }
    let answered = perms::answer(&sid, &answer);
    let short: String = sid.chars().take(8).collect();
    watchdog::log(&format!(
        "permission request of {short}: {answer}{}",
        if answered {
            ""
        } else {
            " — no longer waiting"
        }
    ));
    answered
}

#[tauri::command]
fn autostart_get(app: tauri::AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn autostart_set(app: tauri::AppHandle, on: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if on { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    Ok(al.is_enabled().unwrap_or(false))
}

#[tauri::command]
fn hook_status() -> hooks::Status {
    hooks::status()
}

#[tauri::command]
fn hook_install() -> Result<hooks::Status, String> {
    hooks::install()
}

#[tauri::command]
fn hook_uninstall() -> Result<hooks::Status, String> {
    hooks::uninstall()
}

#[tauri::command]
fn open_session(local: String) {
    sessions::open_in_desktop(&local);
}

#[tauri::command]
fn show(app: AppHandle, window: WebviewWindow, beat: State<watchdog::Beat>) {
    watchdog::shown(&window, &beat);
    // Unless there is a reason for it to stay hidden: a full-screen window was in front already.
    visible::apply(&app);
}

#[tauri::command]
fn ignore(window: WebviewWindow, beat: State<watchdog::Beat>, on: bool) {
    watchdog::ignoring(&window, &beat, on);
}

#[tauri::command]
fn page_hidden(beat: State<watchdog::Beat>, hidden: bool) {
    watchdog::hidden(&beat, hidden);
}

#[tauri::command]
fn drag_start(window: WebviewWindow, drag: State<DragOffset>) {
    let p = snapshot(&window);
    *drag.0.lock().unwrap() = Some((p.cursor.0 - p.win.x, p.cursor.1 - p.win.y));
}

#[tauri::command]
fn drag_move(window: WebviewWindow, drag: State<DragOffset>) {
    let offset = *drag.0.lock().unwrap();
    if let (Some((dx, dy)), Ok(c)) = (offset, window.cursor_position()) {
        let _ = window.set_position(PhysicalPosition::new(
            (c.x - dx).round() as i32,
            (c.y - dy).round() as i32,
        ));
    }
}

#[tauri::command]
fn drag_end(window: WebviewWindow, drag: State<DragOffset>) -> Poll {
    *drag.0.lock().unwrap() = None;
    snapshot(&window)
}

fn main() {
    let context = tauri::generate_context!();
    // The installer's calls (windows/installer-hooks.nsh) do their work and exit, with no window.
    match std::env::args().nth(1).as_deref() {
        Some("--uninstall") => return lifecycle::uninstall(&context.package_info().name),
        Some("--restore") => return lifecycle::restore(&context.package_info().name),
        _ => {}
    }
    if msix::relaunch_outside() {
        return;
    }
    // One widget at a time (instance.rs); the lock is held until the process ends.
    let Some(_only) = instance::take() else {
        watchdog::log("another widget is already running: leaving");
        return;
    };
    msix::migrate(&context.package_info().name);
    // Wayland lets no window place itself or see the cursor outside it, and the widget does both
    // (dragging, snapping to an edge, the eyes): wherever there is an X server — XWayland under a
    // Wayland session — it goes through X, which GTK would otherwise pick only after Wayland.
    // WebKitGTK's DMA-BUF renderer leaves the page blank where the GPU can't share buffers (NVIDIA,
    // virtual machines, WSL); the older path is plenty for a widget this small.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var_os("DISPLAY").is_some() && std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .register_uri_scheme_protocol("round", rounds::handle)
        .manage(DragOffset::default())
        .manage(visible::Away::default())
        .manage(watchdog::Beat::default())
        .manage(sessions::Latest(Mutex::new(Vec::new())))
        .manage(limits::Latest(Mutex::new(limits::Limits::default())))
        .invoke_handler(tauri::generate_handler![
            poll,
            place,
            start_hint,
            sessions,
            limits,
            open_round,
            open_session,
            get_config,
            set_config,
            set_lang,
            autostart_get,
            autostart_set,
            hook_status,
            hook_install,
            hook_uninstall,
            show,
            ignore,
            page_hidden,
            drag_start,
            drag_move,
            drag_end,
            tray_state,
            open_folder,
            copy_text,
            notify,
            permit
        ])
        .setup(|app| {
            // A widget, not an app to switch to: no Dock icon and no menu bar of its own.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            ontop::hide_on_fullscreen(
                config::read()
                    .get("hideOnFullscreen")
                    .and_then(|v| v.as_bool())
                    != Some(false),
            );
            let menu = tray_menu(app.handle())?;
            let tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(app.package_info().name.clone())
                .menu(&menu)
                .on_menu_event(|app, event| tray_clicked(app, event.id.as_ref()));
            // On Windows a click on the icon opens the card, the menu is on the right button; a
            // Mac's menu bar item opens its menu, and Linux tells of no clicks at all.
            #[cfg(windows)]
            let tray = tray
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        tray_clicked(tray.app_handle(), "open");
                    }
                });
            tray.build(app)?;
            hooks::refresh_copy();
            hooks::catch_up();
            // The skill the sessions read is the one this widget came with (release builds only,
            // like the hook's copy).
            if !cfg!(debug_assertions) && hooks::status().installed {
                let _ = skill::install();
            }
            perms::spawn();
            sessions::spawn(app.handle().clone());
            limits::spawn(app.handle().clone());
            watchdog::spawn(app.handle().clone());
            ontop::spawn(app.handle().clone());
            Ok(())
        })
        .build(context)
        .expect("error while running the widget")
        .run(|_, event| {
            // With "quit from the tray" or the watchdog's lines before it, or neither (a closed window).
            if let tauri::RunEvent::Exit = event {
                watchdog::log("exit");
            }
        });
}
