// SPDX-License-Identifier: MIT

//! Turning any [`ScoreFile`] track into a playable [`HarpChart`].
//!
//! This is where a file that knows nothing about harmonicas becomes one:
//! every pitch is resolved onto a hole, breath and technique by
//! `harmonicon_core::pitch_map`, the same resolver the Song Editor's MIDI
//! import and live recording already use. One mapper, so an imported chart
//! can't disagree with an authored one about what a harp can play.
//!
//! **A converted chart is only as playable as the part it came from.** A
//! guitar line on a harmonica is mostly notes out of reach; `pick_harmonica_
//! track` exists to steer callers away from that, and [`ConversionReport`]
//! says plainly how much of what came out is actually reachable, so a caller
//! can refuse rather than hand the player an unplayable chart.

use harmonicon_core::chart::{
    Difficulty, HarpChart, Metadata, Modifier, NoteEvent, Scoring, Song, TempoPoint, Timing,
    TrackItem,
};
use harmonicon_core::harmonica::Harmonica;
use harmonicon_core::midi::midi_to_note;
use harmonicon_core::pitch_map::{
    HarpKind, HoleAssignment, Technique, harp_for_key, map_pitch_playable, suggest_key,
};

use crate::{ScoreError, ScoreFile, ScoreTrack};

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
        let (chart, report) = to_chart(score, track.index, &harp, artist)?;
        converted.push(TrackConversion {
            track: track.clone(),
            chart,
            report,
        });
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
    let playable = || {
        tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.report.is_worth_playing())
    };

    let named = crate::pick_harmonica_track(
        &tracks
            .iter()
            .map(|t| t.track.clone())
            .collect::<Vec<ScoreTrack>>(),
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
    let chromatic = harp_for_key(
        suggest_key(pitches, HarpKind::Chromatic),
        HarpKind::Chromatic,
    );
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
    let hit = pitches
        .iter()
        .filter(|&&p| map_pitch_playable(p, harp).is_some())
        .count();
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

/// Converts one track onto `harp`.
pub fn to_chart(
    score: &dyn ScoreFile,
    track: usize,
    harp: &Harmonica,
    artist: &str,
) -> Result<(HarpChart, ConversionReport), ScoreError> {
    let notes = score.notes(track)?;
    let tempo = score.tempo_bpm().max(1.0);
    let (numerator, denominator) = score.time_signature();

    let mut report = ConversionReport::default();
    let mut items = Vec::new();

    for note in &notes {
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
                vec![Modifier::Bend {
                    semitones: -depth,
                    intensity: None,
                }]
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

        items.push(TrackItem {
            id: None,
            time: Some(note.start_secs),
            tick: None,
            duration: note.duration_secs,
            phrase: None,
            groove: None,
            play_mode: None,
            call: false,
            events: vec![NoteEvent {
                hole: assignment.hole,
                action: assignment.action,
                note: Some(chart_note(note.midi, &assignment, harp)),
                modifiers: (!modifiers.is_empty()).then_some(modifiers),
            }],
        });
    }

    let chart = HarpChart {
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
            tempo_bpm: tempo,
            key: harmonicon_core::harmonica::detected_harp_key(harp)
                .unwrap_or_else(|| "C".to_string()),
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
            tempo_map: vec![TempoPoint {
                tick: 0,
                bpm: tempo,
            }],
            time_signature_map: None,
        },
        harmonica: harp.clone(),
        track: items,
        loop_section: None,
        // The same windows every bundled chart uses.
        scoring: Scoring {
            perfect_window_ms: 50,
            good_window_ms: 100,
            miss_window_ms: 130,
            combo: None,
            style_bonus: None,
        },
    };

    Ok((chart, report))
}

#[cfg(test)]
mod tests;
