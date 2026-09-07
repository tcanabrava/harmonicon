// SPDX-License-Identifier: MIT

//! Generated Jam Session backing: a synthesized 12-bar bass line, in a
//! genre-selectable rhythmic shape ([`Genre`]), for any key/tempo/
//! progression, so Jam Session doesn't require picking an existing song.
//! See `PLAN.md`'s "Backing track variety" entry.
//!
//! Deliberately not the harmonica-timbre synth `song_editor::playback`
//! shares with `gameplay::call_response` — a backing bass is a different
//! instrument, and reusing harmonica partials here would risk sounding like
//! a second harmonica part to echo instead of backing to play over.

use std::f32::consts::TAU;
use std::path::PathBuf;

use bevy::audio::AudioSource;
use bevy::prelude::*;

use harmonicon_audio::waveform::{WAVEFORM_BUCKETS, bucket_peaks};
use harmonicon_core::chart::{
    Action, Difficulty, Feel, HarpChart, Metadata, NoteEvent, Scoring, Song, TempoPoint, Timing,
    TrackItem,
};
use harmonicon_core::harmonica::{Position, Progression, progression_bars, richter_harp, semitone};
use harmonicon_core::midi::{midi_to_freq_hz, note_to_midi};
use harmonicon_core::wav::encode_wav;
use harmonicon_song::song::{NoteCube3dConfig, NoteThemeConfig, SongManifest};

pub const SAMPLE_RATE: u32 = 44_100;

/// How many 12-bar choruses to render into one generated backing loop —
/// long enough for a real practice session (a few minutes) without an
/// unreasonably large buffer/asset. `JamLoop` (the existing player toggle)
/// still works normally once this runs out.
pub const CHORUSES: u32 = 8;

const ATTACK_SECS: f32 = 0.01;
const RELEASE_SECS: f32 = 0.05;
/// Fraction of each note's own slot left as silence before the next bass
/// note, so consecutive notes don't blur into one continuous tone.
const NOTE_GAP_FRAC: f32 = 0.08;

/// Which rhythmic/groove character a generated jam's bass line uses —
/// selectable on the "Generate Jam" config page alongside `Progression`/
/// `Position`. Deliberately its own axis rather than a `Progression`
/// variant: `Progression` only changes which chord *roots* play over the
/// 12-bar form, but the thing that actually makes a genre sound like that
/// genre is almost entirely rhythm/groove, not chord choice — see
/// [`genre_pattern`].
///
/// A player can freely combine any `Genre` with any `Progression` (e.g.
/// `Rock` rhythm over the `JazzBlues` changes) — the two are independent
/// choices, not a fixed pairing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Genre {
    #[default]
    Blues,
    Jazz,
    Rock,
    Reggae,
    Country,
}

impl Genre {
    /// Every selectable genre, in the order the "Generate Jam" combobox
    /// offers them.
    pub fn all() -> &'static [Genre] {
        &[
            Genre::Blues,
            Genre::Jazz,
            Genre::Rock,
            Genre::Reggae,
            Genre::Country,
        ]
    }

    /// Display label for the picker.
    pub fn label(self) -> &'static str {
        match self {
            Genre::Blues => "Blues",
            Genre::Jazz => "Jazz",
            Genre::Rock => "Rock",
            Genre::Reggae => "Reggae",
            Genre::Country => "Country",
        }
    }

    /// Inverse of [`label`](Self::label) — same `all().find(...)` pattern
    /// as `Progression::from_label`/`Position::from_label`/`Scale::from_label`.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::all().iter().copied().find(|g| g.label() == label)
    }

    /// The straight/shuffle metronome feel this genre implies, seeded into
    /// a generated chart's `Song::feel` (see `generated_chart`) — picked up
    /// for free by `gameplay::metronome_overlay::feel_from_chart`, the same
    /// mechanism a hand-authored chart's own `feel` field already drives.
    fn metronome_feel(self) -> Feel {
        match self {
            Genre::Blues | Genre::Jazz => Feel::Shuffle,
            Genre::Rock | Genre::Reggae | Genre::Country => Feel::Straight,
        }
    }
}

/// The genre picked for the current generated jam, kept around after
/// `build_generated_manifest` bakes it into the audio/chart so a live
/// gameplay-side system (`jam::rhythm_guide`) can still read it — unlike
/// `Genre` itself, which is otherwise only ever a plain function parameter.
/// Lives here (not `app::`, alongside `JamProgression`/`JamScale`) because
/// `Genre` lives in this same `jam::` layer already; `app::` is reserved
/// for wrapping types from *lower* layers (`song::`) that a higher one
/// like `jam::` reads — putting `Genre` there would mean `app::` importing
/// from `jam::`, inverting the "dependencies point downward" rule `jam`
/// itself already relies on (`jam::session` imports `harmonicon_app::app::
/// JamProgression`/`JamScale`, not the other way around).
///
/// Set on Start by `menu::pages::jam_generate` (alongside `JamProgression`/
/// `JamScale`) and reset to the default (`Genre::Blues`) by the real-song
/// "Jam Session" button (`menu::pages::jam_session`) — a real song has no
/// genre concept attached to it; `jam::rhythm_guide`'s widget only ever
/// spawns for a `GeneratedJamSession` regardless, so this reset is just
/// defense against a stale value lingering, not load-bearing on its own.
#[derive(Resource, Default)]
pub struct JamGenre(pub Genre);

/// Semitone offsets of the classic 12-bar "blues box" bass shape, relative
/// to whatever chord root is sounding: root, root, 5th, 5th, flat-7th,
/// flat-7th, 5th, flat-7th — 8 slots per bar. Quality-agnostic like every
/// pattern below: root/5th/flat-7th are shared between a dominant-7th and
/// minor-7th chord (`song::harmonica::chord_intervals`) — only the 3rd
/// differs, and none of these patterns ever play one.
const BLUES_PATTERN: [Option<i32>; 8] = [
    Some(0),
    Some(0),
    Some(7),
    Some(7),
    Some(10),
    Some(10),
    Some(7),
    Some(10),
];
/// Walking-quarter-note contour: root, 5th, flat-7th, 5th, one note per
/// beat (the off-beat slots rest) — swung like the classic blues shape,
/// just sparser.
const JAZZ_PATTERN: [Option<i32>; 8] =
    [Some(0), None, Some(7), None, Some(10), None, Some(7), None];
/// Straight, driving root pulse with a 5th lift in the second half of the
/// bar — a simplified "power chord" bass line, no swing.
const ROCK_PATTERN: [Option<i32>; 8] = [
    Some(0),
    Some(0),
    Some(0),
    Some(0),
    Some(7),
    Some(7),
    Some(0),
    Some(0),
];
/// The classic reggae "skank": silence on every downbeat, a hit on every
/// off-beat. This is a deliberate simplification, not a transcription of
/// real reggae form (which usually isn't a 12-bar blues at all) — the
/// genre character here comes entirely from this off-beat rhythm sitting
/// on top of the same 12-bar practice-loop scaffold every other genre
/// uses, not from an authentic reggae chord progression.
const REGGAE_PATTERN: [Option<i32>; 8] =
    [None, Some(0), None, Some(7), None, Some(10), None, Some(7)];
/// Straight "boom-chick": root on the beat, 5th on the off-beat, no swing.
const COUNTRY_PATTERN: [Option<i32>; 8] =
    [Some(0), None, Some(7), None, Some(0), None, Some(7), None];

/// This genre's per-bar note pattern (see the `*_PATTERN` constants above)
/// and whether its eighth-note pairs swing 2:1 or play straight/even.
/// `pub(crate)` so `jam::rhythm_guide` can drive its live harmonica-attack
/// pulse row from the exact same rhythmic skeleton the bass audio uses —
/// one shared "what does this genre's groove look like" source, two
/// different renderings (audio synthesis here, a visual guide there).
pub(crate) fn genre_pattern(genre: Genre) -> (&'static [Option<i32>; 8], bool) {
    match genre {
        Genre::Blues => (&BLUES_PATTERN, true),
        Genre::Jazz => (&JAZZ_PATTERN, true),
        Genre::Rock => (&ROCK_PATTERN, false),
        Genre::Reggae => (&REGGAE_PATTERN, false),
        Genre::Country => (&COUNTRY_PATTERN, false),
    }
}

/// The long eighth of a swung pair takes this fraction of the beat (the
/// short one takes the rest) — the same 2:1 "triplet swing" ratio
/// `metronome_overlay`'s `MetronomeFeel::Shuffle` clicks to, so a genre
/// that swings (see [`genre_pattern`]) swings in step with the shuffle-feel
/// metronome. A straight genre splits the beat evenly instead. `pub(crate)`
/// so `jam::rhythm_guide::active_slot` can use the identical split for its
/// live pulse timing instead of a second, possibly-drifting copy.
pub(crate) const SWING_LONG_FRAC: f32 = 2.0 / 3.0;

/// One simple bass tone: a sine fundamental plus a second and third harmonic
/// for warmth, and a short attack/release envelope. The harmonics matter for
/// more than tone color: octave 2's fundamentals (see [`bar_beat_freqs`])
/// sit around 65–110 Hz, below what small/laptop speakers can reproduce, so
/// the *speaker-audible* part of this tone is disproportionately the
/// 2nd/3rd harmonics (130–330 Hz) — the classic "psychoacoustic bass"
/// problem, not a playback bug.
fn bass_tone(freq_hz: f32, duration_secs: f32) -> Vec<f32> {
    let n = (duration_secs * SAMPLE_RATE as f32).max(1.0) as usize;
    let attack = (SAMPLE_RATE as f32 * ATTACK_SECS) as usize;
    let release = (SAMPLE_RATE as f32 * RELEASE_SECS) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let atk = if attack > 0 && i < attack {
                i as f32 / attack as f32
            } else {
                1.0
            };
            let rel = if n > release && i > n - release {
                (n - i) as f32 / release as f32
            } else {
                1.0
            };
            let env = atk.min(rel).clamp(0.0, 1.0);
            let s = (TAU * freq_hz * t).sin()
                + 0.4 * (TAU * freq_hz * 2.0 * t).sin()
                + 0.22 * (TAU * freq_hz * 3.0 * t).sin();
            env * s * 0.5
        })
        .collect()
}

/// The 8 note frequencies (Hz) of one bar of `pattern` (see the `*_PATTERN`
/// constants and [`genre_pattern`]), rooted on `root`, in the bass register
/// (octave 3 — one octave higher than a real bass guitar, deliberately: a
/// single sine-ish voice with no amp/cabinet coloring, and octave 2's
/// ~65–110 Hz fundamentals are below what small/laptop speakers reproduce,
/// see [`bass_tone`]). `None` for a rest slot, or a note whose resolved
/// name doesn't parse — the latter shouldn't happen for the roots
/// `progression_bars` produces.
fn bar_beat_freqs(root: &str, pattern: &[Option<i32>; 8]) -> [Option<f32>; 8] {
    pattern.map(|slot| {
        let semitones = slot?;
        let note_class = semitone(root, semitones);
        note_to_midi(&format!("{note_class}3")).map(|m| midi_to_freq_hz(m as f32))
    })
}

/// Renders [`CHORUSES`] repeats of a `progression`'s 12-bar bass line in
/// `key` at `bpm` (4/4 throughout), shaped by `genre`'s rhythm pattern and
/// straight/swing feel (see [`genre_pattern`]). Pure and deterministic —
/// the whole backing loop is fully described by
/// `key`/`bpm`/`progression`/`genre`.
pub fn generate_bass_pcm(key: &str, bpm: f32, progression: Progression, genre: Genre) -> Vec<f32> {
    let secs_per_beat = 60.0 / bpm.max(1.0);
    let (pattern, swung) = genre_pattern(genre);
    // Each bar's 8 notes are 4 pairs, one pair per beat. A swung genre's
    // long note takes `SWING_LONG_FRAC` of the beat, the short note the
    // rest; a straight genre splits the beat evenly instead — either way
    // long+short always sums to exactly one beat, so a bar's total length
    // is unaffected by genre (still 4 beats), only how it's subdivided.
    let long_secs = if swung {
        secs_per_beat * SWING_LONG_FRAC
    } else {
        secs_per_beat * 0.5
    };
    let short_secs = secs_per_beat - long_secs;
    let roots = progression_bars(key, progression).map(|(root, _)| root);
    let mut buf = Vec::new();
    for _ in 0..CHORUSES {
        for root in &roots {
            for (i, freq) in bar_beat_freqs(root, pattern).into_iter().enumerate() {
                let note_secs = if i % 2 == 0 { long_secs } else { short_secs };
                let gap_samples = ((note_secs * NOTE_GAP_FRAC) * SAMPLE_RATE as f32) as usize;
                match freq {
                    Some(hz) => buf.extend(bass_tone(hz, note_secs * (1.0 - NOTE_GAP_FRAC))),
                    None => {
                        let silent_samples = (note_secs * SAMPLE_RATE as f32) as usize;
                        buf.extend(std::iter::repeat_n(0.0, silent_samples));
                        continue;
                    }
                }
                buf.extend(std::iter::repeat_n(0.0, gap_samples));
            }
        }
    }
    buf
}

/// The chart half of a generated jam: a diatonic Richter harp for `position`
/// in the jam's `key` (e.g. `Position::Second` picks a harp a 4th below
/// `key` — see `Position::harp_key`), timed to a standard 12-bar
/// progression, and a single marker track item (Jam Session never scores
/// notes, so its only job is satisfying the chart schema's `minItems: 1`
/// and giving the progress bar something to measure against). `song.feel`
/// is seeded from `genre` (see `Genre::metronome_feel`) — picked up by
/// `gameplay::metronome_overlay::feel_from_chart` the same way a
/// hand-authored chart's own `feel` field already is, so the on-screen
/// metronome's swing/straight toggle reflects the picked genre for free.
pub fn generated_chart(
    key: &str,
    bpm: f32,
    progression: Progression,
    position: Position,
    genre: Genre,
    total_secs: f64,
) -> HarpChart {
    let harp_key = position.harp_key(key);
    let mut harmonica = richter_harp(&harp_key);
    if let harmonicon_core::harmonica::Harmonica::Diatonic { position: pos, .. } = &mut harmonica {
        *pos = Some(position.label().to_string());
    }
    HarpChart {
        metadata: Some(Metadata {
            format_version: Some("1.1.0".to_string()),
            author: Some("Harmonicon".to_string()),
            source: Some("Procedurally generated".to_string()),
            license: Some("MIT".to_string()),
            description: Some(format!(
                "Generated {} {} 12-bar jam backing, key of {key}, {bpm:.0} bpm.",
                genre.label(),
                progression.label()
            )),
        }),
        song: Song {
            title: format!("Generated Jam \u{2014} Key of {key}"),
            artist: "Harmonicon".to_string(),
            tempo_bpm: bpm,
            key: key.to_string(),
            time_signature: Some("4/4".to_string()),
            difficulty: Difficulty::Easy,
            feel: Some(genre.metronome_feel()),
        },
        timing: Timing {
            resolution: 480,
            tempo_map: vec![TempoPoint { tick: 0, bpm }],
            time_signature_map: None,
        },
        harmonica,
        track: vec![TrackItem {
            id: None,
            time: Some(0.0),
            tick: None,
            duration: total_secs,
            phrase: None,
            groove: None,
            play_mode: None,
            call: false,
            events: vec![NoteEvent {
                hole: 1,
                action: Action::Blow,
                note: None,
                modifiers: None,
            }],
        }],
        loop_section: None,
        scoring: Scoring {
            perfect_window_ms: 150,
            good_window_ms: 350,
            miss_window_ms: 600,
            combo: None,
            style_bonus: None,
        },
    }
}

/// Builds the full generated-jam `SongManifest`: synthesizes the bass line,
/// registers it as a real `AudioSource` asset, and assembles the chart
/// around it. `background`/`elements` are the caller's choice of
/// placeholder art — Jam Session never reads `elements` at all; `background`
/// paints behind the hole map/12-bar grid (see `jam::session::setup`), so a
/// theme's generic `default_background` is the natural choice.
pub fn build_generated_manifest(
    key: &str,
    bpm: f32,
    progression: Progression,
    position: Position,
    genre: Genre,
    background: Handle<Image>,
    elements: Handle<Image>,
    sources: &mut Assets<AudioSource>,
) -> SongManifest {
    let pcm = generate_bass_pcm(key, bpm, progression, genre);
    let music_duration_secs = pcm.len() as f64 / SAMPLE_RATE as f64;
    let waveform = bucket_peaks(&pcm, WAVEFORM_BUCKETS);
    let wav = encode_wav(&pcm, SAMPLE_RATE);
    let music = sources.add(AudioSource { bytes: wav.into() });

    SongManifest {
        path: PathBuf::from(format!("generated/{key}")),
        chart: generated_chart(key, bpm, progression, position, genre, music_duration_secs),
        background,
        music: Some(music),
        midi_tracks: None,
        waveform,
        music_duration_secs,
        elements,
        assets_2d: None,
        assets_2d_config: NoteThemeConfig::default(),
        assets_3d: None,
        assets_3d_config: NoteCube3dConfig::default(),
        // A generated jam has no source file to have picked a track from,
        // so there is nothing for the harp-check picker to offer.
        source_tracks: Vec::new(),
        source_track: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bar_beat_freqs ───────────────────────────────────────────────────────

    #[test]
    fn bar_beat_freqs_follows_the_blues_box_shape() {
        // R R 5 5 b7 b7 5 b7 — see `BLUES_PATTERN`.
        let freqs = bar_beat_freqs("C", &BLUES_PATTERN);
        let hz = |note: &str| midi_to_freq_hz(note_to_midi(note).unwrap() as f32);
        let (root_hz, fifth_hz, flat7_hz) = (hz("C3"), hz("G3"), hz("A#3"));
        let expected = [
            root_hz, root_hz, fifth_hz, fifth_hz, flat7_hz, flat7_hz, fifth_hz, flat7_hz,
        ];
        for (i, (got, want)) in freqs.iter().zip(expected).enumerate() {
            assert!(
                (got.unwrap() - want).abs() < 0.01,
                "note {i}: got {got:?}, expected {want}"
            );
        }
    }

    #[test]
    fn bar_beat_freqs_stays_above_typical_small_speaker_cutoff() {
        // Regression guard for the "technically playing, inaudible on a
        // laptop speaker" bug: every note every genre can produce, across
        // every key, must clear ~100 Hz. C is the lowest pitch class, so if
        // it clears the bar every other key does too.
        for genre in Genre::all() {
            let (pattern, _) = genre_pattern(*genre);
            for &f in bar_beat_freqs("C", pattern).iter().flatten() {
                assert!(
                    f > 100.0,
                    "{genre:?}: {f} Hz is below typical small-speaker cutoff"
                );
            }
        }
    }

    // ── Genre ────────────────────────────────────────────────────────────────

    #[test]
    fn every_genre_label_round_trips_through_from_label() {
        for &genre in Genre::all() {
            assert_eq!(Genre::from_label(genre.label()), Some(genre));
        }
    }

    #[test]
    fn from_label_is_none_for_an_unknown_label() {
        assert_eq!(Genre::from_label("Ska"), None);
    }

    #[test]
    fn blues_and_jazz_swing_the_rest_play_straight() {
        assert!(genre_pattern(Genre::Blues).1);
        assert!(genre_pattern(Genre::Jazz).1);
        assert!(!genre_pattern(Genre::Rock).1);
        assert!(!genre_pattern(Genre::Reggae).1);
        assert!(!genre_pattern(Genre::Country).1);
    }

    #[test]
    fn metronome_feel_matches_swing_flag() {
        // Every swung genre should feel like Shuffle live, every straight
        // genre like Straight — the two shouldn't be able to disagree.
        for &genre in Genre::all() {
            let (_, swung) = genre_pattern(genre);
            let expected = if swung { Feel::Shuffle } else { Feel::Straight };
            assert_eq!(genre.metronome_feel(), expected, "{genre:?}");
        }
    }

    // ── generate_bass_pcm ────────────────────────────────────────────────────

    #[test]
    fn generate_bass_pcm_is_audible() {
        let pcm = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        assert!(!pcm.is_empty());
        assert!(
            pcm.iter().any(|&s| s.abs() > 0.01),
            "generated backing should not be silent"
        );
    }

    #[test]
    fn every_genre_is_audible() {
        for &genre in Genre::all() {
            let pcm = generate_bass_pcm("C", 90.0, Progression::Standard, genre);
            assert!(
                pcm.iter().any(|&s| s.abs() > 0.01),
                "{genre:?}: generated backing should not be silent"
            );
        }
    }

    #[test]
    fn generate_bass_pcm_length_matches_chorus_count_and_tempo() {
        let bpm = 120.0;
        let pcm = generate_bass_pcm("C", bpm, Progression::Standard, Genre::Blues);
        let secs_per_beat = 60.0 / bpm;
        let expected_secs = CHORUSES as f64 * 12.0 * 4.0 * secs_per_beat as f64;
        let actual_secs = pcm.len() as f64 / SAMPLE_RATE as f64;
        assert!(
            (actual_secs - expected_secs).abs() < 0.5,
            "expected ~{expected_secs}s, got {actual_secs}s"
        );
    }

    #[test]
    fn faster_tempo_yields_a_shorter_loop() {
        let slow = generate_bass_pcm("C", 60.0, Progression::Standard, Genre::Blues);
        let fast = generate_bass_pcm("C", 120.0, Progression::Standard, Genre::Blues);
        assert!(fast.len() < slow.len());
    }

    #[test]
    fn every_progression_renders_the_same_length_loop() {
        // Only the chord *roots* differ between progressions — same 12
        // bars, same beats per bar, so the rendered length shouldn't budge.
        let standard = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        let quick = generate_bass_pcm("C", 90.0, Progression::QuickChange, Genre::Blues);
        let minor = generate_bass_pcm("C", 90.0, Progression::Minor, Genre::Blues);
        let jazz = generate_bass_pcm("C", 90.0, Progression::JazzBlues, Genre::Blues);
        assert_eq!(standard.len(), quick.len());
        assert_eq!(standard.len(), minor.len());
        assert_eq!(standard.len(), jazz.len());
    }

    #[test]
    fn every_genre_renders_the_same_length_loop() {
        // Every pattern is still 8 slots summing to exactly one bar (4
        // beats), swung or straight — genre reshapes the rhythm, not the
        // total loop duration. A small tolerance, not exact equality: the
        // swung-vs-straight timing math rounds to whole samples slightly
        // differently, the same reason `generate_bass_pcm_length_matches_
        // chorus_count_and_tempo` above tolerates float error rather than
        // asserting an exact sample count.
        let blues = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        for &genre in Genre::all() {
            let pcm = generate_bass_pcm("C", 90.0, Progression::Standard, genre);
            let diff_secs = (blues.len() as f64 - pcm.len() as f64).abs() / SAMPLE_RATE as f64;
            assert!(
                diff_secs < 0.1,
                "{genre:?} loop length diverged from Blues by {diff_secs}s"
            );
        }
    }

    #[test]
    fn different_genres_produce_different_audio() {
        // Guards against a future refactor silently collapsing genres back
        // to identical output.
        let blues = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Blues);
        let rock = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Rock);
        let reggae = generate_bass_pcm("C", 90.0, Progression::Standard, Genre::Reggae);
        assert_ne!(blues, rock);
        assert_ne!(blues, reggae);
        assert_ne!(rock, reggae);
    }

    // ── generated_chart ──────────────────────────────────────────────────────

    #[test]
    fn generated_chart_carries_the_requested_key_and_tempo() {
        let chart = generated_chart(
            "G",
            100.0,
            Progression::Standard,
            Position::First,
            Genre::Blues,
            30.0,
        );
        assert_eq!(chart.song.key, "G");
        assert_eq!(chart.song.tempo_bpm, 100.0);
        assert_eq!(chart.timing.tempo_map[0].bpm, 100.0);
    }

    #[test]
    fn generated_chart_harmonica_is_a_diatonic_richter_harp_in_key_at_first_position() {
        let chart = generated_chart(
            "D",
            90.0,
            Progression::Standard,
            Position::First,
            Genre::Blues,
            30.0,
        );
        match chart.harmonica {
            harmonicon_core::harmonica::Harmonica::Diatonic {
                holes,
                layout,
                position,
                ..
            } => {
                assert_eq!(holes, 10);
                let layout = layout.expect("richter_harp always sets a layout");
                assert_eq!(layout.blow.unwrap()[0], "D4");
                assert_eq!(position.as_deref(), Some("1st"));
            }
            _ => panic!("expected a diatonic harp"),
        }
    }

    #[test]
    fn generated_chart_second_position_picks_a_harp_a_fourth_below_the_jam_key() {
        // A cross-harp jam in G is played on a C harp.
        let chart = generated_chart(
            "G",
            90.0,
            Progression::Standard,
            Position::Second,
            Genre::Blues,
            30.0,
        );
        match chart.harmonica {
            harmonicon_core::harmonica::Harmonica::Diatonic {
                layout, position, ..
            } => {
                let layout = layout.expect("richter_harp always sets a layout");
                assert_eq!(layout.blow.unwrap()[0], "C4");
                assert_eq!(position.as_deref(), Some("2nd"));
            }
            _ => panic!("expected a diatonic harp"),
        }
    }

    #[test]
    fn generated_chart_track_is_never_empty() {
        // The chart schema requires `track.minItems: 1` — a generated jam
        // has no real notes to schedule, but must still satisfy it.
        let chart = generated_chart(
            "C",
            90.0,
            Progression::Standard,
            Position::First,
            Genre::Blues,
            30.0,
        );
        assert!(!chart.track.is_empty());
        assert!(!chart.track[0].events.is_empty());
    }

    #[test]
    fn generated_chart_feel_matches_genre() {
        for &genre in Genre::all() {
            let chart = generated_chart(
                "C",
                90.0,
                Progression::Standard,
                Position::First,
                genre,
                30.0,
            );
            assert_eq!(chart.song.feel, Some(genre.metronome_feel()), "{genre:?}");
        }
    }

    // ── build_generated_manifest ─────────────────────────────────────────────

    #[test]
    fn build_generated_manifest_registers_a_real_audio_asset() {
        let mut sources = Assets::<AudioSource>::default();
        let manifest = build_generated_manifest(
            "C",
            90.0,
            Progression::Standard,
            Position::First,
            Genre::Blues,
            Handle::default(),
            Handle::default(),
            &mut sources,
        );
        let music = manifest.music.expect("generated jam always has music");
        assert!(sources.get(&music).is_some());
        assert!(manifest.music_duration_secs > 0.0);
        assert_eq!(manifest.waveform.len(), WAVEFORM_BUCKETS);
    }
}
