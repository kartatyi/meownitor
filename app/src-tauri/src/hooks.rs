// Installing and removing our Claude Code hook from ~/.claude/settings.json. Only our own entries
// are touched (recognised by "meownitor-hook" in the command, or "claude-widget-hook" from when it
// was Claude Widget), every write is preceded by a dated backup next to the file, and the rest of
// the file keeps its keys in their order.
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

const EVENTS: [&str; 11] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionRequest",
    "Stop",
    "SubagentStart",
    "SubagentStop",
];
const MARKS: [&str; 2] = ["meownitor-hook", "claude-widget-hook"];

fn ours(command: &str) -> bool {
    MARKS.iter().any(|m| command.contains(m))
}

#[derive(Serialize)]
pub struct Status {
    pub installed: bool,
    events: usize,
    settings: String,
}

fn settings_path() -> Option<PathBuf> {
    crate::sessions::home().map(|h| h.join(".claude").join("settings.json"))
}

pub fn exe_name() -> &'static str {
    if cfg!(windows) {
        "meownitor-hook.exe"
    } else {
        "meownitor-hook"
    }
}

/// Where the hook lives for Claude Code: outside the build folder, so rebuilding never breaks Claude.
fn installed_hook() -> Option<PathBuf> {
    crate::sessions::data_dir().map(|d| d.join("bin").join(exe_name()))
}

fn read_settings() -> Result<Value, String> {
    let p = settings_path().ok_or("no home folder")?;
    match std::fs::read_to_string(&p) {
        Ok(t) => serde_json::from_str(&t)
            .map_err(|e| format!("settings.json is not valid JSON ({e}) — not touching it")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("cannot read settings.json: {e}")),
    }
}

fn is_ours(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .is_some_and(|hs| {
            hs.iter()
                .any(|h| h.get("command").and_then(|c| c.as_str()).is_some_and(ours))
        })
}

/// The command our entries should run: the hook's copy in the data folder.
fn command() -> Option<String> {
    installed_hook().map(|t| format!("\"{}\"", t.to_string_lossy().replace('\\', "/")))
}

/// Our hook commands in settings.json.
fn our_commands(s: &Value) -> Vec<String> {
    let entries = s
        .get("hooks")
        .and_then(|h| h.as_object())
        .into_iter()
        .flat_map(|o| o.values())
        .filter_map(|a| a.as_array())
        .flatten();
    entries
        .filter_map(|e| e.get("hooks")?.as_array())
        .flatten()
        .filter_map(|h| h.get("command")?.as_str())
        .filter(|c| ours(c))
        .map(String::from)
        .collect()
}

/// Installed, but some entry runs the hook from another place (the data folder moved, msix.rs).
pub fn elsewhere() -> bool {
    let (Ok(s), Some(cmd)) = (read_settings(), command()) else {
        return false;
    };
    our_commands(&s).iter().any(|c| *c != cmd)
}

fn count_ours(s: &Value) -> usize {
    s.get("hooks").and_then(|h| h.as_object()).map_or(0, |o| {
        o.values()
            .filter(|arr| arr.as_array().is_some_and(|a| a.iter().any(is_ours)))
            .count()
    })
}

pub fn status() -> Status {
    let s = read_settings().unwrap_or_else(|_| json!({}));
    let n = count_ours(&s);
    Status {
        installed: n > 0,
        events: n,
        settings: settings_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}

fn write_with_backup(s: &Value) -> Result<(), String> {
    let p = settings_path().ok_or("no home folder")?;
    if p.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        std::fs::copy(
            &p,
            p.with_file_name(format!("settings.json.bak-meownitor-{stamp}")),
        )
        .map_err(|e| format!("backup failed: {e}"))?;
    }
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())? + "\n";
    let tmp = p.with_extension("json.widget-tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// Copies the hook next to the widget into the data folder and adds it to every event it needs;
/// entries already there are pointed at that copy.
pub fn install() -> Result<Status, String> {
    let target = installed_hook().ok_or("no data folder")?;
    let source = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name(exe_name());
    if source.exists() && source != target {
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::copy(&source, &target).map_err(|e| format!("cannot copy the hook: {e}"))?;
    }
    if !target.exists() {
        return Err(format!("{} not found next to the widget", exe_name()));
    }
    let mut s = read_settings()?;
    let command = command().ok_or("no data folder")?;
    let hooks = s
        .as_object_mut()
        .ok_or("settings.json is not an object")?
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or("\"hooks\" is not an object")?;
    for ev in EVENTS {
        let arr = hooks.entry(ev).or_insert_with(|| json!([]));
        let Some(arr) = arr.as_array_mut() else {
            continue;
        };
        let mut had = false;
        for h in arr
            .iter_mut()
            .filter(|e| is_ours(e))
            .filter_map(|e| e.get_mut("hooks")?.as_array_mut())
            .flatten()
        {
            if h.get("command").and_then(|c| c.as_str()).is_some_and(ours) {
                h["command"] = json!(command);
                had = true;
            }
        }
        if !had {
            arr.push(json!({ "matcher": "", "hooks": [{ "type": "command", "command": command, "timeout": 5 }] }));
        }
    }
    write_with_backup(&s)?;
    Ok(status())
}

/// After an update the installer leaves a new hook next to the widget while Claude Code keeps
/// running the copy in the data folder: while the hook is installed, keep that copy the same.
/// Release builds only, so running a debug build never swaps the hook under Claude.
pub fn refresh_copy() {
    if cfg!(debug_assertions) {
        return;
    }
    let (Some(target), Ok(exe)) = (installed_hook(), std::env::current_exe()) else {
        return;
    };
    let source = exe.with_file_name(exe_name());
    if source == target || !source.exists() || !target.exists() || !status().installed {
        return;
    }
    if std::fs::read(&source).ok() != std::fs::read(&target).ok() {
        let _ = std::fs::copy(&source, &target);
    }
}

/// Removes only our entries; an event left with nothing is removed too.
pub fn uninstall() -> Result<Status, String> {
    let mut s = read_settings()?;
    if let Some(hooks) = s.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        let keys: Vec<String> = hooks.keys().cloned().collect();
        for k in keys {
            if let Some(arr) = hooks.get_mut(&k).and_then(|a| a.as_array_mut()) {
                arr.retain(|e| !is_ours(e));
                if arr.is_empty() {
                    hooks.remove(&k);
                }
            }
        }
    }
    write_with_backup(&s)?;
    Ok(status())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Install and uninstall against a throwaway home: foreign hooks and key order survive,
    /// installing twice adds nothing, every write leaves a backup.
    #[test]
    fn install_and_uninstall_touch_only_our_entries() {
        let home = std::env::temp_dir().join(format!("cw-hooks-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::env::set_var("USERPROFILE", &home);
        std::env::set_var("HOME", &home);
        let hook = installed_hook().unwrap();
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(&hook, b"x").unwrap();
        let original = r#"{
  "zeta": 1,
  "hooks": {
    "Stop": [{"matcher": "", "hooks": [{"type": "command", "command": "node other.js", "timeout": 5}]}]
  },
  "alpha": true
}"#;
        let settings = home.join(".claude").join("settings.json");
        std::fs::write(&settings, original).unwrap();

        install().unwrap();
        install().unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["zeta", "hooks", "alpha"], "key order kept");
        let stop = v["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "foreign Stop hook kept, ours added once");
        assert!(stop[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("other.js"));
        assert_eq!(count_ours(&v), EVENTS.len());
        assert!(status().installed);
        assert!(!elsewhere());

        // Entries left pointing at an old copy are re-pointed, not doubled.
        let path = installed_hook()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        std::fs::write(
            &settings,
            std::fs::read_to_string(&settings).unwrap().replace(
                &path,
                "C:/Users/x/.claude-widget/bin/claude-widget-hook.exe",
            ),
        )
        .unwrap();
        assert!(elsewhere());
        install().unwrap();
        assert!(!elsewhere());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(count_ours(&v), EVENTS.len());
        assert_eq!(
            v["hooks"]["Stop"].as_array().unwrap().len(),
            2,
            "re-pointed, not added again"
        );

        uninstall().unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(count_ours(&v), 0);
        assert_eq!(
            v["hooks"]["Stop"].as_array().unwrap().len(),
            1,
            "foreign hook survives uninstall"
        );
        assert!(
            v["hooks"].get("PreToolUse").is_none(),
            "an event left empty is removed"
        );
        let backups = std::fs::read_dir(home.join(".claude"))
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.bak-meownitor-")
            })
            .count();
        assert!(backups >= 1, "backups written");
        let _ = std::fs::remove_dir_all(&home);
    }
}
