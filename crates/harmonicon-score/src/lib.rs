// SPDX-License-Identifier: MIT

//! Reading a playable score out of whatever file the player has.
//!
//! Harmonicon's own `.harpchart` is one format among several: a player is far
//! more likely to own a MIDI file or a Guitar Pro tab than a chart authored
//! here. [`ScoreFile`] is the single door all of them come through.
//!
//! **The native format implements the trait too.** That is deliberate: a
//! trait with one real implementation and one special case drifts, because
//! nothing forces the special case to keep fitting. Making `.harpchart` go
//! through the same door means the shape is exercised by the format we
//! control.
//!
//! Bevy-free, like `harmonicon-core` below it. Loading bytes through the
//! `AssetServer` is `harmonicon-song`'s job, a level up; this crate turns
//! bytes into notes.
//!
//! What a format has to supply is deliberately small — notes in seconds, a
//! tempo, a time signature, and a list of tracks. Everything harmonica-
//! specific (which hole, which breath, which technique) is *derived* by
//! [`convert`], using `harmonicon_core::pitch_map`, rather than being asked
//! of a format that knows nothing about harmonicas.

pub mod convert;
pub mod guitar_pro;
pub mod harpchart;
pub mod midi;
pub mod track;

pub use track::{HARMONICA_TRACK_NAMES, pick_harmonica_track};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScoreError {
    #[error("not valid {format}: {detail}")]
    Parse {
        format: &'static str,
        detail: String,
    },
    #[error("this file has no tracks with any notes in it")]
    NoPlayableTracks,
    #[error("track {0} does not exist in this file")]
    NoSuchTrack(usize),
    #[error("no reader for a .{0} file")]
    UnsupportedFormat(String),
}

/// The file extensions [`parse_import`] understands — an asset loader's
/// `extensions()` should be exactly this, so registering a format and
/// advertising it can't drift apart.
///
/// **`harpchart` is deliberately absent.** It is already a chart: playing
/// one goes through `harmonicon_song`'s own loader, which validates,
/// migrates and version-checks it. [`harpchart::HarpChartScore`] exists so
/// a chart can be a *source* — re-derived onto a different harmonica —
/// not a second, weaker way to play one.
pub const IMPORT_EXTENSIONS: &[&str] = &[
    "mid", "midi", // MIDI
    "gp3", "gp4", "gp5", // Guitar Pro's binary formats
    "gpx", "gp",   // Guitar Pro 6 and 7, both zipped containers
    "mscz", // MuseScore
    "musicxml", "xml", // MusicXML, the interchange format everything exports
];

/// Reads `bytes` as whatever `extension` says they are.
///
/// **This is the only place a concrete reader is named.** Everything above
/// this crate works through [`ScoreFile`], so adding a format is a module,
/// one arm here, and an entry in [`IMPORT_EXTENSIONS`] — no loader, no
/// converter and no menu code learns that the format exists. A trait whose
/// implementations are still selected by name at every call site would buy
/// nothing, which is the mistake this function exists to prevent.
pub fn parse_import(extension: &str, bytes: Vec<u8>) -> Result<Box<dyn ScoreFile>, ScoreError> {
    match extension.to_ascii_lowercase().as_str() {
        "mid" | "midi" => Ok(Box::new(midi::MidiScore::parse(bytes)?)),
        gp @ ("gp3" | "gp4" | "gp5" | "gpx" | "gp" | "mscz" | "musicxml" | "xml") => {
            Ok(Box::new(guitar_pro::GpScore::parse(gp, bytes)?))
        }
        other => Err(ScoreError::UnsupportedFormat(other.to_string())),
    }
}

/// Which file format a score came from — for messages and for deciding
/// whether a track picker is worth showing at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScoreFormat {
    HarpChart,
    Midi,
    GuitarPro,
    MuseScore,
    MusicXml,
}

impl ScoreFormat {
    pub fn label(self) -> &'static str {
        match self {
            ScoreFormat::HarpChart => "Harmonicon chart",
            ScoreFormat::Midi => "MIDI",
            ScoreFormat::GuitarPro => "Guitar Pro",
            ScoreFormat::MuseScore => "MuseScore",
            ScoreFormat::MusicXml => "MusicXML",
        }
    }
}

/// One playable part within a file.
///
/// `name` is what makes automatic track selection possible at all — see
/// [`pick_harmonica_track`]. It's optional because plenty of MIDI files
/// never name their tracks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScoreTrack {
    pub index: usize,
    pub name: Option<String>,
    /// Notes in this track. Zero means it carries only tempo or metadata —
    /// common for a MIDI file's first track — and such tracks are worth
    /// hiding from a picker rather than offering as a choice that plays
    /// silence.
    pub note_count: usize,
}

impl ScoreTrack {
    pub fn is_playable(&self) -> bool {
        self.note_count > 0
    }
}

/// One note, in absolute seconds from the start of the piece.
///
/// Seconds rather than ticks because ticks are meaningless without their
/// file's own resolution and tempo map, and every format spells those
/// differently. Resolving to time in the reader keeps that variety from
/// leaking into everything downstream.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScoreNote {
    pub start_secs: f64,
    pub duration_secs: f64,
    pub midi: u8,
}

/// A score file, whatever format it arrived in.
pub trait ScoreFile {
    fn format(&self) -> ScoreFormat;

    /// The piece's title, when the format records one.
    fn title(&self) -> Option<&str>;

    /// Every track, including unplayable ones — a picker decides what to
    /// show, and hiding them here would make "track 3" ambiguous between
    /// the file's numbering and ours.
    fn tracks(&self) -> &[ScoreTrack];

    /// One track's notes, sorted by start time.
    fn notes(&self, track: usize) -> Result<Vec<ScoreNote>, ScoreError>;

    /// The nominal tempo. A real tempo map is already baked into
    /// [`ScoreNote::start_secs`]; this is for the chart's own metadata and
    /// for the metronome.
    fn tempo_bpm(&self) -> f32;

    fn time_signature(&self) -> (u8, u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_advertised_extension_has_a_reader() {
        // The drift this catches: advertising an extension on a loader that
        // then refuses every file with it, which reads to a player as "the
        // game says it supports this and doesn't."
        for extension in IMPORT_EXTENSIONS {
            let err = parse_import(extension, b"not a real file".to_vec())
                .err()
                .expect("garbage bytes parsed as a valid score");
            assert!(
                !matches!(err, ScoreError::UnsupportedFormat(_)),
                ".{extension} is advertised but has no reader"
            );
        }
    }

    #[test]
    fn an_unknown_extension_is_refused_by_name() {
        // Sibelius, which nothing here reads.
        let err = parse_import("sib", Vec::new()).err().unwrap();
        assert!(matches!(err, ScoreError::UnsupportedFormat(ext) if ext == "sib"));
    }

    #[test]
    fn an_extension_is_matched_case_insensitively() {
        // Real files arrive as .MID as often as .mid.
        let err = parse_import("MID", b"not a real file".to_vec())
            .err()
            .unwrap();
        assert!(!matches!(err, ScoreError::UnsupportedFormat(_)));
    }
}
