// SPDX-License-Identifier: MIT

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::audio_system::midi::midi_to_note;
use crate::audio_system::pitch_detect::PitchEvent;
use crate::gameplay::{classify_note, compute_points, sustain_points, HitQuality, NoteOutcome};
use crate::settings::AudioSettings;

use super::playback::{key_offset, note_freq, EditorAudio, Playhead};
use super::state::EditorState;
use super::TICKS_PER_BEAT;

// ── Timing windows ────────────────────────────────────────────────────────────

/// Onset must land within ±60 ms of the note start for a Perfect.
const PERFECT_WINDOW: f64 = 0.060;
/// Onset within ±130 ms scores a Good.
const GOOD_WINDOW: f64 = 0.130;
/// After 200 ms past the onset the note is marked Missed.
const MISS_WINDOW: f64 = 0.200;

/// 2^(0.5/12) — frequency ratio spanning ±50 cents.
/// Detected pitches within this band of the expected frequency count as a match.
const PITCH_TOLERANCE: f32 = 1.029_302_2;

// ── Types ─────────────────────────────────────────────────────────────────────

/// One note from the editor grid, compiled into a practice-scoring record.
struct PracticeNote {
    start_secs:    f64,
    end_secs:      f64,
    /// Expected pitch frequency in Hz (key-transposed, bend-adjusted).
    expected_freq: f32,
    /// Human-readable name of the expected pitch, e.g. "G4".
    expected_name: String,
    hit:           bool,
    missed:        bool,
    /// Seconds the player held the correct pitch after scoring the onset.
    held:          f64,
    /// True once the sustain bonus for this note has been paid out.
    sustain_done:  bool,
}

#[derive(Resource, Default)]
pub(super) struct PracticeState {
    pub active: bool,
    notes:      Vec<PracticeNote>,
    /// Indices of notes consumed by the current sustained breath.
    /// A note is dropped from this set once its frequency stops sounding,
    /// re-arming it for the next articulation (same logic as gameplay's PitchGate).
    consumed:   std::collections::HashSet<usize>,
    pub score:  u32,
    pub hits:   u32,
    pub misses: u32,
    pub total:  u32,
    /// Status line shown in the editor's status bar while practice is running.
    pub msg:    String,
}

impl PracticeState {
    pub(super) fn reset(&mut self) {
        *self = PracticeState::default();
    }
}

// ── Schedule builder ──────────────────────────────────────────────────────────

fn build_schedule(state: &EditorState) -> Vec<PracticeNote> {
    let bpm: f32 = state.tempo.parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
    let k_off = key_offset(&state.key);

    let mut notes: Vec<PracticeNote> = state
        .notes
        .iter()
        .filter_map(|n| {
            let freq = note_freq(n, k_off)?;
            let midi = (69.0_f32 + 12.0 * (freq / 440.0).log2()).round() as i32;
            let name = midi_to_note(midi);
            Some(PracticeNote {
                start_secs:    n.tick as f64 * secs_per_tick as f64,
                end_secs:      (n.tick + n.len) as f64 * secs_per_tick as f64,
                expected_freq: freq,
                expected_name: name,
                hit:           false,
                missed:        false,
                held:          0.0,
                sustain_done:  false,
            })
        })
        .collect();

    notes.sort_by(|a, b| {
        a.start_secs
            .partial_cmp(&b.start_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    notes
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Start practice mode: plays background music only (no synthesized notes),
/// then scores the player's microphone input against the editor's note grid.
pub(super) fn start_practice(
    state:    &EditorState,
    sources:  &mut Assets<AudioSource>,
    settings: &AudioSettings,
    playing:  &Query<Entity, With<EditorAudio>>,
    practice: &mut PracticeState,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }

    practice.reset();
    practice.notes = build_schedule(state);
    practice.total = practice.notes.len() as u32;
    practice.active = true;

    let bpm: f32 = state.tempo.parse::<f32>().unwrap_or(120.0).max(1.0);
    let secs_per_tick = 60.0 / bpm / TICKS_PER_BEAT as f32;
    let end_tick = state.notes.iter().map(|n| n.tick + n.len).max().unwrap_or(0);

    *playhead = Playhead {
        playing:      true,
        elapsed:      0.0,
        total:        end_tick as f32 * secs_per_tick,
        secs_per_tick,
    };

    let music = state.music.trim();
    if !music.is_empty() {
        match std::fs::read(music) {
            Ok(bytes) => {
                let handle = sources.add(AudioSource { bytes: bytes.into() });
                commands.spawn((
                    EditorAudio,
                    AudioPlayer::<AudioSource>(handle),
                    PlaybackSettings::DESPAWN
                        .with_volume(Volume::Linear(settings.music_volume)),
                ));
            }
            Err(e) => warn!("Practice: can't read background music {music:?}: {e}"),
        }
    } else {
        practice.msg = "No background music set — play along with the chart!".into();
    }
}

/// Stop practice and reset all state. Safe to call when practice is not active.
pub(super) fn stop_practice(
    playing:  &Query<Entity, With<EditorAudio>>,
    practice: &mut PracticeState,
    playhead: &mut Playhead,
    commands: &mut Commands,
) {
    for e in playing {
        commands.entity(e).despawn();
    }
    playhead.playing = false;
    practice.reset();
}

// ── System ────────────────────────────────────────────────────────────────────

pub(super) fn practice_tick(
    time:             Res<Time>,
    playhead:         Res<Playhead>,
    settings:         Res<AudioSettings>,
    mut pitch_events: MessageReader<PitchEvent>,
    mut practice:     ResMut<PracticeState>,
) {
    if !practice.active {
        // Drain unread pitch events so they don't pile up while idle.
        for _ in pitch_events.read() {}
        return;
    }

    // Playback ended — finalize any unscored notes and show the summary.
    if !playhead.playing {
        let mut extra_misses = 0u32;
        for note in practice.notes.iter_mut() {
            if !note.hit && !note.missed {
                note.missed = true;
                extra_misses += 1;
            }
        }
        practice.misses += extra_misses;
        let (hits, total, score) = (practice.hits, practice.total, practice.score);
        practice.msg = format!("Done — {hits}/{total} notes  ·  {score} pts");
        practice.active = false;
        return;
    }

    // Collect the freshest detected pitches; last event wins (pitch events arrive
    // at the audio pipeline's chunk rate, ~10 Hz, not at the frame rate).
    let mut detected: Vec<f32> = Vec::new();
    for ev in pitch_events.read() {
        detected = ev.0.iter().map(|p| p.frequency).collect();
    }

    // Latency compensation: a pitch detected now was actually played
    // `input_latency_ms` ago, so shift the judgment point back.
    let latency = settings.input_latency_ms as f64 / 1000.0;
    let judged = playhead.elapsed as f64 - latency;
    let dt = time.delta_secs_f64();

    // Take ownership of `consumed` so we can freely read `notes` while filtering.
    // Re-arm any entry whose frequency is no longer sounding — the player must
    // re-articulate to score the next occurrence of the same pitch.
    let mut consumed = std::mem::take(&mut practice.consumed);
    consumed.retain(|&idx| {
        practice
            .notes
            .get(idx)
            .is_some_and(|n| detected.iter().any(|&f| freq_matches(f, n.expected_freq)))
    });

    // Score all notes, collecting mutations for application after the loop.
    let mut new_consumed:  Vec<usize> = Vec::new();
    let mut hits_delta:    u32 = 0;
    let mut misses_delta:  u32 = 0;
    let mut score_delta:   u32 = 0;
    let mut new_msg: Option<String> = None;

    for (i, note) in practice.notes.iter_mut().enumerate() {
        if note.missed { continue; }

        // Sustain phase: onset was already scored — reward holding the pitch.
        if note.hit {
            if note.sustain_done { continue; }
            if judged < note.end_secs {
                if detected.iter().any(|&f| freq_matches(f, note.expected_freq)) {
                    note.held += dt;
                }
            } else {
                score_delta += sustain_points(note.held, note.end_secs - note.start_secs);
                note.sustain_done = true;
            }
            continue;
        }

        let offset = judged - note.start_secs;
        // A note scores only on a fresh attack: the pitch must be sounding AND
        // not already consumed by an earlier note in this continuous breath.
        let playing_expected = !consumed.contains(&i)
            && detected.iter().any(|&f| freq_matches(f, note.expected_freq));

        match classify_note(offset, playing_expected, PERFECT_WINDOW, GOOD_WINDOW, MISS_WINDOW) {
            NoteOutcome::Missed => {
                note.missed = true;
                misses_delta += 1;
                new_msg.get_or_insert_with(|| format!("✗ Missed {}", note.expected_name));
            }
            NoteOutcome::Waiting => {
                let got = detected.first().copied().map(freq_to_name).unwrap_or_default();
                new_msg.get_or_insert_with(|| {
                    if got.is_empty() {
                        format!("▶ Play {}…", note.expected_name)
                    } else {
                        format!("▶ {} → need {}", got, note.expected_name)
                    }
                });
            }
            NoteOutcome::Hit(quality) => {
                note.hit = true;
                hits_delta += 1;
                new_consumed.push(i);
                let pts = compute_points(quality, 1.0);
                score_delta += pts;
                new_msg = Some(match quality {
                    HitQuality::Perfect => format!("✓ PERFECT  {}  +{pts}", note.expected_name),
                    HitQuality::Good    => format!("✓ GOOD  {}  +{pts}", note.expected_name),
                });
            }
            NoteOutcome::TooEarly | NoteOutcome::Gap => {}
        }
    }

    consumed.extend(new_consumed);
    practice.consumed = consumed;
    practice.hits   += hits_delta;
    practice.misses += misses_delta;
    practice.score  += score_delta;
    if let Some(msg) = new_msg {
        practice.msg = msg;
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// True when `detected` is within ±50 cents of `expected`.
fn freq_matches(detected: f32, expected: f32) -> bool {
    if expected <= 0.0 { return false; }
    let ratio = detected / expected;
    ratio >= 1.0 / PITCH_TOLERANCE && ratio <= PITCH_TOLERANCE
}

/// Nearest MIDI note name for a raw frequency (used in "you played X" messages).
fn freq_to_name(freq: f32) -> String {
    if freq <= 0.0 { return String::new(); }
    let midi = (69.0_f32 + 12.0 * (freq / 440.0).log2()).round() as i32;
    if !(0..=127).contains(&midi) { return String::new(); }
    midi_to_note(midi)
}
