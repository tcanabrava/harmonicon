// SPDX-License-Identifier: MIT

//! Crash-safe writes for the two JSON files the game keeps in the user's
//! config directory (`settings.rs`, `profile.rs`).
//!
//! Writing straight over the live file leaves a window where it is
//! truncated but not yet rewritten: a crash or a power loss there costs
//! the player every preference and every per-song record, since both
//! loaders fall back to defaults on a file they can't parse. Writing a
//! sibling temp file and renaming it over the original closes that
//! window. A rename within one directory is atomic on every platform the
//! game ships to, so a reader sees either the whole old file or the whole
//! new one.

use std::path::Path;

/// Writes `contents` to `path` via a temp file in the same directory.
/// The temp file has to be a sibling, not somewhere under `/tmp`: a
/// rename is only atomic within a single filesystem, and the config
/// directory is routinely on a different one.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let tmp = temp_path(path);
    std::fs::write(&tmp, contents)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            // Leaving the temp file behind would have the next save fail
            // the same way with no hint of why.
            let _ = std::fs::remove_file(&tmp);
            Err(err)
        }
    }
}

/// The sibling temp file [`write_atomic`] stages through. Kept pure so the
/// "same directory" property is testable without touching a disk.
fn temp_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_temp_file_is_a_sibling_of_the_target() {
        let target = PathBuf::from("/home/someone/.config/harmonicon/settings.json");
        let tmp = temp_path(&target);
        assert_eq!(tmp.parent(), target.parent());
        assert_eq!(tmp.file_name().and_then(|n| n.to_str()), Some("settings.json.tmp"));
    }

    #[test]
    fn writing_replaces_an_existing_file_whole() {
        let dir = std::env::temp_dir().join("harmonicon-config-file-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "old").unwrap();

        write_atomic(&path, "new").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(!temp_path(&path).exists(), "the temp file must not survive a successful write");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
