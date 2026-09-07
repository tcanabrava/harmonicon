// SPDX-License-Identifier: MIT

//! Guitar Pro, MuseScore and MusicXML, behind [`ScoreFile`].
//!
//! One adapter for all three because the `guitarpro` crate funnels every one
//! of them into the same `Song` model — a tab is measures of beats of notes
//! whatever container it arrived in. Only the first step differs, which is
//! why [`GpScore::parse`] is a short dispatch and everything after it is
//! shared.
//!
//! **Notes are fretted, not pitched.** A tab records "third fret of the
//! fourth string"; the sounding pitch is that fret plus the string's tuning,
//! which is per-track and can be anything (drop D, a bass, a seven-string).
//! Resolving it here is what lets [`crate::convert`] treat this like any
//! other source of pitches.
//!
//! **Timing has to be reconstructed, not read.** Only the `gp3`/`gp4`/`gp5`
//! readers fill in `Beat::start`; the GP6/GP7 importer — which the MuseScore
//! and MusicXML paths also pass through — leaves it empty and states
//! durations instead. Trusting that field alone yielded *no notes at all*
//! for four of the seven formats here, silently, as an empty song. So
//! positions are accumulated per measure ([`GpScore::placed_beats`]) and a
//! stated `start` is used only when there is one. Measure positions come the
//! same way, added up from the time signatures ([`measure_starts`]), because
//! `MeasureHeader::start` is unreliable for exactly the same reason.
//!
//! Lengths are then clamped to the next onset in the track, so a tuplet or a
//! dotted figure can't overrun the note after it — which also means the
//! adapter never has to be certain about tuplet arithmetic.

use guitarpro::Song as GpSong;
use guitarpro::model::legacy::key_signature::Duration as GpDuration;
use guitarpro::{Beat, NoteType, Track as GpTrack};

use crate::{ScoreError, ScoreFile, ScoreFormat, ScoreNote, ScoreTrack};

/// Ticks per quarter note in the `guitarpro` model
/// (`DURATION_QUARTER_TIME`). Restated rather than imported because the
/// crate does not export it.
const TICKS_PER_QUARTER: f64 = 960.0;

/// Tempo used when a file states none. Guitar Pro's own default.
const DEFAULT_BPM: f64 = 120.0;

pub struct GpScore {
    song: GpSong,
    format: ScoreFormat,
    tracks: Vec<ScoreTrack>,
    /// `(tick_from_start, bpm)`, sorted, always non-empty and starting at
    /// tick 0 so a lookup can never fall off the front. Positions are
    /// relative to [`origin`](Self::origin), like everything
    /// [`GpScore::seconds_at`] is asked about.
    tempo_map: Vec<(i64, f64)>,
    /// Absolute tick position of each measure, by header index — see
    /// [`measure_starts`].
    measure_starts: Vec<i64>,
    /// Tick position the piece starts at. Guitar Pro numbers measures from
    /// one quarter note in, not from zero, so this is subtracted from every
    /// position — without it every song would begin one beat late.
    origin: i64,
    time_signature: (u8, u8),
}

impl GpScore {
    /// Reads any format the `guitarpro` crate understands.
    ///
    /// The extension has already been matched by [`crate::parse_import`];
    /// an unknown one here is a programming error rather than a bad file,
    /// so it reports [`ScoreError::UnsupportedFormat`] the same way.
    pub fn parse(extension: &str, bytes: Vec<u8>) -> Result<Self, ScoreError> {
        let mut song = GpSong::default();
        let format = match extension {
            "gp3" => {
                song.read_gp3(&bytes).map_err(|e| parse_error("gp3", e))?;
                ScoreFormat::GuitarPro
            }
            "gp4" => {
                song.read_gp4(&bytes).map_err(|e| parse_error("gp4", e))?;
                ScoreFormat::GuitarPro
            }
            "gp5" => {
                song.read_gp5(&bytes).map_err(|e| parse_error("gp5", e))?;
                ScoreFormat::GuitarPro
            }
            "gpx" => {
                song.read_gpx(&bytes).map_err(|e| parse_error("gpx", e))?;
                ScoreFormat::GuitarPro
            }
            "gp" => {
                song.read_gp(&bytes).map_err(|e| parse_error("gp", e))?;
                ScoreFormat::GuitarPro
            }
            "mscz" => {
                song = mscz_song(&bytes)?;
                ScoreFormat::MuseScore
            }
            "musicxml" | "xml" => {
                song = musicxml_song(&bytes)?;
                ScoreFormat::MusicXml
            }
            other => return Err(ScoreError::UnsupportedFormat(other.to_string())),
        };
        Ok(Self::from_song(song, format))
    }

    fn from_song(song: GpSong, format: ScoreFormat) -> Self {
        let origin = song
            .measure_headers
            .iter()
            .map(|h| h.start)
            .min()
            .unwrap_or(0);
        let measure_starts = measure_starts(&song, origin);
        let tempo_map = build_tempo_map(&song, &measure_starts, origin);
        let time_signature = song
            .measure_headers
            .first()
            .map(|h| {
                (
                    h.time_signature.numerator.max(1) as u8,
                    denominator_of(&h.time_signature.denominator),
                )
            })
            .unwrap_or((4, 4));

        let tracks = song
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| ScoreTrack {
                index,
                name: (!track.name.trim().is_empty()).then(|| track.name.trim().to_string()),
                note_count: pitched_note_count(track),
            })
            .collect();

        Self {
            song,
            format,
            tracks,
            tempo_map,
            measure_starts,
            origin,
            time_signature,
        }
    }

    /// Seconds from the start of the piece for an absolute tick position.
    ///
    /// Walks the tempo map rather than dividing by one BPM: a tab that
    /// slows for a bridge would otherwise drift further out of time with
    /// every bar after it.
    fn seconds_at(&self, tick: i64) -> f64 {
        let tick = (tick - self.origin).max(0);
        let mut seconds = 0.0;
        for (index, &(start, bpm)) in self.tempo_map.iter().enumerate() {
            if tick <= start {
                break;
            }
            let segment_end = self
                .tempo_map
                .get(index + 1)
                .map(|&(next, _)| next)
                .unwrap_or(i64::MAX);
            let end = segment_end.min(tick);
            seconds += ticks_to_seconds(end - start, bpm);
            if end >= tick {
                break;
            }
        }
        seconds
    }

    /// Every beat of a track, placed at an absolute tick.
    ///
    /// **A beat's own position cannot be relied on.** The `gp3`/`gp4`/`gp5`
    /// readers fill `Beat::start`, but the GP6/GP7 importer — which the
    /// MuseScore and MusicXML paths also go through — builds beats from
    /// durations and leaves it empty. Reading only `start` returned *no
    /// notes at all* for those four formats. So position is accumulated
    /// from each measure's own beginning, and a stated `start` is used when
    /// there is one.
    fn placed_beats<'a>(&self, track: &'a GpTrack) -> Vec<(i64, f64, &'a Beat)> {
        let mut placed = Vec::new();
        let mut fallback_start = self.origin;
        for measure in &track.measures {
            let base = self
                .measure_starts
                .get(measure.header_index)
                .copied()
                .unwrap_or(fallback_start);
            let mut measure_end = base;
            for voice in &measure.voices {
                // Each voice restarts at the bar line — two voices are
                // simultaneous parts, not one after the other.
                let mut cursor = base;
                for beat in &voice.beats {
                    let start = beat.start.unwrap_or(cursor);
                    let length = beat_ticks(&beat.duration);
                    placed.push((start, length, beat));
                    cursor = start + length.round() as i64;
                }
                measure_end = measure_end.max(cursor);
            }
            fallback_start = measure_end;
        }
        placed
    }
}

impl ScoreFile for GpScore {
    fn format(&self) -> ScoreFormat {
        self.format
    }

    fn title(&self) -> Option<&str> {
        // Unlike MIDI, these formats have a real title field — so it is
        // used, and only falls back to the folder name when genuinely
        // blank. See `harmonicon_song::song::score_song`.
        let name = self.song.name.trim();
        (!name.is_empty()).then_some(name)
    }

    fn tracks(&self) -> &[ScoreTrack] {
        &self.tracks
    }

    fn notes(&self, track: usize) -> Result<Vec<ScoreNote>, ScoreError> {
        let source = self
            .song
            .tracks
            .get(track)
            .ok_or(ScoreError::NoSuchTrack(track))?;
        let placed = self.placed_beats(source);

        // Every onset in the track, so a length can be clamped to whatever
        // comes next — including a rest, which occupies time and sounds
        // nothing.
        let mut onsets: Vec<i64> = placed.iter().map(|&(start, _, _)| start).collect();
        onsets.sort_unstable();
        onsets.dedup();

        let mut notes = Vec::new();
        for &(start, written, beat) in &placed {
            let next = onsets
                .iter()
                .copied()
                .find(|&t| t > start)
                .map(|t| (t - start) as f64)
                .unwrap_or(written);
            let length = written.min(next).max(1.0);
            let start_secs = self.seconds_at(start);
            let duration_secs = self.seconds_at(start + length.round() as i64) - start_secs;
            for note in &beat.notes {
                let Some(midi) = pitch_of(note, source) else {
                    continue;
                };
                notes.push(ScoreNote {
                    start_secs,
                    duration_secs: duration_secs.max(0.01),
                    midi,
                });
            }
        }
        notes.sort_by(|a, b| {
            a.start_secs
                .partial_cmp(&b.start_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(notes)
    }

    fn tempo_bpm(&self) -> f32 {
        self.tempo_map
            .first()
            .map(|&(_, bpm)| bpm)
            .unwrap_or(DEFAULT_BPM) as f32
    }

    fn time_signature(&self) -> (u8, u8) {
        self.time_signature
    }
}

/// A note's sounding pitch, or `None` when it doesn't have one.
///
/// Only `Normal` notes sound: a `Tie` continues the note before it (counting
/// it again would double every tied pair), a `Rest` is silence, and a `Dead`
/// note is a muted click with no pitch at all.
fn pitch_of(note: &guitarpro::Note, track: &GpTrack) -> Option<u8> {
    if note.kind != NoteType::Normal {
        return None;
    }
    // `string` is 1-based, and `strings` holds `(number, tuning)` — the
    // open-string MIDI pitch that the fret number counts up from.
    let tuning = track.strings.get(note.string.checked_sub(1)? as usize)?.1;
    let midi = i32::from(note.value) + i32::from(tuning);
    (0..=127).contains(&midi).then_some(midi as u8)
}

/// How many notes in this track would actually produce a pitch.
///
/// A percussion track reports zero however many notes it has: its "frets"
/// are drum-kit indices, and reading them as pitches yields a plausible-
/// looking but meaningless part that could win the track picker outright.
fn pitched_note_count(track: &GpTrack) -> usize {
    if track.percussion_track {
        return 0;
    }
    track
        .measures
        .iter()
        .flat_map(|m| &m.voices)
        .flat_map(|v| &v.beats)
        .flat_map(|b| &b.notes)
        .filter(|n| pitch_of(n, track).is_some())
        .count()
}

/// A beat's written length in ticks.
///
/// `Duration::value` is the note value's denominator (4 = quarter), so a
/// whole note is four quarters and everything else divides down from there.
/// Tuplets scale by `times/enters` — three-in-the-time-of-two makes each
/// note *shorter*. (The crate's own private helper has that ratio the other
/// way up; this only ever narrows a length, and every result is clamped to
/// the next onset regardless, so a disagreement can't push a note past its
/// neighbour.)
fn beat_ticks(duration: &GpDuration) -> f64 {
    let value = f64::from(duration.value.max(1));
    let mut ticks = TICKS_PER_QUARTER * 4.0 / value;
    if duration.dotted {
        ticks *= 1.5;
    } else if duration.double_dotted {
        ticks *= 1.75;
    }
    if duration.tuplet_enters > 0 && duration.tuplet_times > 0 {
        ticks = ticks * f64::from(duration.tuplet_times) / f64::from(duration.tuplet_enters);
    }
    ticks
}

fn ticks_to_seconds(ticks: i64, bpm: f64) -> f64 {
    (ticks as f64 / TICKS_PER_QUARTER) * 60.0 / bpm.max(1.0)
}

/// `(start_tick, bpm)` for every tempo in force, in order.
///
/// A measure header carries `0` to mean "unchanged", so the previous tempo
/// is carried forward; the first entry is pinned to the origin so
/// [`GpScore::seconds_at`] always has something to start from.
fn build_tempo_map(song: &GpSong, measure_starts: &[i64], origin: i64) -> Vec<(i64, f64)> {
    let mut current = if song.tempo > 0 {
        f64::from(song.tempo)
    } else {
        DEFAULT_BPM
    };
    let mut map = vec![(0, current)];
    for (index, header) in song.measure_headers.iter().enumerate() {
        if header.tempo > 0 && f64::from(header.tempo) != current {
            current = f64::from(header.tempo);
            // Keyed off the computed measure position, not `header.start`,
            // for the same reason beats are: the GP6/GP7 importer leaves
            // that field at its default on every header.
            let at = measure_starts.get(index).copied().unwrap_or(origin) - origin;
            map.push((at.max(0), current));
        }
    }
    map.sort_by_key(|&(tick, _)| tick);
    map.dedup_by_key(|&mut (tick, _)| tick);
    map
}

/// Absolute tick position of each measure, derived from the time signatures
/// rather than read from the file.
///
/// `MeasureHeader::start` is only filled in by the legacy binary readers;
/// everything arriving through GP6/GP7 keeps the default on every header, so
/// trusting it would stack the whole piece on one instant. Adding up bar
/// lengths needs nothing but the meter, which every format states.
fn measure_starts(song: &GpSong, origin: i64) -> Vec<i64> {
    let whole = TICKS_PER_QUARTER * 4.0;
    let mut starts = Vec::with_capacity(song.measure_headers.len());
    let mut at = origin;
    for header in &song.measure_headers {
        starts.push(at);
        let numerator = f64::from(header.time_signature.numerator.max(1));
        let denominator = f64::from(denominator_of(&header.time_signature.denominator).max(1));
        at += (whole * numerator / denominator).round() as i64;
    }
    starts
}

/// The lower number of a time signature, from the note value naming it.
fn denominator_of(duration: &GpDuration) -> u8 {
    let value = duration.value.max(1);
    u8::try_from(value).unwrap_or(4)
}

fn parse_error(format: &'static str, error: impl std::fmt::Display) -> ScoreError {
    ScoreError::Parse {
        format,
        detail: error.to_string(),
    }
}

/// MuseScore: unzip, parse the `.mscx` inside, then reuse the crate's own
/// conversion into the shared `Song` model.
fn mscz_song(bytes: &[u8]) -> Result<GpSong, ScoreError> {
    let file = guitarpro::read_mscz_bytes(bytes).map_err(|e| parse_error("mscz", e))?;
    let outcome = guitarpro::convert::mscz::to_optimized::mscx_to_loaded_score(&file.mscx);
    Ok(guitarpro::convert::legacy::loaded_score_to_legacy_song(
        &outcome.score,
    ))
}

/// MusicXML: the crate models `score-partwise` and can convert it, but has
/// no reader, so the XML is deserialized here.
fn musicxml_song(bytes: &[u8]) -> Result<GpSong, ScoreError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| parse_error("musicxml", format!("not valid UTF-8: {e}")))?;
    let score: guitarpro::model::musicxml::ScorePartwise =
        quick_xml::de::from_str(text).map_err(|e| parse_error("musicxml", e))?;
    Ok(guitarpro::convert::guitarpro::musicxml_to_legacy_song(
        &score,
    ))
}

#[cfg(test)]
mod tests;
