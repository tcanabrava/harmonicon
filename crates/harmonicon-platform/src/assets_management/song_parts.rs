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
