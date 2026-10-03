// SPDX-License-Identifier: MIT

//! Persistent song exclusions, scoped to their asset source and song folder.

use crate::assets_management::AvailableSongs;
use bevy::prelude::*;
use std::{collections::BTreeSet, io, path::Path};

#[derive(Resource)]
pub struct SongLibrary {
    hidden: BTreeSet<String>,
}

/// Chart filenames can change on update; the source and song folder identify
/// a catalog entry. A second repository's identical song remains independent.
pub fn song_identity(asset_path: &str) -> &str {
    asset_path.rsplit_once("/song/").map_or(asset_path, |(folder, _)| folder)
}

impl Default for SongLibrary {
    fn default() -> Self {
        let hidden = crate::paths::config_file("hidden-songs.json")
            .and_then(|path| match std::fs::read(&path) {
                Ok(bytes) => match serde_json::from_slice(&bytes) {
                    Ok(hidden) => Some(hidden),
                    Err(error) => {
                        warn!("Could not read hidden songs: {error}");
                        None
                    }
                },
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => {
                    warn!("Could not read hidden songs: {error}");
                    None
                }
            })
            .unwrap_or_default();
        Self { hidden }
    }
}

impl SongLibrary {
    pub fn filter(&self, songs: &mut AvailableSongs) {
        songs.0.retain(|_, entries| {
            entries.retain(|song| !self.hidden.contains(song_identity(&song.asset_path)));
            !entries.is_empty()
        });
    }

    pub fn hide(&mut self, asset_path: &str) -> io::Result<()> {
        let path = crate::paths::config_file("hidden-songs.json")
            .ok_or_else(|| io::Error::other("no writable config directory"))?;
        self.hide_at(asset_path, &path)
    }

    fn hide_at(&mut self, asset_path: &str, path: &Path) -> io::Result<()> {
        let mut hidden = self.hidden.clone();
        hidden.insert(song_identity(asset_path).to_owned());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&hidden)?)?;
        std::fs::rename(&temporary, path)?;
        self.hidden = hidden;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusions_survive_reload_and_chart_renames_but_do_not_cross_repositories() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("hidden.json");
        let mut library = SongLibrary { hidden: BTreeSet::new() };
        library.hide_at("packs://first/Band/Song/song/old.harpchart", &path).unwrap();
        let hidden: BTreeSet<String> =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(hidden.contains(song_identity("packs://first/Band/Song/song/new.harpchart")));
        assert!(!hidden.contains(song_identity("packs://second/Band/Song/song/old.harpchart")));
        let mut library = SongLibrary { hidden };
        library.hide_at("packs://second/Band/Other/song/chart.harpchart", &path).unwrap();
        assert_eq!(library.hidden.len(), 2);
        let entry = |source: &str| crate::assets_management::SongEntry {
            artist: "Band".into(),
            name: "Song".into(),
            genre: "Rock".into(),
            difficulty: "easy".into(),
            source_name: source.into(),
            retained: false,
            asset_path: format!("packs://{source}/Band/Song/song/new.harpchart"),
        };
        let mut songs = AvailableSongs::default();
        for _ in 0..2 {
            // Simulates a fresh catalog scan after restart or repository update.
            songs.0.insert("Band".into(), vec![entry("first"), entry("second")]);
            library.filter(&mut songs);
            assert_eq!(songs.0["Band"].len(), 1);
            assert_eq!(songs.0["Band"][0].source_name, "second");
        }
        assert!(library.hide_at("another", &path.join("invalid")).is_err());
        assert_eq!(library.hidden.len(), 2);
    }
}
