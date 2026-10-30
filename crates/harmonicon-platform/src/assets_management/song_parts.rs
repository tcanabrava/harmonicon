// SPDX-License-Identifier: MIT

//! Filesystem discovery shared by the runtime catalog and the bundled manifest.

use std::path::{Path, PathBuf};

/// All native parts, sorted by filename. A score file is only a fallback:
/// a MIDI alongside a native chart is backing music, not another part.
pub fn charts_in(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut files: Vec<_> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| !path.file_name().unwrap_or_default().to_string_lossy().starts_with('.'))
        .collect();
    files.sort();
    let charts: Vec<_> = files
        .iter()
        .filter(|path| {
            path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("harpchart"))
        })
        .cloned()
        .collect();
    if !charts.is_empty() {
        return charts;
    }
    files.into_iter().find(|path| harmonicon_score::is_importable_file(path)).into_iter().collect()
}

/// Authors name parts with their filenames; single-chart song names stay unchanged.
pub fn part_name(song: &str, chart: &Path, count: usize) -> String {
    if count > 1 {
        format!("{song} — {}", chart.file_stem().unwrap_or_default().to_string_lossy())
    } else {
        song.to_owned()
    }
}

/// What the song picker shows about one chart, read without loading or
/// validating it: its genre and difficulty, and the instrument it's for
/// (`harmonica` for a chart with no `instrument` field, as every chart
/// before format 1.8.0 is).
pub struct CatalogFields {
    pub genre: String,
    pub difficulty: String,
    pub instrument: String,
}

pub fn catalog_fields(chart: &Path) -> CatalogFields {
    let value = std::fs::read(chart)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let song_field = |field: &str| {
        value
            .as_ref()
            .and_then(|v| v.get("song")?.get(field)?.as_str())
            .unwrap_or("Uncategorized")
            .to_owned()
    };
    let instrument = value
        .as_ref()
        .and_then(|v| v.get("instrument")?.get("type")?.as_str())
        .unwrap_or("harmonica")
        .to_owned();
    CatalogFields { genre: song_field("genre"), difficulty: song_field("difficulty"), instrument }
}
