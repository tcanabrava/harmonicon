// SPDX-License-Identifier: MIT

//! Preserve song folders removed by a repository update, within that pack.

use std::{collections::BTreeSet, fs, io, path::Path};

pub const INDEX: &str = ".harmonicon-retained-songs.json";

pub fn read_index(root: &Path) -> BTreeSet<String> {
    fs::read(root.join(INDEX))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn has_chart(folder: &Path) -> io::Result<bool> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                || error.kind() == io::ErrorKind::NotADirectory =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "harpchart"
                            | "mid"
                            | "midi"
                            | "gp3"
                            | "gp4"
                            | "gp5"
                            | "gpx"
                            | "gp"
                            | "mscz"
                            | "musicxml"
                            | "xml"
                            | "mxl"
                    )
                })
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn song_folders(root: &Path) -> io::Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    if !root.exists() {
        return Ok(result);
    }
    for artist in fs::read_dir(root)? {
        let artist = artist?;
        if !artist.file_type()?.is_dir() || artist.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        for song in fs::read_dir(artist.path())? {
            let song = song?;
            if song.file_type()?.is_dir()
                && !song.file_name().to_string_lossy().starts_with('.')
                && has_chart(&song.path().join("song"))?
            {
                result.insert(format!(
                    "{}/{}",
                    artist.file_name().to_string_lossy(),
                    song.file_name().to_string_lossy()
                ));
            }
        }
    }
    Ok(result)
}

fn copy_folder(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::other("cannot preserve a symbolic link"));
        }
        if kind.is_dir() {
            copy_folder(&entry.path(), &to.join(entry.file_name()))?;
        } else if kind.is_file() {
            fs::copy(entry.path(), to.join(entry.file_name()))?;
        } else {
            return Err(io::Error::other("cannot preserve a special file"));
        }
    }
    Ok(())
}

/// Called before the atomic checkout swap. Each update recomputes the index:
/// returning upstream songs use the new repository copy and lose the warning.
pub fn preserve_removed(installed: &Path, staging: &Path) -> io::Result<()> {
    let previous = song_folders(installed)?;
    let upstream = song_folders(staging)?;
    let removed: BTreeSet<_> = previous.difference(&upstream).cloned().collect();
    for folder in &removed {
        // Do not merge into a partial/restructured upstream folder: retain
        // the complete playable copy rather than accidentally mixing versions.
        let target = staging.join(folder);
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        copy_folder(&installed.join(folder), &target)?;
    }
    fs::write(staging.join(INDEX), serde_json::to_vec_pretty(&removed)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn song(root: &Path, name: &str, bytes: &str) {
        fs::create_dir_all(root.join(name).join("song")).unwrap();
        fs::write(root.join(name).join("song/chart.harpchart"), bytes).unwrap();
        fs::write(root.join(name).join("backing.ogg"), bytes).unwrap();
    }
    #[test]
    fn removed_songs_survive_repeated_updates_and_returning_songs_use_upstream() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old");
        let next = temp.path().join("next");
        song(&old, "Band/Removed", "old");
        song(&old, "Band/Changed", "old");
        song(&next, "Band/Changed", "new");
        preserve_removed(&old, &next).unwrap();
        assert_eq!(read_index(&next), BTreeSet::from(["Band/Removed".into()]));
        assert_eq!(
            fs::read_to_string(next.join("Band/Removed/backing.ogg")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_to_string(next.join("Band/Changed/backing.ogg")).unwrap(),
            "new"
        );
        let later = temp.path().join("later");
        fs::create_dir_all(&later).unwrap();
        preserve_removed(&next, &later).unwrap();
        assert!(read_index(&later).contains("Band/Removed"));
        let returned = temp.path().join("returned");
        song(&returned, "Band/Removed", "returned");
        preserve_removed(&later, &returned).unwrap();
        assert!(!read_index(&returned).contains("Band/Removed"));
        assert_eq!(
            fs::read_to_string(returned.join("Band/Removed/backing.ogg")).unwrap(),
            "returned"
        );
    }
    #[test]
    fn removing_only_the_chart_preserves_the_complete_song_folder() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old");
        let next = temp.path().join("next");
        song(&old, "Band/Song", "old");
        song(&next, "Band/Song", "partial");
        fs::remove_file(next.join("Band/Song/song/chart.harpchart")).unwrap();
        preserve_removed(&old, &next).unwrap();
        assert_eq!(
            fs::read_to_string(next.join("Band/Song/backing.ogg")).unwrap(),
            "old"
        );
        assert!(next.join("Band/Song/song/chart.harpchart").exists());
        assert!(read_index(&next).contains("Band/Song"));
    }

    #[test]
    fn repositories_do_not_share_retained_songs() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        song(&first, "Band/Song", "first");
        let second = temp.path().join("second");
        fs::create_dir_all(&second).unwrap();
        preserve_removed(&temp.path().join("other"), &second).unwrap();
        assert!(!second.join("Band/Song").exists());
    }
}
