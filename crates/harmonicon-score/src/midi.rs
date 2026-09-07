// SPDX-License-Identifier: MIT

//! MIDI files, behind [`ScoreFile`].
//!
//! Thin on purpose: `harmonicon_core::midi_file` already parses tempo maps,
//! pairs note-on/note-off and reads track names, because the Song Editor's
//! import needed all of it first. This adapts that to the trait rather than
//! reimplementing it — the same parser serves authoring and playing, so a
//! fix to one is a fix to both.

use harmonicon_core::midi_file;
use midly::Smf;

use crate::{ScoreError, ScoreFile, ScoreFormat, ScoreNote, ScoreTrack};

/// A parsed MIDI file.
///
/// Holds the raw bytes and re-parses per query rather than storing an
/// `Smf`, which borrows from them — the same reason the Song Editor's
/// `MidiImport` does. Owning a self-referential parse would need lifetime
/// bookkeeping across frames for no gain; these files are small.
pub struct MidiScore {
    bytes: Vec<u8>,
    tracks: Vec<ScoreTrack>,
    tempo_bpm: f32,
    time_signature: (u8, u8),
}

impl MidiScore {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ScoreError> {
        let smf = Smf::parse(&bytes).map_err(|e| ScoreError::Parse {
            format: "MIDI",
            detail: e.to_string(),
        })?;

        let tracks: Vec<ScoreTrack> = smf
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| ScoreTrack {
                index,
                name: midi_file::track_name_of(track),
                note_count: midi_file::note_on_count(track),
            })
            .collect();

        if !tracks.iter().any(ScoreTrack::is_playable) {
            return Err(ScoreError::NoPlayableTracks);
        }

        // The first tempo event, which is what the file opens at. A full
        // tempo map is already folded into each note's own seconds below,
        // so this is metadata rather than timing.
        let tempo_bpm = midi_file::collect_tempo_map(&smf)
            .first()
            .map(|&(_, micros_per_quarter)| 60_000_000.0 / micros_per_quarter as f32)
            .unwrap_or(120.0);

        let time_signature = time_signature_of(&smf).unwrap_or((4, 4));

        Ok(Self {
            bytes,
            tracks,
            tempo_bpm,
            time_signature,
        })
    }
}

/// The file's first time signature, if it declares one.
///
/// MIDI stores the denominator as a power of two (`3` meaning /8), which is
/// the kind of detail worth converting once here rather than at each call
/// site.
fn time_signature_of(smf: &Smf) -> Option<(u8, u8)> {
    for track in &smf.tracks {
        for event in track {
            if let midly::TrackEventKind::Meta(midly::MetaMessage::TimeSignature(
                numerator,
                denominator_pow2,
                _,
                _,
            )) = event.kind
            {
                return Some((numerator, 1u8.checked_shl(denominator_pow2 as u32)?));
            }
        }
    }
    None
}

impl ScoreFile for MidiScore {
    fn format(&self) -> ScoreFormat {
        ScoreFormat::Midi
    }

    fn title(&self) -> Option<&str> {
        // Always `None`, deliberately. MIDI's convention is that the title
        // is the *first track's* name — but a harmonica file's first track
        // is named "Harmonica", which is an instrument, not a song. Passing
        // that off as a title produced exactly that: a song called
        // "Harmonica", observed in the running game. Reporting honestly
        // that the format carries no title lets each caller fall back to
        // something it actually knows (the folder name), instead of every
        // caller having to un-do the same wrong answer.
        None
    }

    fn tracks(&self) -> &[ScoreTrack] {
        &self.tracks
    }

    fn notes(&self, track: usize) -> Result<Vec<ScoreNote>, ScoreError> {
        let smf = Smf::parse(&self.bytes).map_err(|e| ScoreError::Parse {
            format: "MIDI",
            detail: e.to_string(),
        })?;
        let events = smf
            .tracks
            .get(track)
            .ok_or(ScoreError::NoSuchTrack(track))?;

        let tpq = midi_file::ticks_per_quarter(&smf).map_err(|detail| ScoreError::Parse {
            format: "MIDI",
            detail,
        })?;
        let tempo = midi_file::collect_tempo_map(&smf);

        let mut notes: Vec<ScoreNote> = midi_file::extract_notes(events)
            .into_iter()
            .map(|raw| {
                // Both ends go through the tempo map rather than scaling a
                // tick duration by one BPM: a tempo change inside a note
                // would otherwise give it the wrong length.
                let start = midi_file::tick_to_seconds(raw.start_tick, tpq, &tempo);
                let end = midi_file::tick_to_seconds(raw.start_tick + raw.dur_ticks, tpq, &tempo);
                ScoreNote {
                    start_secs: start,
                    duration_secs: (end - start).max(0.0),
                    midi: raw.key,
                }
            })
            .collect();
        // `extract_notes` already orders by start tick; re-sorting is cheap
        // insurance, since the trait promises sorted output and a future
        // reader might not.
        notes.sort_by(|a, b| {
            a.start_secs
                .partial_cmp(&b.start_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(notes)
    }

    fn tempo_bpm(&self) -> f32 {
        self.tempo_bpm
    }

    fn time_signature(&self) -> (u8, u8) {
        self.time_signature
    }
}

#[cfg(test)]
mod tests;
