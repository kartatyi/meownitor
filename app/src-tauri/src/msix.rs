// Claude Desktop on Windows is an MSIX package, and everything it starts — Claude Code sessions, our
// hook, a widget launched from a session — sees %LOCALAPPDATA%, %APPDATA% and HKCU\Software through
// the package's private copy (…\Packages\Claude_<id>\LocalCache): what lands there, `claude` in a
// terminal and the rest of Windows never see. Hence the data folder is ~\.meownitor, which no
// package redirects (sessions.rs), and a widget started inside the package starts itself again
// outside it, so its start-with-Windows entry goes into the real registry. Data from before — when
// it was Claude Widget, in ~\.claude-widget and before that %LOCALAPPDATA%\ClaudeWidget, as either
// side sees it — is copied over once; the hook copy there is swapped for this version, so sessions
// already running write to the new folder as well, and a round asked before the move gets its
// answer in the old folder too, where its `wait` is watching.
use std::path::{Path, PathBuf};

const MOVED: &str = "MOVED.txt";
/// The hook's name in the old folders.
const OLD_HOOK: &str = "claude-widget-hook.exe";

/// Claude Desktop's private copies of AppData: …\Packages\Claude_<publisher id>\LocalCache.
pub fn claude_packages() -> Vec<PathBuf> {
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        if let Ok(rd) = std::fs::read_dir(PathBuf::from(local).join("Packages")) {
            return rd
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("Claude_"))
                .map(|e| e.path().join("LocalCache"))
                .collect();
        }
    }
    Vec::new()
}

/// Where the data lived before ~\.meownitor, newest first: what is copied keeps what came first.
fn legacy_dirs() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let home = std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".claude-widget"));
    let plain = std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("ClaudeWidget"));
    let packaged = claude_packages()
        .into_iter()
        .map(|p| p.join("Local").join("ClaudeWidget"));
    home.into_iter()
        .chain(plain)
        .chain(packaged)
        .filter(|d| d.is_dir())
        .collect()
}

/// Copies what `from` has and `to` doesn't, two levels deep (rounds/<session>/<file>).
fn copy_missing(from: &Path, to: &Path, depth: u8) {
    let Ok(rd) = std::fs::read_dir(from) else {
        return;
    };
    let _ = std::fs::create_dir_all(to);
    for e in rd.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            if depth > 0 {
                copy_missing(&src, &dst, depth - 1);
            }
        } else if !dst.exists() {
            let _ = std::fs::copy(&src, &dst);
        }
    }
}

/// A session started before the move runs the hook from the old folder: that copy becomes this
/// version. A copy that is running (a session's `wait`) is renamed aside (hooks.rs `put_copy`).
/// Release builds only, so a debug build never swaps the hook under a running session.
fn swap_hook(old: &Path) {
    let Some(ours) = std::env::current_exe()
        .ok()
        .map(|e| e.with_file_name(crate::hooks::exe_name()))
    else {
        return;
    };
    if cfg!(debug_assertions) || !old.exists() || !ours.exists() {
        return;
    }
    crate::hooks::clear_aside(old);
    if std::fs::read(old).ok() == std::fs::read(&ours).ok() {
        return;
    }
    match crate::hooks::put_copy(&ours, old) {
        Ok(()) => crate::watchdog::log(&format!("updated the old hook copy {}", old.display())),
        Err(e) => crate::watchdog::log(&format!(
            "could not update the old hook copy {}: {e}",
            old.display()
        )),
    }
}

/// Started with Windows as Claude Widget: that entry gives way to one under this name, for this exe.
/// Release builds only, as with swap_hook.
fn move_autostart(name: &str) {
    let Some(old) = crate::lifecycle::autolaunch("Claude Widget") else {
        return;
    };
    if cfg!(debug_assertions) || !old.is_enabled().unwrap_or(false) || old.disable().is_err() {
        return;
    }
    match crate::lifecycle::autolaunch(name).map(|a| a.enable()) {
        Some(Ok(())) => crate::watchdog::log("moved start-with-Windows from Claude Widget"),
        _ => crate::watchdog::log("could not move start-with-Windows from Claude Widget"),
    }
}

/// Once per old folder: config, sessions and rounds come over; every start: the old hook copy is
/// kept current and settings.json points at the new one.
pub fn migrate(name: &str) {
    move_autostart(name);
    let Some(new) = crate::sessions::data_dir() else {
        return;
    };
    for old in legacy_dirs() {
        if !old.join(MOVED).exists() {
            // The hook passes on answers newer than rounds/.since (bin/hook.rs `late_answers`): what
            // comes over keeps its older time, so none of it is passed on again.
            let since = new.join("rounds").join(".since");
            if !since.exists() && std::fs::create_dir_all(new.join("rounds")).is_ok() {
                let _ = std::fs::write(&since, "");
            }
            copy_missing(&old, &new, 0);
            copy_missing(&old.join("sessions"), &new.join("sessions"), 0);
            copy_missing(&old.join("rounds"), &new.join("rounds"), 1);
            let _ = std::fs::write(
                old.join(MOVED),
                format!(
                    "Claude Widget is Meownitor now and keeps its data in {}.\n",
                    new.display()
                ),
            );
            crate::watchdog::log(&format!("copied the data from {}", old.display()));
        }
        swap_hook(&old.join("bin").join(OLD_HOOK));
    }
    if crate::hooks::elsewhere() {
        match crate::hooks::install() {
            Ok(_) => crate::watchdog::log("pointed the Claude Code hook at the new folder"),
            Err(e) => crate::watchdog::log(&format!(
                "could not point the Claude Code hook at the new folder: {e}"
            )),
        }
    }
}

/// Where else the answer to a round goes: next to the same round in an old folder, unanswered there.
pub fn legacy_answers(sid: &str, name: &str) -> Vec<PathBuf> {
    let rounds = legacy_dirs()
        .into_iter()
        .map(|d| d.join("rounds").join(sid));
    rounds
        .filter(|d| d.join(format!("{name}.html")).exists())
        .map(|d| d.join(format!("{name}.answer.md")))
        .filter(|a| !a.exists())
        .collect()
}

/// The package copy of AppData this process sees AppData through, if it does. Claude Code's
/// processes do, though they carry no package identity to ask about: a file made in %LOCALAPPDATA%
/// turns up in the copy.
#[cfg(windows)]
fn container() -> Option<PathBuf> {
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    let probe = format!("meownitor-probe-{}.tmp", std::process::id());
    std::fs::write(local.join(&probe), b"").ok()?;
    let copy = claude_packages()
        .into_iter()
        .find(|p| p.join("Local").join(&probe).exists());
    let _ = std::fs::remove_file(local.join(&probe));
    copy
}

/// The exe exists only in the package's copy of AppData (installed from a Claude session):
/// outside there is nothing to start.
#[cfg(windows)]
fn only_inside(copy: &Path, exe: &Path) -> bool {
    [("LOCALAPPDATA", "Local"), ("APPDATA", "Roaming")]
        .iter()
        .any(|(var, sub)| {
            let rel = std::env::var_os(var).and_then(|base| {
                exe.strip_prefix(PathBuf::from(base))
                    .ok()
                    .map(Path::to_path_buf)
            });
            rel.is_some_and(|rel| copy.join(sub).join(rel).exists())
        })
}

/// Other processes of this exe's name.
#[cfg(windows)]
fn others() -> Vec<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let Some(me) = std::env::current_exe()
        .ok()
        .and_then(|e| e.file_name().map(|n| n.to_string_lossy().into_owned()))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return out;
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
            if e.th32ProcessID != std::process::id()
                && String::from_utf16_lossy(&e.szExeFile[..n]).eq_ignore_ascii_case(&me)
            {
                out.push(e.th32ProcessID);
            }
            more = Process32NextW(snap, &mut e) != 0;
        }
        CloseHandle(snap);
    }
    out
}

/// Started inside Claude's container (from a session): Explorer, which is outside it, starts this
/// exe again, and this one steps aside once that one is running. True when it is.
#[cfg(windows)]
pub fn relaunch_outside() -> bool {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    let Some(copy) = container() else {
        return false;
    };
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    if only_inside(&copy, &exe) {
        crate::watchdog::log("started inside Claude's container from its copy of AppData (installed from a Claude session) — install from Explorer to run outside it");
        return false;
    }
    // Not again within a minute, should the new one land inside as well.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let Some(mark) = crate::sessions::data_dir().map(|d| d.join("relaunched-at")) else {
        return false;
    };
    if std::fs::read_to_string(&mark)
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .is_some_and(|t| now.saturating_sub(t) < 60)
    {
        crate::watchdog::log(
            "started inside Claude's container right after starting outside it: staying",
        );
        return false;
    }
    let _ = mark.parent().map(std::fs::create_dir_all);
    let _ = std::fs::write(&mark, now.to_string());
    let before = others();
    if let Err(e) = std::process::Command::new("explorer.exe").arg(&exe).spawn() {
        crate::watchdog::log(&format!(
            "started inside Claude's container; Explorer could not start it outside: {e}"
        ));
        return false;
    }
    let until = Instant::now() + Duration::from_secs(8);
    while Instant::now() < until {
        if others().iter().any(|p| !before.contains(p)) {
            crate::watchdog::log(
                "started inside Claude's container: Explorer started it again outside",
            );
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    crate::watchdog::log(
        "started inside Claude's container; no widget came up outside it: staying",
    );
    false
}

#[cfg(not(windows))]
pub fn relaunch_outside() -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// A round asked before the move gets its answer in the package's old folder too, once.
    #[test]
    fn answers_reach_rounds_from_before_the_move() {
        let local = std::env::temp_dir().join(format!("cw-msix-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&local);
        let round = local
            .join("Packages")
            .join("Claude_test")
            .join("LocalCache")
            .join("Local")
            .join("ClaudeWidget")
            .join("rounds")
            .join("sid-1");
        std::fs::create_dir_all(&round).unwrap();
        std::fs::write(round.join("q.html"), "<title>q</title>").unwrap();
        std::env::set_var("LOCALAPPDATA", &local);
        assert_eq!(
            legacy_answers("sid-1", "q"),
            vec![round.join("q.answer.md")]
        );
        assert!(legacy_answers("sid-1", "other").is_empty());
        std::fs::write(round.join("q.answer.md"), "a").unwrap();
        assert!(legacy_answers("sid-1", "q").is_empty(), "answered already");
        let _ = std::fs::remove_dir_all(&local);
    }
}
