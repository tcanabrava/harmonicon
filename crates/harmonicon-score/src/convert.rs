// SPDX-License-Identifier: MIT

//! Turning any [`ScoreFile`] track into a playable [`HarpChart`] for an
//! [`Instrument`].
//!
//! On a harmonica every pitch is resolved onto a hole, breath and technique
//! by `harmonicon_core::pitch_map`, the same resolver the Song Editor's MIDI
//! import and live recording already use. One mapper, so an imported chart
//! can't disagree with an authored one about what a harp can play. On a
//! guitar the whole part is placed at once so the hand stays in position
//! (`guitar::fingering`), keeping a tab's own string and fret wherever they
//! still sound the note.
//!
//! **A converted chart is only as playable as the part it came from.** A
//! guitar line on a harmonica is mostly notes out of reach; `pick_harmonica_
//! track` exists to steer callers away from that, and [`ConversionReport`]
//! says plainly how much of what came out is actually reachable, so a caller
//! can refuse rather than hand the player an unplayable chart.

use harmonicon_core::chart::{
    ChartInstrument, Difficulty, HarpChart, Metadata, Modifier, NoteEvent, Scoring, Song,
    TempoPoint, Timing, TrackItem,
};
use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::instrument::Instrument;
use harmonicon_core::instrument::flute::Flute;
use harmonicon_core::instrument::guitar::fingering::{Onset, fingering};
use harmonicon_core::instrument::guitar::{
    FretAssignment, Guitar, GuitarBody, STANDARD_TUNING, StringTechnique,
};
use harmonicon_core::midi::midi_to_note;
use harmonicon_core::pitch_map::{
    HarpKind, HoleAssignment, Technique, harp_for_key, map_pitch_playable, suggest_key,
};

use crate::{ScoreError, ScoreFile, ScoreNote, ScoreTrack};

/// How well a track survived being put on a harmonica.
///
/// Reported rather than silently absorbed: converting a part written for
/// another instrument routinely loses notes, and the honest answer to
/// "this file has no harmonica in it" is to say so, not to emit a chart
/// nobody can play.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ConversionReport {
    pub total: usize,
    /// Notes landing on a plain blow or draw reed.
    pub natural: usize,
    pub bends: usize,
    pub overblows: usize,
    /// Notes the harmonica cannot produce at all. These are dropped.
    pub unreachable: usize,
}

impl ConversionReport {
    /// Fraction of the source track that made it onto the harp.
    pub fn reachable_fraction(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.total - self.unreachable) as f32 / self.total as f32
    }

    /// Whether this is worth offering to a player at all.
    ///
    /// The threshold is a judgement, not a measurement: below it the result
    /// reads as a broken chart rather than a hard one. A caller wanting a
    /// different line can use [`Self::reachable_fraction`] directly.
    pub fn is_worth_playing(&self) -> bool {
        self.total > 0 && self.reachable_fraction() >= MIN_REACHABLE
    }
}

/// Fraction of a part's notes that must land on the harp before it counts
/// as playable at all.
pub const MIN_REACHABLE: f32 = 0.8;

/// One track of a source file, converted onto its own best-fitting harp.
#[derive(Debug, Clone)]
pub struct TrackConversion {
    pub track: ScoreTrack,
    pub chart: HarpChart,
    pub report: ConversionReport,
}

/// Converts *every* playable track, each onto the harmonica that suits it.
///
/// Not just the one a caller expects to use: a file written for a band
/// names no harmonica part, so whichever track gets chosen is a guess, and
/// the alternatives are what let a player correct it without a reload.
/// Conversion is arithmetic over notes already in memory — cheap next to
/// the read that produced them.
///
/// Each part gets its own harp because the right key for a melody is rarely
/// the right one for a bass line, and a shared harp would make every part
/// but one look unplayable in the picker.
pub fn convert_all_tracks(
    score: &dyn ScoreFile,
    artist: &str,
) -> Result<Vec<TrackConversion>, ScoreError> {
    let mut converted = Vec::new();
    for track in score.tracks().iter().filter(|t| t.is_playable()) {
        let notes = score.notes(track.index)?;
        let harp = suggested_harp(&notes.iter().map(|n| n.midi).collect::<Vec<_>>());
        let (chart, report) = to_chart(score, track.index, &harp.into(), artist)?;
        converted.push(TrackConversion { track: track.clone(), chart, report });
    }
    Ok(converted)
}

/// Which converted track to open on, or `None` if none is playable.
///
/// A track the file *names* as the harmonica wins over a better-scoring
/// one — the file is telling us, and a few extra bends there means the part
/// is harder, not wrong. It does **not** win over being unplayable: a bass
/// line named "Harmonica" converts to a chart with no notes in it at all,
/// and opening a song on that is worse than opening on a part that works.
///
/// Failing a name, it's whichever part survives the harmonica best. That
/// beats "the busiest track", which is routinely a guitar, and is only safe
/// because it is a *default*: the picker makes it correctable, which is
/// what allows offering an ambiguous file at all instead of refusing it.
pub fn choose_track(tracks: &[TrackConversion]) -> Option<usize> {
    let playable = || tracks.iter().enumerate().filter(|(_, t)| t.report.is_worth_playing());

    let named = crate::pick_harmonica_track(
        &tracks.iter().map(|t| t.track.clone()).collect::<Vec<ScoreTrack>>(),
    );
    if let Some(index) =
        named.and_then(|i| playable().find(|(_, t)| t.track.index == i).map(|(n, _)| n))
    {
        return Some(index);
    }
    playable()
        .max_by(|(ai, a), (bi, b)| {
            a.report
                .reachable_fraction()
                .total_cmp(&b.report.reachable_fraction())
                // Ties go to the part needing fewer bends (easier), then to
                // the one with more notes (a lead part rather than a
                // six-chord comp), then to the earlier track. Spelled out
                // to the last step on purpose: `max_by` keeps the *last*
                // maximum, so an unbroken tie would otherwise resolve by
                // position with nothing saying so.
                .then_with(|| b.report.bends.cmp(&a.report.bends))
                .then_with(|| a.report.total.cmp(&b.report.total))
                .then_with(|| bi.cmp(ai))
        })
        .map(|(index, _)| index)
}

/// The harmonica a set of pitches fits best.
///
/// Tries diatonic first and only prefers a chromatic when it genuinely fits
/// better, so a tune a C diatonic plays cleanly doesn't hand a beginner a
/// 12-hole chromatic.
///
/// **The second branch is rarely taken**, because both instruments are
/// fully chromatic across their own range: a diatonic through its bends and
/// overblows, a chromatic through its slide. So neither dominates on note
/// coverage and the diatonic-first preference decides almost every time.
/// What can still swing it is *range* — a part sitting outside the best
/// diatonic's span but inside a chromatic's.
pub fn suggested_harp(pitches: &[u8]) -> Harmonica {
    let diatonic = harp_for_key(suggest_key(pitches, HarpKind::Diatonic), HarpKind::Diatonic);
    if reachable(pitches, &diatonic) >= MIN_REACHABLE {
        return diatonic;
    }
    let chromatic = harp_for_key(suggest_key(pitches, HarpKind::Chromatic), HarpKind::Chromatic);
    if reachable(pitches, &chromatic) > reachable(pitches, &diatonic) {
        chromatic
    } else {
        diatonic
    }
}

/// Fraction of `pitches` this harp can actually produce.
fn reachable(pitches: &[u8], harp: &Harmonica) -> f32 {
    if pitches.is_empty() {
        return 0.0;
    }
    let hit = pitches.iter().filter(|&&p| map_pitch_playable(p, harp).is_some()).count();
    hit as f32 / pitches.len() as f32
}

/// What a chart event's `note` field must say for this assignment.
///
/// **A bend is the exception, and getting it wrong is silent.** Everywhere
/// else `note` is the pitch that sounds — including an overblow's or a
/// slide's, neither of which a blow/draw lookup could reproduce. But both
/// readers of the field, `gameplay::notes::target_pitch` and
/// `harp_remap::source_pitch`, *add the bend modifier to it*, so a bent
/// note must state the **unbent reed** and let the modifier do the moving.
/// Every hand-authored chart in this repository already does (`note: "B4"`
/// with `semitones: -1`); the converter wrote the sounding pitch instead,
/// which made an imported song's bent notes judge a semitone or two below
/// the music and asked the player to over-bend every one of them.
fn chart_note(sounding: u8, assignment: &HoleAssignment, harp: &Harmonica) -> String {
    match assignment.technique {
        Technique::Bend(_) => harp.wind_direction_label(assignment.hole, &assignment.action),
        _ => midi_to_note(sounding as i32),
    }
}

/// Ticks per beat the generated chart uses.
///
/// Matches `harmonicon_core::synth::TICKS_PER_BEAT` — 12, divisible by both
/// 4 and 3 so straight sixteenths and triplets are both representable. A
/// chart carries its own `timing.resolution`, so this only has to be
/// self-consistent, but agreeing with the editor's grid means an imported
/// chart opens there without rescaling.
pub const TICKS_PER_BEAT: u32 = harmonicon_core::synth::TICKS_PER_BEAT as u32;

/// Converts one track onto `instrument`.
pub fn to_chart(
    score: &dyn ScoreFile,
    track: usize,
    instrument: &Instrument,
    artist: &str,
) -> Result<(HarpChart, ConversionReport), ScoreError> {
    let notes = score.notes(track)?;
    let (items, report, key) = match instrument {
        Instrument::Harmonica(harp) => {
            let (items, report) = harmonica_items(&notes, harp);
            (items, report, detected_harmonica_key(harp))
        }
        Instrument::Fretted(guitar) => {
            let (items, report) = guitar_items(&notes, guitar);
            (items, report, likely_key(&notes))
        }
        Instrument::Flute(flute) => {
            let (items, report) = flute_items(&notes, flute);
            (items, report, likely_key(&notes))
        }
        Instrument::Piano(piano) => {
            let (items, report) = piano_items(&notes, piano);
            (items, report, likely_key(&notes))
        }
    };
    let mut chart = chart_shell(score, artist, key, items);
    match instrument {
        Instrument::Harmonica(harp) => chart.harmonica = Some(harp.clone()),
        Instrument::Fretted(guitar) => {
            chart.instrument_spec = Some(ChartInstrument::from_guitar(guitar));
        }
        Instrument::Flute(flute) => {
            chart.instrument_spec = Some(ChartInstrument::from_flute(flute));
        }
        Instrument::Piano(piano) => {
            chart.instrument_spec = Some(ChartInstrument::from_piano(piano));
        }
    }
    Ok((chart, report))
}

fn detected_harmonica_key(harp: &Harmonica) -> String {
    harmonicon_core::harmonica::detected_harp_key(harp).unwrap_or_else(|| "C".to_string())
}

/// A guess at a part's key from nothing but its notes: the pitch class it
/// sounds most, by time held. A guitar has no key of its own to read one off
/// the way a harmonica does, and a wrong-but-plausible key is what the chart
/// author would correct anyway.
fn likely_key(notes: &[ScoreNote]) -> String {
    let mut weight = [0.0f64; 12];
    for note in notes {
        weight[usize::from(note.midi % 12)] += note.duration_secs.max(0.01);
    }
    let tonic = (0..12).max_by(|&a, &b| weight[a].total_cmp(&weight[b]).then(b.cmp(&a)));
    harmonicon_core::midi::NOTE_NAMES[tonic.unwrap_or(0)].to_string()
}

/// One track item holding one event.
fn item(note: &ScoreNote, event: NoteEvent) -> TrackItem {
    TrackItem {
        id: None,
        time: Some(note.start_secs),
        tick: None,
        duration: note.duration_secs,
        phrase: None,
        groove: None,
        chord: None,
        play_mode: None,
        call: false,
        lyric: None,
        events: vec![event],
    }
}

fn harmonica_items(notes: &[ScoreNote], harp: &Harmonica) -> (Vec<TrackItem>, ConversionReport) {
    let mut report = ConversionReport::default();
    let mut items = Vec::new();
    for note in notes {
        report.total += 1;
        // The strict resolver, not the always-resolves one: an importer
        // wanting every note to land *somewhere* is right for authoring,
        // where a human then fixes it, and wrong here, where the nearest
        // playable note would silently rewrite the tune.
        let Some(assignment) = map_pitch_playable(note.midi, harp) else {
            report.unreachable += 1;
            continue;
        };

        let modifiers = match assignment.technique {
            Technique::Natural => {
                report.natural += 1;
                Vec::new()
            }
            Technique::Bend(depth) => {
                report.bends += 1;
                // Charts store a bend as a negative (downward) offset.
                vec![Modifier::Bend { semitones: -depth, intensity: None }]
            }
            Technique::Overblow => {
                report.overblows += 1;
                vec![Modifier::Overblow]
            }
            Technique::Overdraw => {
                report.overblows += 1;
                vec![Modifier::Overdraw]
            }
            Technique::Slide => {
                report.natural += 1;
                vec![Modifier::Slide]
            }
        };

        items.push(item(
            note,
            NoteEvent {
                note: Some(chart_note(note.midi, &assignment, harp)),
                modifiers: (!modifiers.is_empty()).then_some(modifiers),
                ..NoteEvent::hole(assignment.hole, assignment.action)
            },
        ));
    }
    (items, report)
}

/// Places every note of a part on `guitar` at once, so the hand stays in
/// position (`guitar::fingering`). Notes starting together form one onset
/// and are voiced as a chord. A tab's own string and fret are kept wherever
/// they still sound the note; a note the neck can't reach, or a chord no
/// hand can hold, is reported unreachable.
fn guitar_items(notes: &[ScoreNote], guitar: &Guitar) -> (Vec<TrackItem>, ConversionReport) {
    // `notes` is sorted by start; group equal starts.
    let mut groups: Vec<Vec<&ScoreNote>> = Vec::new();
    for note in notes {
        match groups.last_mut() {
            Some(group) if (group[0].start_secs - note.start_secs).abs() < 1e-6 => group.push(note),
            _ => groups.push(vec![note]),
        }
    }
    let onsets: Vec<Onset> = groups
        .iter()
        .map(|group| Onset {
            notes: group
                .iter()
                .map(|n| {
                    let authored = n.fret.map(|f| FretAssignment {
                        string: f.string,
                        fret: f.fret,
                        technique: StringTechnique::Fretted,
                    });
                    (n.midi, authored)
                })
                .collect(),
        })
        .collect();
    let placed = fingering(guitar, &onsets);

    let mut report = ConversionReport::default();
    let mut items = Vec::new();
    for (group, voicing) in groups.iter().zip(placed) {
        report.total += group.len();
        let Some(voicing) = voicing else {
            report.unreachable += group.len();
            continue;
        };
        report.natural += group.len();
        for (note, position) in group.iter().zip(voicing) {
            let name = midi_to_note(i32::from(note.midi));
            items.push(item(note, NoteEvent::fretted(position.string, position.fret, name)));
        }
    }
    (items, report)
}

/// A track as a flute plays it: one note at a time, on each note's standard
/// fingering. Of notes starting together only the highest is kept (a
/// melody sits on top), and a note still sounding when the next starts is
/// cut short there, since the flute has one voice to give it. A note
/// outside the flute's range is dropped and counted, never moved an
/// octave: that would be a different tune.
fn flute_items(notes: &[ScoreNote], flute: &Flute) -> (Vec<TrackItem>, ConversionReport) {
    let mut report = ConversionReport { total: notes.len(), ..Default::default() };
    let mut line: Vec<ScoreNote> = Vec::new();
    for note in notes {
        match line.last_mut() {
            Some(last) if (last.start_secs - note.start_secs).abs() < 1e-6 => {
                if note.midi > last.midi {
                    *last = *note;
                }
                report.unreachable += 1;
            }
            _ => line.push(*note),
        }
    }
    let mut items = Vec::new();
    for (i, note) in line.iter().enumerate() {
        if flute.fingerings_for(note.midi).is_empty() {
            report.unreachable += 1;
            continue;
        }
        report.natural += 1;
        let mut note = *note;
        if let Some(next) = line.get(i + 1) {
            note.duration_secs = note.duration_secs.min(next.start_secs - note.start_secs);
        }
        items.push(item(&note, NoteEvent::fingered(midi_to_note(i32::from(note.midi)), 0)));
    }
    (items, report)
}

/// A piano plays every note the keyboard has, chords included, each where
/// the score has it; a note off the keyboard is the only one lost.
fn piano_items(
    notes: &[ScoreNote],
    piano: &harmonicon_core::instrument::piano::Piano,
) -> (Vec<TrackItem>, ConversionReport) {
    let mut report = ConversionReport { total: notes.len(), ..Default::default() };
    let mut items = Vec::new();
    for note in notes {
        if !piano.has(note.midi) {
            report.unreachable += 1;
            continue;
        }
        report.natural += 1;
        items.push(item(note, NoteEvent::keyed(midi_to_note(i32::from(note.midi)))));
    }
    (items, report)
}

/// The guitar a track is written for: its own tuning when the source
/// recorded one (a tab does), standard tuning otherwise. Imported guitars
/// get 24 frets, enough for anything tab writes; the source's own fret
/// count isn't read yet.
pub fn suggested_guitar(track: &ScoreTrack) -> Guitar {
    Guitar {
        kind: harmonicon_core::instrument::fretted_strings::FrettedKind::Guitar,
        strings: track.tuning.clone().unwrap_or_else(|| STANDARD_TUNING.to_vec()),
        frets: 24,
        capo: 0,
        body: GuitarBody::Electric,
    }
}

/// Everything a converted chart has besides its instrument and notes.
fn chart_shell(
    score: &dyn ScoreFile,
    artist: &str,
    key: String,
    track: Vec<TrackItem>,
) -> HarpChart {
    // Into the range the chart schema allows (`tempo_map[].bpm`, 20–300): a
    // file can state anything, and a MIDI tempo of 0 µs per quarter is an
    // infinite BPM. Notes keep their own seconds, so this moves none of them.
    let tempo = score.tempo_bpm().clamp(20.0, 300.0);
    let (numerator, denominator) = score.time_signature();
    HarpChart {
        metadata: Some(Metadata {
            format_version: Some(harmonicon_core::chart::CURRENT_FORMAT_VERSION.to_string()),
            author: None,
            source: Some(format!("imported from {}", score.format().label())),
            license: None,
            description: None,
        }),
        song: Song {
            title: score.title().unwrap_or("Imported").to_string(),
            artist: artist.to_string(),
            genre: "Uncategorized".to_string(),
            tempo_bpm: tempo,
            key,
            // Imported material has no difficulty rating of its own, and
            // guessing one from note density would be a worse lie than a
            // neutral default the author can change.
            difficulty: Difficulty::Intermediate,
            time_signature: Some(format!("{numerator}/{denominator}")),
            feel: None,
        },
        timing: Timing {
            resolution: TICKS_PER_BEAT,
            // One point: every note already carries absolute seconds, so
            // the map is metadata for the metronome rather than the
            // timebase. Carrying a source file's full tempo automation
            // through is worth doing later; it changes nothing about when
            // notes land.
            tempo_map: vec![TempoPoint { tick: 0, bpm: tempo }],
            time_signature_map: None,
            pickup_ticks: None,
            repeats: Vec::new(),
        },
        harmonica: None,
        instrument_spec: None,
        track,
        loop_section: None,
        // The same windows every bundled chart uses.
        scoring: Scoring {
            perfect_window_ms: 50,
            good_window_ms: 100,
            miss_window_ms: 130,
            combo: None,
            style_bonus: None,
        },
    }
}

#[cfg(test)]
mod tests;
