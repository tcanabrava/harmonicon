// SPDX-License-Identifier: MIT

//! Standalone Bending Trainer: the Let's Bend-style harmonica bend diagram + the
//! metronome, with a directly pickable key and an adjustable tempo — no song.
//!
//! It's its own [`AppState::BendingTrainer`](crate::menu::AppState), driving the
//! decoupled [`MetronomeTempo`] and its own copy of the gameplay clock so the
//! metronome ticks without any song loaded. The harp is synthesised for the
//! chosen key (transposed Richter layout) and the diagram is rebuilt whenever
//! the key changes.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::audio_system::midi::{midi_to_note, note_to_midi};
use crate::audio_system::wav::encode_wav;
use crate::dialogs::button;
use crate::menu::AppState;
use crate::song::chart::{BendingProfile, DiatonicLayout};
use crate::song::harmonica::Harmonica;

use super::harmonica_overlay::{hole_notes, spawn_harmonica_overlay, HoleNotes};
use super::metronome_overlay::{MetronomeTempo, spawn_metronome};
use super::{ActivePitches, GameplayClock, GameplayRoot};

const KEYS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const C_BLOW: [&str; 10] = ["C4", "E4", "G4", "C5", "E5", "G5", "C6", "E6", "G6", "C7"];
const C_DRAW: [&str; 10] = ["D4", "G4", "B4", "D5", "F5", "A5", "B5", "D6", "F6", "A6"];

const MIN_BPM: f32 = 40.0;
const MAX_BPM: f32 = 220.0;
const BPM_STEP: f32 = 5.0;

/// The key the trainer's diagram is currently built for.
#[derive(Resource)]
pub struct TrainerKey(pub String);

impl Default for TrainerKey {
    fn default() -> Self {
        Self("C".to_string())
    }
}

/// Wraps the harmonica diagram so it can be despawned + rebuilt on key change.
#[derive(Component)]
pub struct OverlayHost;

/// The "Key: X" readout.
#[derive(Component)]
pub struct KeyLabel;

// ── Ear-training target ─────────────────────────────────────────────────────────

/// Which of the six technique rows in the diagram is the current target.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Technique {
    Blow,
    Draw,
    Bend1,
    Bend2,
    Bend3,
    Over,
}

impl Technique {
    fn label(self) -> &'static str {
        match self {
            Technique::Blow => "Blow",
            Technique::Draw => "Draw",
            Technique::Bend1 => "\u{00BD}-step bend",
            Technique::Bend2 => "1-step bend",
            Technique::Bend3 => "1\u{00BD}-step bend",
            Technique::Over => "Overblow/draw",
        }
    }

    fn next(self) -> Self {
        match self {
            Technique::Blow => Technique::Draw,
            Technique::Draw => Technique::Bend1,
            Technique::Bend1 => Technique::Bend2,
            Technique::Bend2 => Technique::Bend3,
            Technique::Bend3 => Technique::Over,
            Technique::Over => Technique::Blow,
        }
    }

    fn prev(self) -> Self {
        // Same cycle, walked backward — six variants, so five `next` steps.
        (0..5).fold(self, |t, _| t.next())
    }

    fn note(self, holes: &HoleNotes) -> Option<&str> {
        match self {
            Technique::Blow => holes.blow.as_deref(),
            Technique::Draw => holes.draw.as_deref(),
            Technique::Bend1 => holes.bends.first().map(String::as_str),
            Technique::Bend2 => holes.bends.get(1).map(String::as_str),
            Technique::Bend3 => holes.bends.get(2).map(String::as_str),
            Technique::Over => holes.over.as_deref(),
        }
    }
}

/// The hole + technique the "Listen" button and the live tuner readout target.
/// Not every hole has every technique (e.g. hole 5 has no bend) — [`Technique::note`]
/// returns `None` for a hole/technique pair the harp can't produce.
#[derive(Resource, Clone, Copy)]
pub struct TrainerTarget {
    pub hole: u8,
    pub technique: Technique,
}

impl Default for TrainerTarget {
    fn default() -> Self {
        // Hole 2's half-step draw bend: the classic first bend most players learn.
        Self { hole: 2, technique: Technique::Bend1 }
    }
}

/// The "Target: Hole N Draw" readout.
#[derive(Component)]
pub struct TargetLabel;

/// The live cents-off tuner readout.
#[derive(Component)]
pub struct TunerReadout;

/// Note name for the current target on `harp`, or `None` if that hole doesn't
/// have that technique (e.g. hole 5 has no bend, most holes have no overblow).
fn target_note(harp: &Harmonica, target: TrainerTarget) -> Option<String> {
    let holes = hole_notes(harp, target.hole);
    target.technique.note(&holes).map(str::to_string)
}

/// Frequency in Hz for a note label like `"C#4"`.
fn note_freq_hz(note: &str) -> Option<f32> {
    let midi = note_to_midi(note)?;
    Some(440.0 * 2f32.powf((midi - 69) as f32 / 12.0))
}

/// A short, clean reference tone (fundamental + two soft harmonics) — plain
/// enough to make the *pitch* the whole focus, unlike the full harmonica
/// synth used elsewhere, which is deliberately breathy/textured.
fn synth_reference_tone(freq: f32) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 44_100;
    const DUR_SECS: f32 = 1.1;
    let n = (SAMPLE_RATE as f32 * DUR_SECS) as usize;
    let mut buf = vec![0.0f32; n];
    for (i, sample) in buf.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        let attack = (t / 0.02).min(1.0);
        let release = ((DUR_SECS - t) / 0.15).clamp(0.0, 1.0);
        let env = attack.min(release);
        let tau = std::f32::consts::TAU;
        let s = (tau * freq * t).sin()
            + 0.30 * (tau * freq * 2.0 * t).sin()
            + 0.12 * (tau * freq * 3.0 * t).sin();
        *sample = env * 0.28 * s;
    }
    encode_wav(&buf, SAMPLE_RATE)
}

// ── Key helpers ───────────────────────────────────────────────────────────────

fn next_key(k: &str) -> String {
    let i = KEYS.iter().position(|&x| x == k).unwrap_or(0);
    KEYS[(i + 1) % 12].to_string()
}

fn prev_key(k: &str) -> String {
    let i = KEYS.iter().position(|&x| x == k).unwrap_or(0);
    KEYS[(i + 11) % 12].to_string()
}

/// Semitone shift from a C harp to `key`, choosing the octave the real harp
/// sits in: keys up to F# pitch above C, G–B pitch below (the "low" harps).
fn key_offset(key: &str) -> i32 {
    let k = KEYS.iter().position(|&x| x == key).unwrap_or(0) as i32;
    if k <= 6 { k } else { k - 12 }
}

/// A Richter diatonic harp for `key`, transposed from the C reference layout.
fn richter_harp(key: &str) -> Harmonica {
    let off = key_offset(key);
    let tr = |notes: &[&str]| -> Vec<String> {
        notes
            .iter()
            .filter_map(|n| note_to_midi(n).map(|m| midi_to_note(m + off)))
            .collect()
    };
    Harmonica::Diatonic {
        holes: 10,
        bending_profile: BendingProfile::RichterStandard,
        position: None,
        layout: Some(DiatonicLayout {
            blow: Some(tr(&C_BLOW)),
            draw: Some(tr(&C_DRAW)),
        }),
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

pub fn setup(
    mut commands: Commands,
    mut clock: ResMut<GameplayClock>,
    mut tempo: ResMut<MetronomeTempo>,
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
) {
    clock.0 = 0.0;
    tempo.beats_per_bar = 4;
    // Keep whatever BPM was last set; default to a comfortable practice tempo.
    if tempo.bpm < MIN_BPM || tempo.bpm > MAX_BPM {
        tempo.bpm = 90.0;
    }

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            GameplayRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Bending Trainer"),
                TextFont { font_size: FontSize::Px(26.0), ..default() },
                TextColor(Color::WHITE),
            ));

            // ── Key control: ◂  Key: C  ▸ ───────────────────────────────────
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    "\u{25C2}",
                    |_: On<Pointer<Click>>,
                     mut key: ResMut<TrainerKey>| {
                        key.0 = prev_key(&key.0);
                    },
                ));
                row.spawn((
                    Node { width: Val::Px(110.0), justify_content: JustifyContent::Center, ..default() },
                    Text::new(format!("Key: {}", key.0)),
                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                    TextColor(Color::srgb(0.95, 0.80, 0.35)),
                    KeyLabel,
                ));
                row.spawn_empty().apply_scene(button::small(
                    "\u{25B8}",
                    |_: On<Pointer<Click>>,
                     mut key: ResMut<TrainerKey>| {
                        key.0 = next_key(&key.0);
                    },
                ));
            });

            // ── Ear-training target: hole + technique, Listen, tuner ────────
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    "\u{25C2}",
                    |_: On<Pointer<Click>>, mut target: ResMut<TrainerTarget>| {
                        target.hole = if target.hole <= 1 { 10 } else { target.hole - 1 };
                    },
                ));
                row.spawn((
                    Node { width: Val::Px(220.0), justify_content: JustifyContent::Center, ..default() },
                    Text::new(target_label_text(target.hole, target.technique)),
                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                    TextColor(Color::srgb(0.80, 0.90, 0.95)),
                    TargetLabel,
                ));
                row.spawn_empty().apply_scene(button::small(
                    "\u{25B8}",
                    |_: On<Pointer<Click>>, mut target: ResMut<TrainerTarget>| {
                        target.hole = if target.hole >= 10 { 1 } else { target.hole + 1 };
                    },
                ));
                row.spawn_empty().apply_scene(button::small(
                    "technique \u{21BB}",
                    |_: On<Pointer<Click>>, mut target: ResMut<TrainerTarget>| {
                        target.technique = target.technique.next();
                    },
                ));
                row.spawn_empty().apply_scene(button::default(
                    "\u{1F50A} Listen",
                    |_: On<Pointer<Click>>,
                     key: Res<TrainerKey>,
                     target: Res<TrainerTarget>,
                     mut sources: ResMut<Assets<AudioSource>>,
                     mut commands: Commands| {
                        let harp = richter_harp(&key.0);
                        let Some(note) = target_note(&harp, *target) else { return };
                        let Some(freq) = note_freq_hz(&note) else { return };
                        let wav = synth_reference_tone(freq);
                        let handle = sources.add(AudioSource { bytes: wav.into() });
                        commands.spawn((
                            AudioPlayer::<AudioSource>(handle),
                            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.6)),
                        ));
                    },
                ));
            });
            root.spawn((
                Text::new(""),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.55, 0.85, 0.60)),
                TunerReadout,
            ));

            // ── The bend diagram (rebuilt on key change) ────────────────────
            root.spawn((Node::default(), OverlayHost))
                .with_children(|host| {
                    spawn_harmonica_overlay(host, &richter_harp(&key.0));
                });

            // ── Tempo control: −  ♩ = NN (in the metronome)  + ──────────────
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    "\u{2212}",
                    |_: On<Pointer<Click>>,
                     mut tempo: ResMut<MetronomeTempo>| {
                        tempo.bpm = (tempo.bpm - BPM_STEP).max(MIN_BPM);
                    },
                ));
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|metro| {
                    spawn_metronome(metro, tempo.beats_per_bar, tempo.bpm);
                });
                row.spawn_empty().apply_scene(button::small(
                    "+",
                    |_: On<Pointer<Click>>,
                     mut tempo: ResMut<MetronomeTempo>| {
                        tempo.bpm = (tempo.bpm + BPM_STEP).min(MAX_BPM);
                    },
                ));
            });

            root.spawn((
                Text::new("Esc to go back  \u{00B7}  M mutes the click  \u{00B7}  feel toggles straight/shuffle"),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));
        });
}

/// Advance the trainer's own clock (no song to drive it).
pub fn tick_clock(mut clock: ResMut<GameplayClock>, time: Res<Time>) {
    clock.0 += time.delta_secs_f64();
}

/// Rebuild the bend diagram when the key changes.
pub fn rebuild_overlay(
    key: Res<TrainerKey>,
    hosts: Query<(Entity, Option<&Children>), With<OverlayHost>>,
    mut commands: Commands,
) {
    if !key.is_changed() {
        return;
    }
    let harp = richter_harp(&key.0);
    for (host, children) in &hosts {
        if let Some(children) = children {
            for &c in children {
                commands.entity(c).despawn();
            }
        }
        commands
            .entity(host)
            .with_children(|h| spawn_harmonica_overlay(h, &harp));
    }
}

/// Keep the "Key: X" readout in step with the chosen key.
pub fn update_key_label(key: Res<TrainerKey>, mut labels: Query<&mut Text, With<KeyLabel>>) {
    if !key.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(format!("Key: {}", key.0));
    }
}

/// Esc returns to the menu.
pub fn handle_escape(keyboard: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Menu);
    }
}

/// "Target: Hole 2 · ½-step bend" — or a note that the current harp can't
/// actually produce there, so the reader knows why Listen did nothing.
fn target_label_text(hole: u8, technique: Technique) -> String {
    format!("Target: Hole {hole}  \u{00B7}  {}", technique.label())
}

/// Keep the "Target: ..." readout in step with the chosen hole/technique.
pub fn update_target_label(
    target: Res<TrainerTarget>,
    mut labels: Query<&mut Text, With<TargetLabel>>,
) {
    if !target.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(target_label_text(target.hole, target.technique));
    }
}

/// Within this many cents of the target, the readout calls it "in tune"
/// rather than reporting a flat/sharp direction — no bend is perfectly
/// stable, and a human ear can't reliably split hairs finer than this anyway.
const IN_TUNE_CENTS: f32 = 6.0;

/// Live cents-off tuner: compares the closest currently-sounding pitch (from
/// the mic) against the selected target note, and reports how far off — and
/// in which direction — the player currently is.
pub fn update_tuner_readout(
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    active: Res<ActivePitches>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TunerReadout>>,
) {
    let Ok((mut text, mut color)) = labels.single_mut() else { return };
    let harp = richter_harp(&key.0);
    let Some(target_note) = target_note(&harp, *target) else {
        *text = Text::new("This hole has no note for that technique.");
        color.0 = Color::srgb(0.60, 0.60, 0.65);
        return;
    };
    let Some(target_freq) = note_freq_hz(&target_note) else { return };

    let Some(heard) = active.0.iter().min_by(|a, b| {
        (a.frequency.log2() - target_freq.log2())
            .abs()
            .total_cmp(&(b.frequency.log2() - target_freq.log2()).abs())
    }) else {
        *text = Text::new(format!("Play it \u{2014} target {target_note}"));
        color.0 = Color::srgb(0.60, 0.60, 0.65);
        return;
    };

    let cents = 1200.0 * (heard.frequency / target_freq).log2();
    if cents.abs() <= IN_TUNE_CENTS {
        *text = Text::new(format!("\u{2713} In tune  ({target_note})"));
        color.0 = Color::srgb(0.45, 0.85, 0.50);
    } else if cents > 0.0 {
        *text = Text::new(format!("\u{2191} {cents:+.0} cents sharp  (target {target_note})"));
        color.0 = Color::srgb(0.90, 0.70, 0.30);
    } else {
        *text = Text::new(format!("\u{2193} {cents:+.0} cents flat  (target {target_note})"));
        color.0 = Color::srgb(0.90, 0.70, 0.30);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_cycle_both_ways() {
        assert_eq!(next_key("C"), "C#");
        assert_eq!(next_key("B"), "C");
        assert_eq!(prev_key("C"), "B");
    }

    #[test]
    fn key_offsets_pick_the_real_harp_octave() {
        // C harp unchanged; D up two; A and G are the low harps (pitched down).
        assert_eq!(key_offset("C"), 0);
        assert_eq!(key_offset("D"), 2);
        assert_eq!(key_offset("F#"), 6);
        assert_eq!(key_offset("G"), -5);
        assert_eq!(key_offset("A"), -3);
    }

    #[test]
    fn c_harp_keeps_the_reference_layout() {
        let Harmonica::Diatonic { layout: Some(l), .. } = richter_harp("C") else {
            panic!("expected diatonic");
        };
        assert_eq!(l.blow.unwrap()[0], "C4");
        assert_eq!(l.draw.unwrap()[0], "D4");
    }

    #[test]
    fn d_harp_hole_1_blow_is_d4() {
        let Harmonica::Diatonic { layout: Some(l), .. } = richter_harp("D") else {
            panic!("expected diatonic");
        };
        assert_eq!(l.blow.unwrap()[0], "D4");
    }

    #[test]
    fn g_harp_hole_1_blow_is_g3() {
        // The G harp is a low harp — hole-1 blow sits below C4.
        let Harmonica::Diatonic { layout: Some(l), .. } = richter_harp("G") else {
            panic!("expected diatonic");
        };
        assert_eq!(l.blow.unwrap()[0], "G3");
    }

    #[test]
    fn technique_cycles_both_ways_through_all_six() {
        let start = Technique::Blow;
        let mut t = start;
        for _ in 0..6 {
            t = t.next();
        }
        assert_eq!(t, start, "six steps forward returns to the start");
        assert_eq!(start.next().prev(), start);
        assert_eq!(Technique::Blow.prev(), Technique::Over);
    }

    #[test]
    fn target_note_reads_the_right_technique_off_the_harp() {
        let harp = richter_harp("C");
        // Hole 1: blow C4, draw D4, single ½-step bend C#4, overblow D#4.
        assert_eq!(
            target_note(&harp, TrainerTarget { hole: 1, technique: Technique::Blow }).as_deref(),
            Some("C4")
        );
        assert_eq!(
            target_note(&harp, TrainerTarget { hole: 1, technique: Technique::Bend1 }).as_deref(),
            Some("C#4")
        );
        // Hole 5 has no bend (blow E5, draw F5 are a semitone apart).
        assert_eq!(
            target_note(&harp, TrainerTarget { hole: 5, technique: Technique::Bend1 }),
            None
        );
    }

    #[test]
    fn note_freq_hz_matches_concert_pitch() {
        assert!((note_freq_hz("A4").unwrap() - 440.0).abs() < 0.01);
        // One semitone below A4.
        assert!((note_freq_hz("G#4").unwrap() - 415.30).abs() < 0.1);
    }
}
