// SPDX-License-Identifier: MIT

//! Standalone Bending Trainer: the Let's Bend-style harmonica bend diagram + the
//! metronome, with a directly pickable key and an adjustable tempo — no song.
//!
//! It's its own [`AppState::BendingTrainer`](crate::menu::AppState), driving the
//! decoupled [`MetronomeTempo`] and its own copy of the gameplay clock so the
//! metronome ticks without any song loaded. The harp is synthesised for the
//! chosen key (transposed Richter layout) and the diagram is rebuilt whenever
//! the key changes.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::audio_system::midi::{midi_to_note, note_to_midi};
use crate::dialogs::button;
use crate::menu::AppState;
use crate::song::chart::{BendingProfile, DiatonicLayout};
use crate::song::harmonica::Harmonica;

use super::harmonica_overlay::spawn_harmonica_overlay;
use super::metronome_overlay::{MetronomeTempo, spawn_metronome};
use super::{GameplayClock, GameplayRoot};

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
}
