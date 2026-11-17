// One widget at a time. Started twice, it used to run twice: two pets, two chirps, both writing
// config.json (seen 2026-11-12 — every watchdog line in widget.log came in pairs). The widget now
// holds a lock on widget.lock in the data folder for as long as it runs, and one that finds the lock
// taken leaves. The lock goes with the process however it ends, so nothing stale is ever left.
// A widget started to take over from one that is on its way out — by the watchdog (watchdog.rs) —
// waits a few seconds for the lock first.
// Debug builds and capture runs (MEOWNITOR_START, main.rs) are let through: a build under work sits
// next to the installed widget.
use std::fs::{File, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

/// How long to wait for a widget that is leaving: the watchdog's old one is gone within 5 s.
const WAIT: Duration = Duration::from_secs(8);

/// Kept for the life of the process; the lock, if one was taken, is released when it ends.
pub struct Held(#[allow(dead_code)] Option<File>);

/// None when another widget is running.
pub fn take() -> Option<Held> {
    if cfg!(debug_assertions) || std::env::var_os("MEOWNITOR_START").is_some() {
        return Some(Held(None));
    }
    match crate::sessions::data_dir() {
        Some(dir) => take_in(&dir, WAIT),
        None => Some(Held(None)),
    }
}

/// Where the lock can't be had at all — no file, a filesystem without locks — the widget runs
/// without it: two widgets are better than none.
fn take_in(dir: &Path, wait: Duration) -> Option<Held> {
    let _ = std::fs::create_dir_all(dir);
    let Ok(file) = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("widget.lock"))
    else {
        return Some(Held(None));
    };
    let until = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => return Some(Held(Some(file))),
            Err(TryLockError::WouldBlock) if Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(TryLockError::WouldBlock) => return None,
            Err(TryLockError::Error(_)) => return Some(Held(None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_second_one_leaves_and_the_next_one_after_it_starts() {
        let dir = std::env::temp_dir().join(format!("cw-instance-{}", std::process::id()));
        let none = Duration::ZERO;
        let first = take_in(&dir, none).expect("nobody holds the lock yet");
        assert!(first.0.is_some());
        assert!(
            take_in(&dir, none).is_none(),
            "a widget is running: the second one leaves"
        );
        // The first one is on its way out: one that waits for it gets the lock.
        let leaving = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(first);
        });
        assert!(take_in(&dir, Duration::from_secs(5)).is_some());
        leaving.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
