// The meownitor-round skill in ~/.claude/skills: it comes with the widget, so a session asking
// through the widget always reads the contract of the widget that will show its page. Installed
// with the hook, kept the same as the widget's own copy on every start while the hook is in, and
// removed with the hook. Only the skill's own files are written or removed.
use std::path::{Path, PathBuf};

const FILES: [(&str, &[u8]); 6] = [
    (
        "SKILL.md",
        include_bytes!("../../../skill/meownitor-round/SKILL.md"),
    ),
    (
        "reference.md",
        include_bytes!("../../../skill/meownitor-round/reference.md"),
    ),
    (
        "round-kit.css",
        include_bytes!("../../../skill/meownitor-round/round-kit.css"),
    ),
    (
        "round-kit.js",
        include_bytes!("../../../skill/meownitor-round/round-kit.js"),
    ),
    (
        "template-choice.html",
        include_bytes!("../../../skill/meownitor-round/template-choice.html"),
    ),
    (
        "template-review.html",
        include_bytes!("../../../skill/meownitor-round/template-review.html"),
    ),
];

fn folder() -> Option<PathBuf> {
    crate::sessions::home().map(|h| h.join(".claude").join("skills").join("meownitor-round"))
}

/// Writes every file that is missing or differs; true when something was written.
fn install_in(dir: &Path) -> std::io::Result<bool> {
    std::fs::create_dir_all(dir)?;
    let mut wrote = false;
    for (name, bytes) in FILES {
        let p = dir.join(name);
        if std::fs::read(&p).ok().as_deref() != Some(bytes) {
            std::fs::write(&p, bytes)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

fn remove_in(dir: &Path) {
    for (name, _) in FILES {
        let _ = std::fs::remove_file(dir.join(name));
    }
    // the folder goes only when nothing of the user's is left in it
    let _ = std::fs::remove_dir(dir);
}

pub fn install() -> Result<bool, String> {
    let dir = folder().ok_or("no home folder")?;
    install_in(&dir).map_err(|e| format!("cannot install the skill: {e}"))
}

pub fn remove() {
    if let Some(dir) = folder() {
        remove_in(&dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_updates_and_removes_only_its_files() {
        let dir = std::env::temp_dir().join(format!("cw-skill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(install_in(&dir).unwrap());
        assert!(
            !install_in(&dir).unwrap(),
            "nothing to write the second time"
        );
        std::fs::write(dir.join("SKILL.md"), "old").unwrap();
        assert!(install_in(&dir).unwrap());
        assert_eq!(std::fs::read(dir.join("SKILL.md")).unwrap(), FILES[0].1);
        std::fs::write(dir.join("notes.txt"), "mine").unwrap();
        remove_in(&dir);
        assert!(dir.join("notes.txt").exists() && !dir.join("SKILL.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
