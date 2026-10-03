// SPDX-License-Identifier: MIT

//! Atomic writes for persisted settings, profiles, and editor documents.
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
    write_atomic_bytes(path, contents.as_bytes())
}

/// Stages binary or text content without truncating the destination. Each
/// writer owns a unique temporary file; an existing sibling is never followed
/// or overwritten. Sync the staged contents before replacing the destination.
pub fn write_atomic_bytes(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let (tmp, mut file) = loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".tmp-{}-{id}", std::process::id()));
        let tmp = path.with_file_name(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    };
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        // Close before rename, including on Windows.
        drop(file);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("harmonicon-atomic-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writing_replaces_an_existing_file_whole() {
        let dir = test_dir("replace");
        let path = dir.join("chart.harpchart");
        std::fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_replacement_preserves_destination_and_removes_staging_file() {
        let dir = test_dir("failure");
        let path = dir.join("document");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("keep"), "original").unwrap();
        assert!(write_atomic(&path, "new").is_err());
        assert_eq!(std::fs::read_to_string(path.join("keep")).unwrap(), "original");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn binary_content_is_preserved() {
        let dir = test_dir("binary");
        let path = dir.join("music.wav");
        let bytes = [0, 255, 128, 1];
        write_atomic_bytes(&path, &bytes).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_writers_use_independent_staging_files() {
        let dir = test_dir("concurrent");
        let path = dir.join("settings.json");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let writers: Vec<_> = (0..4)
            .map(|n| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    write_atomic(&path, &n.to_string()).unwrap();
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let content = std::fs::read_to_string(path).unwrap();
        assert!(["0", "1", "2", "3"].contains(&content.as_str()));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
