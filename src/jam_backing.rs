// SPDX-License-Identifier: MIT

//! Generated Jam Session backing: a simple synthesized 12-bar walking bass
//! line for any key/tempo/progression, so Jam Session doesn't require
//! picking an existing song. See `PLAN.md`'s "Backing track variety" entry.
//!
//! Deliberately not the harmonica-timbre synth `song_editor::playback`
//! shares with `gameplay::call_response` — a backing bass is a different
//! instrument, and reusing harmonica partials here would risk sounding like
//! a second harmonica part to echo instead of backing to play over.

use std::f32::consts::TAU;
use std::path::PathBuf;

use bevy::audio::AudioSource;
use bevy::prelude::*;

use crate::audio_system::midi::{midi_to_freq_hz, note_to_midi};
use crate::audio_system::wav::encode_wav;
use crate::audio_system::waveform::{WAVEFORM_BUCKETS, bucket_peaks};
use crate::song::chart::{
    Action, Difficulty, HarpChart, Metadata, NoteEvent, Scoring, Song, TempoPoint, Timing,
    TrackItem,
};
use crate::song::harmonica::{Progression, progression_bars, richter_harp, semitone};
use crate::song::{NoteCube3dConfig, NoteThemeConfig, SongManifest};

/// Present while a generated-backing jam is in flight (from the "Start Jam"
/// button through `Playing`, including any Restart). Its presence — checked
/// by both `menu::route_menu_entry` and `gameplay::pause_menu::on_restart`
/// — is what tells those two call sites this `SelectedSong` was built by
/// [`build_generated_manifest`] via `Assets::add` rather than loaded through
/// the `AssetServer`, so it has no tracked `LoadState` for `check_loading`'s
/// `is_loaded_with_dependencies` to ever find: both routes skip
/// `AppState::SongLoading` and go straight to `Playing`. Removed on
/// returning to the menu, the same end-of-life point `LessonContext` uses.
#[derive(Resource)]
pub struct GeneratedJamSession;

pub const SAMPLE_RATE: u32 = 44_100;

/// How many 12-bar choruses to render into one generated backing loop —
/// long enough for a real practice session (a few minutes) without an
/// unreasonably large buffer/asset. `JamLoop` (the existing player toggle)
/// still works normally once this runs out.
pub const CHORUSES: u32 = 8;

const ATTACK_SECS: f32 = 0.01;
const RELEASE_SECS: f32 = 0.05;
/// Fraction of each beat left as silence between bass notes, so consecutive
/// notes don't blur into one continuous tone.
const NOTE_GAP_FRAC: f32 = 0.08;

/// One simple bass tone: a sine fundamental plus a second and third harmonic
/// for warmth, and a short attack/release envelope. The harmonics matter for
/// more than tone color here — octave 2's fundamentals (see
/// [`bar_beat_freqs`]) sit around 65–110 Hz, below what small/laptop
/// speakers can reproduce at all, so the *speaker-audible* part of this
/// tone is disproportionately the 2nd/3rd harmonics (130–330 Hz). Without
/// them, the bass line is technically playing (real, non-silent PCM) but
/// genuinely inaudible on that class of hardware — the classic "psychoacoustic
/// bass" problem, not a playback bug.
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

/// The four beat frequencies (Hz) of one bar's root-fifth-root-fifth walking
/// pattern, in the bass register (octave 3 — one octave higher than a real
/// bass guitar would sit, deliberately: this is a single sine-ish voice with
/// no amp/cabinet coloring, and octave 2's ~65–110 Hz fundamentals are below
/// what small/laptop speakers reproduce at all, see [`bass_tone`]). `None`
/// for a beat whose note name doesn't resolve — shouldn't happen for the
/// roots `progression_bars` produces, but stays honest about the
/// possibility rather than panicking. Quality-agnostic on purpose: root and
/// fifth are the same notes whether the bar's chord is dominant or minor
/// 7th (only the 3rd would differ, and this simple bass line doesn't play
/// one), so a minor blues doesn't need any different treatment here — only
/// its chord *roots* change from `Progression::Standard`'s, same as
/// quick-change.
fn bar_beat_freqs(root: &str) -> [Option<f32>; 4] {
    let fifth = semitone(root, 7);
    let freq =
        |note_class: &str| note_to_midi(&format!("{note_class}3")).map(|m| midi_to_freq_hz(m as f32));
    let r = freq(root);
    let f = freq(&fifth);
    [r, f, r, f]
}

/// Renders [`CHORUSES`] repeats of a `progression`'s 12-bar walking bass
/// line in `key` at `bpm` (4/4 throughout). Pure and deterministic — the
/// whole backing loop is fully described by `key`/`bpm`/`progression`.
pub fn generate_bass_pcm(key: &str, bpm: f32, progression: Progression) -> Vec<f32> {
    let secs_per_beat = 60.0 / bpm.max(1.0);
    let gap_samples = ((secs_per_beat * NOTE_GAP_FRAC) * SAMPLE_RATE as f32) as usize;
    let roots = progression_bars(key, progression).map(|(root, _)| root);
    let mut buf = Vec::new();
    for _ in 0..CHORUSES {
        for root in &roots {
            for freq in bar_beat_freqs(root) {
                match freq {
                    Some(hz) => buf.extend(bass_tone(hz, secs_per_beat * (1.0 - NOTE_GAP_FRAC))),
                    None => {
                        let silent_samples = (secs_per_beat * SAMPLE_RATE as f32) as usize;
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

/// The chart half of a generated jam: a diatonic Richter harp in `key`
/// (1st position — the physical harp and the jam key are the same for now;
/// cross-harp position selection is a natural follow-up, not core to
/// whether generated backing works at all), timed to a standard 12-bar
/// progression, and a single marker track item (Jam Session never scores
/// notes, so its only job is satisfying the chart schema's `minItems: 1`
/// and giving the progress bar something to measure against).
pub fn generated_chart(key: &str, bpm: f32, progression: Progression, total_secs: f64) -> HarpChart {
    HarpChart {
        metadata: Some(Metadata {
            format_version: Some("1.1.0".to_string()),
            author: Some("Harmonicon".to_string()),
            source: Some("Procedurally generated".to_string()),
            license: Some("MIT".to_string()),
            description: Some(format!(
                "Generated {} 12-bar blues jam backing, key of {key}, {bpm:.0} bpm.",
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
            feel: None,
        },
        timing: Timing {
            resolution: 480,
            tempo_map: vec![TempoPoint { tick: 0, bpm }],
            time_signature_map: None,
        },
        harmonica: richter_harp(key),
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
/// paints behind the hole map/12-bar grid (see `jam_session::setup`), so a
/// theme's generic `default_background` is the natural choice.
pub fn build_generated_manifest(
    key: &str,
    bpm: f32,
    progression: Progression,
    background: Handle<Image>,
    elements: Handle<Image>,
    sources: &mut Assets<AudioSource>,
) -> SongManifest {
    let pcm = generate_bass_pcm(key, bpm, progression);
    let music_duration_secs = pcm.len() as f64 / SAMPLE_RATE as f64;
    let waveform = bucket_peaks(&pcm, WAVEFORM_BUCKETS);
    let wav = encode_wav(&pcm, SAMPLE_RATE);
    let music = sources.add(AudioSource { bytes: wav.into() });

    SongManifest {
        path: PathBuf::from(format!("generated/{key}")),
        chart: generated_chart(key, bpm, progression, music_duration_secs),
        background,
        music,
        waveform,
        music_duration_secs,
        elements,
        assets_2d: None,
        assets_2d_config: NoteThemeConfig::default(),
        assets_3d: None,
        assets_3d_config: NoteCube3dConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bar_beat_freqs ───────────────────────────────────────────────────────

    #[test]
    fn bar_beat_freqs_alternates_root_and_fifth() {
        let freqs = bar_beat_freqs("C");
        let root_hz = midi_to_freq_hz(note_to_midi("C3").unwrap() as f32);
        let fifth_hz = midi_to_freq_hz(note_to_midi("G3").unwrap() as f32);
        assert!((freqs[0].unwrap() - root_hz).abs() < 0.01);
        assert!((freqs[1].unwrap() - fifth_hz).abs() < 0.01);
        assert!((freqs[2].unwrap() - root_hz).abs() < 0.01);
        assert!((freqs[3].unwrap() - fifth_hz).abs() < 0.01);
    }

    #[test]
    fn bar_beat_freqs_stays_above_typical_small_speaker_cutoff() {
        // Regression guard for the "technically playing, inaudible on a
        // laptop speaker" bug: every root/fifth this can produce, across
        // every key, must clear ~100 Hz. C is the lowest pitch class, so if
        // it clears the bar every other key does too.
        for &f in bar_beat_freqs("C").iter().flatten() {
            assert!(f > 100.0, "{f} Hz is below typical small-speaker cutoff");
        }
    }

    // ── generate_bass_pcm ────────────────────────────────────────────────────

    #[test]
    fn generate_bass_pcm_is_audible() {
        let pcm = generate_bass_pcm("C", 90.0, Progression::Standard);
        assert!(!pcm.is_empty());
        assert!(
            pcm.iter().any(|&s| s.abs() > 0.01),
            "generated backing should not be silent"
        );
    }

    #[test]
    fn generate_bass_pcm_length_matches_chorus_count_and_tempo() {
        let bpm = 120.0;
        let pcm = generate_bass_pcm("C", bpm, Progression::Standard);
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
        let slow = generate_bass_pcm("C", 60.0, Progression::Standard);
        let fast = generate_bass_pcm("C", 120.0, Progression::Standard);
        assert!(fast.len() < slow.len());
    }

    #[test]
    fn every_progression_renders_the_same_length_loop() {
        // Only the chord *roots* differ between progressions — same 12
        // bars, same beats per bar, so the rendered length shouldn't budge.
        let standard = generate_bass_pcm("C", 90.0, Progression::Standard);
        let quick = generate_bass_pcm("C", 90.0, Progression::QuickChange);
        let minor = generate_bass_pcm("C", 90.0, Progression::Minor);
        assert_eq!(standard.len(), quick.len());
        assert_eq!(standard.len(), minor.len());
    }

    // ── generated_chart ──────────────────────────────────────────────────────

    #[test]
    fn generated_chart_carries_the_requested_key_and_tempo() {
        let chart = generated_chart("G", 100.0, Progression::Standard, 30.0);
        assert_eq!(chart.song.key, "G");
        assert_eq!(chart.song.tempo_bpm, 100.0);
        assert_eq!(chart.timing.tempo_map[0].bpm, 100.0);
    }

    #[test]
    fn generated_chart_harmonica_is_a_diatonic_richter_harp_in_key() {
        let chart = generated_chart("D", 90.0, Progression::Standard, 30.0);
        match chart.harmonica {
            crate::song::harmonica::Harmonica::Diatonic { holes, layout, .. } => {
                assert_eq!(holes, 10);
                let layout = layout.expect("richter_harp always sets a layout");
                assert_eq!(layout.blow.unwrap()[0], "D4");
            }
            _ => panic!("expected a diatonic harp"),
        }
    }

    #[test]
    fn generated_chart_track_is_never_empty() {
        // The chart schema requires `track.minItems: 1` — a generated jam
        // has no real notes to schedule, but must still satisfy it.
        let chart = generated_chart("C", 90.0, Progression::Standard, 30.0);
        assert!(!chart.track.is_empty());
        assert!(!chart.track[0].events.is_empty());
    }

    // ── build_generated_manifest ─────────────────────────────────────────────

    #[test]
    fn build_generated_manifest_registers_a_real_audio_asset() {
        let mut sources = Assets::<AudioSource>::default();
        let manifest = build_generated_manifest(
            "C",
            90.0,
            Progression::Standard,
            Handle::default(),
            Handle::default(),
            &mut sources,
        );
        assert!(sources.get(&manifest.music).is_some());
        assert!(manifest.music_duration_secs > 0.0);
        assert_eq!(manifest.waveform.len(), WAVEFORM_BUCKETS);
    }
}
