// SPDX-License-Identifier: MIT

//! Playing a score file directly, without authoring a chart first.
//!
//! A player is far likelier to own a MIDI file, a Guitar Pro tab or a
//! MuseScore score than a `.harpchart`. This makes one droppable into
//! `~/Harmonicon/songs/<artist>/<song>/song/` and playable, by converting it
//! to a chart at asset-load time through `harmonicon_score`.
//!
//! **Nothing here names a file format.** The extensions come from
//! `harmonicon_score::IMPORT_EXTENSIONS`, the reader from
//! `harmonicon_score::parse_import`, and everything downstream works through
//! the `ScoreFile` trait — so a new format is a module in that crate and
//! touches nothing in this one. That is the whole reason the trait exists;
//! a loader that said `MidiScore::parse` would have made every future format
//! a copy of this file.
//!
//! What genuinely belongs here, and stays: the `AssetLoader` wiring, and
//! reading the artist/title off the folder layout — neither is something a
//! score file knows about.
//!
//! **`song/music.mid` already meant something else** — the backing audio for
//! a `.harpchart` song, rendered per-track by `loader::load_midi_tracks`.
//! Both readings of the same extension are legitimate, so discovery
//! (`assets_management::scan_all_songs`) prefers a `.harpchart` and only
//! treats a score file as the chart when there is no chart beside it.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;
use harmonicon_score::convert::{TrackConversion, choose_track, convert_all_tracks};
use harmonicon_score::{IMPORT_EXTENSIONS, parse_import};

use super::SongManifest;
use super::loader::{SongLoadError, assemble_manifest};

#[derive(Default, TypePath)]
pub struct ScoreSongLoader;

impl AssetLoader for ScoreSongLoader {
    type Asset = SongManifest;
    type Settings = ();
    type Error = SongLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<SongManifest, SongLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let path = load_context.path().path();
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        let converted = convert_score(
            &extension,
            bytes,
            &artist_from_path(path),
            title_from_path(path),
        )?;
        let chart = converted.tracks[converted.selected].chart.clone();
        assemble_manifest(
            chart,
            converted.tracks,
            Some(converted.selected),
            load_context,
        )
        .await
    }

    /// Exactly what `harmonicon_score` can read — never a hand-written list,
    /// which would let the two drift and advertise a format that then fails
    /// to load.
    fn extensions(&self) -> &[&str] {
        IMPORT_EXTENSIONS
    }
}

/// Every playable part of a score file, converted, plus which to open on.
#[derive(Debug)]
pub struct ConvertedScore {
    pub tracks: Vec<TrackConversion>,
    /// Index into [`tracks`](Self::tracks) — always valid, since a file with
    /// nothing playable is rejected rather than returned empty.
    pub selected: usize,
}

/// Reads and converts a score file of any supported format.
///
/// `fallback_title` is used only when the format carries no title of its
/// own — MIDI doesn't (see `harmonicon_score::midi`'s `title`), while Guitar
/// Pro, MuseScore and MusicXML all do — so a format that knows its own name
/// keeps it.
pub fn convert_score(
    extension: &str,
    bytes: Vec<u8>,
    artist: &str,
    fallback_title: Option<String>,
) -> Result<ConvertedScore, SongLoadError> {
    let score = parse_import(extension, bytes).map_err(validation)?;
    let mut tracks = convert_all_tracks(&*score, artist).map_err(validation)?;

    if let Some(title) = score.title().map(str::to_string).or(fallback_title) {
        for track in &mut tracks {
            track.chart.song.title = title.clone();
        }
    }

    let selected = choose_track(&tracks).ok_or_else(|| {
        // Reached only when no part survives the harp — a file with no notes
        // at all was already refused while parsing.
        let best = tracks
            .iter()
            .map(|t| t.report.reachable_fraction())
            .fold(0.0f32, f32::max);
        SongLoadError::Validation(format!(
            "no track in this file is playable on a harmonica — the best manages only \
             {percent}% of its notes. Name the harmonica part \"Harmonica\" if there is one.",
            percent = (best * 100.0).round() as u32,
        ))
    })?;
    Ok(ConvertedScore { tracks, selected })
}

fn validation(e: harmonicon_score::ScoreError) -> SongLoadError {
    SongLoadError::Validation(e.to_string())
}

/// The artist folder a song sits in — `songs/<artist>/<song>/song/x.mid`.
///
/// The file itself carries no artist, and the folder layout already encodes
/// one, so reading it back is better than writing "Unknown" into every
/// imported chart.
fn artist_from_path(path: &std::path::Path) -> String {
    path.ancestors()
        .nth(3)
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported".to_string())
}

/// The song folder's name — `songs/<artist>/<song>/song/x.mid`.
///
/// A fallback, not an override: a format that records a real title keeps it
/// (see [`convert_score`]). Left as `None` when the layout doesn't match.
fn title_from_path(path: &std::path::Path) -> Option<String> {
    path.ancestors()
        .nth(2)
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
