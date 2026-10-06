// Claude session widget: a transparent always-on-top window whose geometry the web side drives
// through a few commands in physical pixels, so dragging, snapping to a screen edge and the
// cursor-following eyes behave the same on any monitor and any DPI.
#![windows_subsystem = "windows"]

mod config;
mod hooks;
mod i18n;
mod lifecycle;
mod limits;
mod msix;
mod rounds;
mod sessions;
mod watchdog;

use serde::Serialize;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
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

#[tauri::command]
fn place(window: WebviewWindow, x: i32, y: i32, w: u32, h: u32) {
    let _ = window.set_size(PhysicalSize::new(w, h));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// How to start: `MEOWNITOR_START` (card, settings, dock-l, dock-r, dock-r-open), `MEOWNITOR_MOOD` and
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

/// The tray menu: the mood and character switches (for trying them out) and Quit.
fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let l = i18n::lang();
    let item = |id: &str| MenuItem::with_id(app, id, l.tr(id), true, None::<&str>);
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
            &moods,
            &kinds,
            &PredefinedMenuItem::separator(app)?,
            &item("quit")?,
        ],
    )
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
fn show(window: WebviewWindow, beat: State<watchdog::Beat>) {
    watchdog::shown(&window, &beat);
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
        .register_uri_scheme_protocol("round", rounds::handle)
        .manage(DragOffset::default())
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
            drag_end
        ])
        .setup(|app| {
            // A widget, not an app to switch to: no Dock icon and no menu bar of its own.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let menu = tray_menu(app.handle())?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip(app.package_info().name.clone())
                .menu(&menu)
                .on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    if id == "quit" {
                        watchdog::log("quit from the tray");
                        app.exit(0);
                    } else {
                        let _ = app.emit("tray", id.to_string());
                    }
                })
                .build(app)?;
            hooks::refresh_copy();
            sessions::spawn(app.handle().clone());
            limits::spawn(app.handle().clone());
            watchdog::spawn(app.handle().clone());
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
