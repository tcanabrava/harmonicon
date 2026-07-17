// SPDX-License-Identifier: MIT

//! Standalone Bending Trainer: the Let's Bend-style harmonica bend diagram + the
//! metronome, with a directly pickable key and an adjustable tempo — no song.
//!
//! It's its own [`AppState::BendingTrainer`](crate::app::AppState), driving the
//! decoupled [`MetronomeTempo`] and its own copy of the gameplay clock so the
//! metronome ticks without any song loaded. The harp is synthesised for the
//! chosen key (transposed Richter layout) and the diagram is rebuilt whenever
//! the key changes.
//!
//! Two columns, the same split `gameplay::jam_session` uses: left has
//! everything but the harmonica itself (title, key control, detect-algorithm
//! picker, ear-training target/Listen, tuner readout, drill toggle, tempo
//! control, hint line); right is entirely the harmonica — the bend diagram
//! plus its technique hint/drill explanation text.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::audio_system::midi::{next_key, prev_key};
use crate::audio_system::pitch_detect::PitchRange;
use crate::audio_system::wav::encode_wav;
use crate::dialogs::algo_picker::{algo_labels, on_algo_selected, spawn_algo_explanation};
use crate::dialogs::button;
use crate::dialogs::combobox;
use crate::localization::LocalizationExt;
use crate::app::AppState;
use crate::profile::{DrillRecord, PlayerProfile};
use crate::settings::AudioSettings;
use crate::song::harmonica::{Harmonica, HoleNotes, hole_notes, richter_harp};

use std::collections::HashSet;

use super::harmonica_overlay::{
    CELL_DEFAULT, DiagramCellTarget, HarpOverlayCell, Row, spawn_harmonica_overlay_selectable,
};
use super::metronome_overlay::{MetronomeTempo, spawn_metronome};
use super::{ActivePitches, GameplayClock, GameplayRoot, PITCH_RANGE_MARGIN_SEMITONES};

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
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Technique {
    Blow,
    Draw,
    Bend1,
    Bend2,
    Bend3,
    Over,
}

/// Every technique row, in diagram order — used to enumerate drill targets.
const ALL_TECHNIQUES: [Technique; 6] = [
    Technique::Blow,
    Technique::Draw,
    Technique::Bend1,
    Technique::Bend2,
    Technique::Bend3,
    Technique::Over,
];

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

    /// Stable name used as the technique half of a `PlayerProfile::drills`
    /// key (`"{hole}:{technique}"`) — separate from [`label`](Self::label),
    /// which is player-facing display text free to change independently of
    /// what's already saved on disk.
    fn storage_key(self) -> &'static str {
        match self {
            Technique::Blow => "blow",
            Technique::Draw => "draw",
            Technique::Bend1 => "bend1",
            Technique::Bend2 => "bend2",
            Technique::Bend3 => "bend3",
            Technique::Over => "over",
        }
    }

    /// Inverse of [`storage_key`](Self::storage_key); `None` for anything
    /// else (e.g. a profile.json hand-edited or from a future version).
    fn from_storage_key(s: &str) -> Option<Self> {
        match s {
            "blow" => Some(Technique::Blow),
            "draw" => Some(Technique::Draw),
            "bend1" => Some(Technique::Bend1),
            "bend2" => Some(Technique::Bend2),
            "bend3" => Some(Technique::Bend3),
            "over" => Some(Technique::Over),
            _ => None,
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
        Self {
            hole: 2,
            technique: Technique::Bend1,
        }
    }
}

/// Maps a diagram [`Row`] to the [`Technique`] it represents — the diagram
/// distinguishes which *wing* a bend/over sits on (`BlowBend`/`DrawBend`,
/// `Overblow`/`Overdraw`) since that determines which reed it's read off;
/// `Technique` doesn't need that distinction (`Technique::note` resolves it
/// from the hole number instead), so several `Row`s collapse to one
/// `Technique`. `None` only for a `Row::*Bend` index outside 0..=2, which
/// never actually appears in [`super::harmonica_overlay`]'s `ROWS` table.
fn row_to_technique(row: Row) -> Option<Technique> {
    match row {
        Row::Blow => Some(Technique::Blow),
        Row::Draw => Some(Technique::Draw),
        Row::BlowBend(0) | Row::DrawBend(0) => Some(Technique::Bend1),
        Row::BlowBend(1) | Row::DrawBend(1) => Some(Technique::Bend2),
        Row::BlowBend(2) | Row::DrawBend(2) => Some(Technique::Bend3),
        Row::BlowBend(_) | Row::DrawBend(_) => None,
        Row::Overblow | Row::Overdraw => Some(Technique::Over),
    }
}

/// Sets the drill/ear-training target from a click on the harmonica diagram
/// — the picker itself, replacing the old hole/technique stepper buttons.
/// Shared across every selectable cell (see `spawn_harmonica_overlay_selectable`);
/// looks up which cell fired via `DiagramCellTarget` on the clicked entity
/// rather than a per-cell closure.
fn on_diagram_cell_clicked(
    ev: On<Pointer<Click>>,
    cells: Query<&DiagramCellTarget>,
    mut target: ResMut<TrainerTarget>,
) {
    let Ok(cell) = cells.get(ev.entity) else {
        return;
    };
    let Some(technique) = row_to_technique(cell.row) else {
        return;
    };
    *target = TrainerTarget {
        hole: cell.hole,
        technique,
    };
}

/// Yellow-borders whichever diagram cell matches the current [`TrainerTarget`]
/// — the visible counterpart of [`on_diagram_cell_clicked`]. Written every
/// frame rather than gated on `target.is_changed()`: the diagram itself is
/// despawned and respawned on every key change (`rebuild_overlay`), and a
/// change-gated system would miss re-applying the border to those fresh
/// cells since the *target* didn't change, only the diagram under it.
pub fn update_selected_cell_border(
    target: Res<TrainerTarget>,
    mut cells: Query<(&DiagramCellTarget, &mut BorderColor)>,
) {
    const SELECTED: Color = Color::srgb(0.95, 0.85, 0.20);
    for (cell, mut border) in &mut cells {
        let selected =
            cell.hole == target.hole && row_to_technique(cell.row) == Some(target.technique);
        let color = if selected { SELECTED } else { Color::NONE };
        let wanted = BorderColor::all(color);
        if *border != wanted {
            *border = wanted;
        }
    }
}

/// The "Target: Hole N Draw" readout.
#[derive(Component)]
pub struct TargetLabel;

/// The "how to physically play this" hint box, kept in step with the target.
#[derive(Component)]
pub struct HintLabel;

/// The live cents-off tuner readout.
#[derive(Component)]
pub struct TunerReadout;

/// The drill's "on/off" readout, plus a running streak/weak-spot summary.
#[derive(Component)]
pub struct DrillLabel;

/// The Drill button's hover explanation; empty while not hovering it.
#[derive(Component)]
pub struct DrillExplanation;

/// What Drill mode actually does, shown only while hovering the button —
/// it's not obvious from the label alone that it's adaptive/weighted.
const DRILL_EXPLANATION: &str = "Auto-picks a random hole + technique, weighted toward whatever you've been missing most. Hold the note in tune to advance \u{2014} it keeps circling back to your weak spots until they're solid.";

/// Practical "how do I actually play this" text for a technique on a given
/// hole. Bends and overs go a different physical direction depending on
/// which side of the harp the hole is on, so both are needed to be accurate:
/// holes 1\u{2013}6 bend (and overblow) by drawing, holes 7\u{2013}10 by blowing.
fn technique_hint(technique: Technique, hole: u8) -> String {
    match technique {
        Technique::Blow => "Blow steadily into the hole \u{2014} no special embouchure needed.".to_string(),
        Technique::Draw => "Draw (inhale) steadily through the hole \u{2014} no special embouchure needed.".to_string(),
        Technique::Bend1 | Technique::Bend2 | Technique::Bend3 => {
            let depth = match technique {
                Technique::Bend1 => "a little",
                Technique::Bend2 => "further",
                _ => "as far as it will go",
            };
            if hole <= 6 {
                format!(
                    "Draw, then lower the back of your tongue and drop your jaw slightly \u{2014} shape your mouth from \"ee\" toward \"oh\" \u{2014} to pull the pitch down {depth} while still drawing."
                )
            } else {
                format!(
                    "Blow, then raise the back of your tongue slightly \u{2014} shape your mouth toward \"ee\" \u{2014} to push the pitch down {depth} while still blowing."
                )
            }
        }
        Technique::Over => match hole {
            1 | 4 | 5 | 6 => "Blow with a tight, controlled embouchure (tongue-blocked, airway narrowed) so the draw reed sounds instead of the blow reed \u{2014} an overblow. Start soft; it takes practice.".to_string(),
            7..=10 => "Draw with a tight, controlled embouchure so the blow reed sounds instead of the draw reed \u{2014} an overdraw. Start soft; it takes practice.".to_string(),
            _ => format!("Hole {hole} doesn't support overblow/overdraw \u{2014} try hole 1, 4, 5, 6, or 7\u{2013}10."),
        },
    }
}

// ── Adaptive drill mode ─────────────────────────────────────────────────────────

/// Per hole/technique hit-rate, used to weight which targets the drill
/// serves up next — misses come back around more often than notes the
/// player already nails.
#[derive(Default, Clone, Copy)]
pub struct DrillStat {
    pub attempts: u32,
    pub hits: u32,
}

impl DrillStat {
    /// Selection weight: never-seen and weak targets are drawn more often;
    /// a target the player consistently hits fades toward the 1.0 floor.
    fn weight(&self) -> f32 {
        if self.attempts == 0 {
            return 2.5;
        }
        let miss_rate = 1.0 - self.hits as f32 / self.attempts as f32;
        1.0 + 3.0 * miss_rate
    }
}

/// Auto-advancing ear-training drill: picks a random hole/technique, waits
/// for the player to sustain it in tune, then moves on. Tracks a running
/// hit rate per target so weak spots come up more often than mastered ones.
#[derive(Resource, Default)]
pub struct DrillState {
    pub enabled: bool,
    pub stats: std::collections::HashMap<(u8, Technique), DrillStat>,
    /// How long the current target has been held in tune, in seconds.
    pub hold_secs: f32,
    /// How long the current target has been active at all, in seconds —
    /// resets the drill to a fresh target if the player gets stuck.
    pub elapsed_secs: f32,
    pub streak: u32,
}

/// A `"{hole}:{technique}"` key for [`PlayerProfile::drills`] — stats aren't
/// keyed by [`TrainerKey`], since the physical skill a (hole, technique) pair
/// drills is the same regardless of which key harp it's practiced on.
fn drill_key(hole: u8, technique: Technique) -> String {
    format!("{hole}:{}", technique.storage_key())
}

/// Snapshot of in-memory drill stats into the flat, string-keyed shape
/// `PlayerProfile::drills` persists.
fn stats_to_profile(
    stats: &std::collections::HashMap<(u8, Technique), DrillStat>,
) -> std::collections::HashMap<String, DrillRecord> {
    stats
        .iter()
        .map(|(&(hole, technique), stat)| {
            (
                drill_key(hole, technique),
                DrillRecord {
                    attempts: stat.attempts,
                    hits: stat.hits,
                },
            )
        })
        .collect()
}

/// Inverse of [`stats_to_profile`], for loading. Entries with a key that
/// doesn't parse (a hand-edited or future-version `profile.json`) are
/// silently dropped rather than failing the whole load.
fn stats_from_profile(
    drills: &std::collections::HashMap<String, DrillRecord>,
) -> std::collections::HashMap<(u8, Technique), DrillStat> {
    drills
        .iter()
        .filter_map(|(key, record)| {
            let (hole_str, tech_str) = key.split_once(':')?;
            let hole: u8 = hole_str.parse().ok()?;
            let technique = Technique::from_storage_key(tech_str)?;
            Some((
                (hole, technique),
                DrillStat {
                    attempts: record.attempts,
                    hits: record.hits,
                },
            ))
        })
        .collect()
}

/// A (hole, technique)'s hit-rate, or `None` if it's never been attempted —
/// kept distinct from a `0.0` accuracy so [`progress_tint`] can tell "never
/// tried" (neutral) apart from "tried and always missed" (red).
fn drill_accuracy(stat: Option<&DrillStat>) -> Option<f32> {
    let stat = stat?;
    if stat.attempts == 0 {
        return None;
    }
    Some(stat.hits as f32 / stat.attempts as f32)
}

/// Idle-cell background color for a (hole, technique)'s drill progress: the
/// diagram's ordinary idle color for "never attempted", blending from a dim
/// red (weak) to a dim green (mastered) as hit-rate climbs — so the same
/// diagram used to pick a drill target also doubles as a progress map, no
/// separate screen needed.
fn progress_tint(accuracy: Option<f32>) -> Color {
    let Some(acc) = accuracy else {
        return CELL_DEFAULT;
    };
    let acc = acc.clamp(0.0, 1.0);
    let weak = Color::srgb(0.32, 0.12, 0.12).to_srgba();
    let strong = Color::srgb(0.14, 0.34, 0.16).to_srgba();
    Color::srgb(
        weak.red + (strong.red - weak.red) * acc,
        weak.green + (strong.green - weak.green) * acc,
        weak.blue + (strong.blue - weak.blue) * acc,
    )
}

/// Tints every idle diagram cell by its drill accuracy (see [`progress_tint`]),
/// layered on top of [`harmonica_overlay::update_harmonica_overlay`]'s live
/// mic highlight — both write `BackgroundColor`, so this must run `.after`
/// it (enforced in `GameplayPlugin::build`) and skips any cell currently lit
/// by a sounding pitch, letting the live highlight win.
pub fn update_drill_progress_tint(
    active: Res<ActivePitches>,
    drill: Res<DrillState>,
    mut cells: Query<(&HarpOverlayCell, &DiagramCellTarget, &mut BackgroundColor)>,
) {
    let played: HashSet<u8> = active.0.iter().map(|p| p.midi).collect();
    for (cell, target, mut bg) in &mut cells {
        if cell.midi.is_some_and(|m| played.contains(&m)) {
            continue;
        }
        let Some(technique) = row_to_technique(target.row) else {
            continue;
        };
        let accuracy = drill_accuracy(drill.stats.get(&(target.hole, technique)));
        *bg = BackgroundColor(progress_tint(accuracy));
    }
}

/// Seconds the player must hold a target in tune before the drill advances.
const DRILL_HOLD_TO_ADVANCE: f32 = 0.5;
/// Seconds before a stuck target is scored as a miss and swapped out.
const DRILL_TIMEOUT_SECS: f32 = 12.0;

/// Every (hole, technique) pair the current harp can actually produce.
fn valid_targets(harp: &Harmonica) -> Vec<TrainerTarget> {
    (1..=10)
        .flat_map(|hole| {
            ALL_TECHNIQUES
                .iter()
                .map(move |&technique| TrainerTarget { hole, technique })
        })
        .filter(|t| target_note(harp, *t).is_some())
        .collect()
}

/// Weighted-random pick of the next drill target, biased toward targets the
/// player has missed more often, and avoiding an immediate repeat of `avoid`
/// when another option exists.
fn pick_next_target(
    harp: &Harmonica,
    stats: &std::collections::HashMap<(u8, Technique), DrillStat>,
    avoid: Option<TrainerTarget>,
) -> Option<TrainerTarget> {
    let mut pool = valid_targets(harp);
    if pool.len() > 1 {
        pool.retain(|&t| Some((t.hole, t.technique)) != avoid.map(|a| (a.hole, a.technique)));
    }
    if pool.is_empty() {
        return None;
    }
    let weights: Vec<f32> = pool
        .iter()
        .map(|t| {
            stats
                .get(&(t.hole, t.technique))
                .copied()
                .unwrap_or_default()
                .weight()
        })
        .collect();
    let total: f32 = weights.iter().sum();
    let mut roll = rand::random_range(0.0..total);
    for (target, w) in pool.iter().zip(weights.iter()) {
        if roll < *w {
            return Some(*target);
        }
        roll -= w;
    }
    pool.last().copied()
}

/// Note name for the current target on `harp`, or `None` if that hole doesn't
/// have that technique (e.g. hole 5 has no bend, most holes have no overblow).
fn target_note(harp: &Harmonica, target: TrainerTarget) -> Option<String> {
    let holes = hole_notes(harp, target.hole);
    target.technique.note(&holes).map(str::to_string)
}

/// Frequency in Hz for a note label like `"C#4"`.
fn note_freq_hz(note: &str) -> Option<f32> {
    crate::audio_system::midi::note_to_freq_hz(note)
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

/// The pitch detector's search range for `key`'s transposed Richter harp,
/// widened by a semitone margin — the trainer's own key-derived range, kept
/// separate from a loaded chart's (see `setup_scoring_config` in `mod.rs`).
fn pitch_range_for_key(key: &str) -> PitchRange {
    richter_harp(key)
        .frequency_range()
        .map(|(lo, hi)| PitchRange::from_freqs([lo, hi], PITCH_RANGE_MARGIN_SEMITONES))
        .unwrap_or_default()
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

pub fn setup(
    mut commands: Commands,
    mut clock: ResMut<GameplayClock>,
    mut tempo: ResMut<MetronomeTempo>,
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    audio: Res<AudioSettings>,
    mut pitch_range: ResMut<PitchRange>,
    mut drill: ResMut<DrillState>,
    profile: Res<PlayerProfile>,
    loc: Res<Localization>,
) {
    clock.set_free(0.0);
    *pitch_range = pitch_range_for_key(&key.0);
    tempo.beats_per_bar = 4;
    // Keep whatever BPM was last set; default to a comfortable practice tempo.
    if tempo.bpm < MIN_BPM || tempo.bpm > MAX_BPM {
        tempo.bpm = 90.0;
    }
    // Restore drill hit-rates from the last session — see `save_drill_progress`,
    // which persists them on the way out.
    drill.stats = stats_from_profile(&profile.drills);

    let mut root_ec = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
        GameplayRoot,
    ));
    // Captured so the Detect-algorithm combobox below can pass it as the
    // *backdrop*'s parent — `combobox::spawn_combobox` requires a
    // full-screen-sized backdrop parent for its click-catching backdrop to
    // size correctly (see its module doc comment), so a click anywhere on
    // the right column (not just the left) still dismisses an open dropdown.
    // The combobox's visible trigger, meanwhile, is parented to the left
    // column itself so it sits in that column's normal vertical flow.
    let root_id = root_ec.id();
    root_ec.with_children(|root| {
            // ── Left half: everything but the harmonica itself ───────────────
            let mut left_ec = root.spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            });
            let left_id = left_ec.id();
            left_ec.with_children(|left| {
            left.spawn((
                Text::new(String::from(loc.msg("bending-trainer"))),
                TextFont { font_size: FontSize::Px(26.0), ..default() },
                TextColor(Color::WHITE),
            ));

            // ── Key control: ◂  Key: C  ▸ ───────────────────────────────────
            left.spawn(Node {
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

            // ── Detect algorithm: combobox (same global AudioSettings::
            // pitch_algorithm the Options page drives) + its explanation ────
            combobox::spawn_combobox(
                left.commands_mut(),
                left_id,
                root_id,
                "Detect",
                &algo_labels(),
                audio.pitch_algorithm.label(),
                on_algo_selected,
            );
            spawn_algo_explanation(left.commands_mut(), left_id, 420.0, audio.pitch_algorithm);

            // ── Ear-training target: readout (set by clicking the diagram
            // below, not stepper buttons — see `on_diagram_cell_clicked`) +
            // Listen ────────────────────────────────────────────────────────
            left.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node { width: Val::Px(220.0), justify_content: JustifyContent::Center, ..default() },
                    Text::new(target_label_text(target.hole, target.technique)),
                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                    TextColor(Color::srgb(0.80, 0.90, 0.95)),
                    TargetLabel,
                ));
                row.spawn_empty().apply_scene(button::small(
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

            left.spawn((
                Text::new(""),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(Color::srgb(0.55, 0.85, 0.60)),
                TunerReadout,
            ));

            // ── Adaptive drill toggle ────────────────────────────────────────
            left.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn_empty()
                    .apply_scene(button::small(
                        "\u{1F3B2} Drill",
                        |_: On<Pointer<Click>>,
                         key: Res<TrainerKey>,
                         mut target: ResMut<TrainerTarget>,
                         mut drill: ResMut<DrillState>| {
                            drill.enabled = !drill.enabled;
                            drill.hold_secs = 0.0;
                            drill.elapsed_secs = 0.0;
                            if drill.enabled {
                                let harp = richter_harp(&key.0);
                                if let Some(next) = pick_next_target(&harp, &drill.stats, Some(*target)) {
                                    *target = next;
                                }
                            }
                        },
                    ))
                    .observe(show_drill_explanation)
                    .observe(hide_drill_explanation);
                row.spawn((
                    Text::new(String::from(loc.msg("bending-drill-off"))),
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(Color::srgb(0.70, 0.70, 0.80)),
                    DrillLabel,
                ));
            });

            // ── Tempo control: −  ♩ = NN (in the metronome)  + ──────────────
            left.spawn(Node {
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

            left.spawn((
                Text::new(String::from(loc.msg("bending-hint"))),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));

            // A visible Back — this screen has no pause menu (unlike a
            // song/Jam Session), so Escape was the only way out; a touch
            // target matters for a future mobile build with no keyboard.
            left.spawn_empty().apply_scene(button::small(
                &String::from(loc.msg("back")),
                |_: On<Pointer<Click>>,
                 mut next_state: ResMut<NextState<AppState>>,
                 mut ret_play: ResMut<crate::app::ReturnToPlay>| {
                    ret_play.0 = true;
                    next_state.set(AppState::Menu);
                },
            ));
            });

            // ── Right half: the harmonica — bend diagram + its explanatory
            // text, the same grouping `jam_session::setup` uses for its own
            // harmonica column.
            root.spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            })
            .with_children(|right| {
                // The bend diagram (rebuilt on key change).
                right.spawn((Node::default(), OverlayHost))
                    .with_children(|host| {
                        spawn_harmonica_overlay_selectable(
                            host,
                            &richter_harp(&key.0),
                            on_diagram_cell_clicked,
                            &loc,
                        );
                    });

                right.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(280.0),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Node { padding: UiRect::all(Val::Px(8.0)), ..default() },
                        BackgroundColor(Color::srgba(0.10, 0.10, 0.14, 0.85)),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Text::new(technique_hint(target.technique, target.hole)),
                            TextFont { font_size: FontSize::Px(15.0), ..default() },
                            TextColor(Color::srgb(0.75, 0.75, 0.85)),
                            HintLabel,
                        ));
                    });
                    col.spawn((
                        Text::new(""),
                        TextFont { font_size: FontSize::Px(15.0), ..default() },
                        TextColor(Color::srgb(0.60, 0.60, 0.70)),
                        DrillExplanation,
                    ));
                });
            });
        });
}

/// Advance the trainer's own clock (no song to drive it).
pub fn tick_clock(mut clock: ResMut<GameplayClock>, time: Res<Time>) {
    clock.advance(time.delta_secs_f64(), None);
}

/// Rebuild the bend diagram when the key changes.
pub fn rebuild_overlay(
    key: Res<TrainerKey>,
    hosts: Query<(Entity, Option<&Children>), With<OverlayHost>>,
    mut commands: Commands,
    loc: Res<Localization>,
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
        commands.entity(host).with_children(|h| {
            spawn_harmonica_overlay_selectable(h, &harp, on_diagram_cell_clicked, &loc);
        });
    }
}

/// Re-derive the pitch detector's range when the key changes.
pub fn update_pitch_range(key: Res<TrainerKey>, mut pitch_range: ResMut<PitchRange>) {
    if !key.is_changed() {
        return;
    }
    *pitch_range = pitch_range_for_key(&key.0);
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

/// Esc returns to the menu — specifically the Play page, where "Bending
/// Trainer" lives, rather than `MenuPage`'s own default of Main.
pub fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut ret_play: ResMut<crate::app::ReturnToPlay>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        ret_play.0 = true;
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

/// Keep the how-to-play hint in step with the chosen hole/technique.
pub fn update_hint_label(
    target: Res<TrainerTarget>,
    mut labels: Query<&mut Text, With<HintLabel>>,
) {
    if !target.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(technique_hint(target.technique, target.hole));
    }
}

/// Show what Drill mode does while the button is hovered.
fn show_drill_explanation(
    _: On<Pointer<Over>>,
    mut labels: Query<&mut Text, With<DrillExplanation>>,
) {
    for mut text in &mut labels {
        *text = Text::new(DRILL_EXPLANATION);
    }
}

/// Hide the Drill explanation once the pointer leaves the button.
fn hide_drill_explanation(
    _: On<Pointer<Out>>,
    mut labels: Query<&mut Text, With<DrillExplanation>>,
) {
    for mut text in &mut labels {
        *text = Text::new("");
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
    loc: Res<Localization>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TunerReadout>>,
) {
    let Ok((mut text, mut color)) = labels.single_mut() else {
        return;
    };
    let harp = richter_harp(&key.0);
    let Some(target_note) = target_note(&harp, *target) else {
        *text = Text::new(String::from(loc.msg("bending-no-note-for-technique")));
        color.0 = Color::srgb(0.60, 0.60, 0.65);
        return;
    };
    let Some(target_freq) = note_freq_hz(&target_note) else {
        return;
    };

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
        *text = Text::new(format!(
            "\u{2191} {cents:+.0} cents sharp  (target {target_note})"
        ));
        color.0 = Color::srgb(0.90, 0.70, 0.30);
    } else {
        *text = Text::new(format!(
            "\u{2193} {cents:+.0} cents flat  (target {target_note})"
        ));
        color.0 = Color::srgb(0.90, 0.70, 0.30);
    }
}

/// Drives the adaptive drill while it's on: holds the current target until
/// the player sustains it in tune, credits a hit, and picks the next target,
/// weighted toward whatever's been missed most. A target the player can't
/// land within [`DRILL_TIMEOUT_SECS`] is scored a miss so the drill keeps
/// moving instead of stalling on one hole.
pub fn drill_update(
    key: Res<TrainerKey>,
    mut target: ResMut<TrainerTarget>,
    active: Res<ActivePitches>,
    mut drill: ResMut<DrillState>,
    time: Res<Time>,
) {
    if !drill.enabled {
        return;
    }
    let harp = richter_harp(&key.0);
    let Some(target_note) = target_note(&harp, *target) else {
        return;
    };
    let Some(target_freq) = note_freq_hz(&target_note) else {
        return;
    };

    let dt = time.delta_secs();
    drill.elapsed_secs += dt;

    let in_tune = active
        .0
        .iter()
        .any(|p| (1200.0 * (p.frequency / target_freq).log2()).abs() <= IN_TUNE_CENTS);
    drill.hold_secs = if in_tune { drill.hold_secs + dt } else { 0.0 };

    let hit = drill.hold_secs >= DRILL_HOLD_TO_ADVANCE;
    let timed_out = drill.elapsed_secs >= DRILL_TIMEOUT_SECS;
    if !hit && !timed_out {
        return;
    }

    let stat = drill
        .stats
        .entry((target.hole, target.technique))
        .or_default();
    stat.attempts += 1;
    if hit {
        stat.hits += 1;
        drill.streak += 1;
    } else {
        drill.streak = 0;
    }

    if let Some(next) = pick_next_target(&harp, &drill.stats, Some(*target)) {
        *target = next;
    }
    drill.hold_secs = 0.0;
    drill.elapsed_secs = 0.0;
}

/// Persists the session's drill hit-rates to `profile.json` on the way out
/// of the trainer — paired with `setup`'s restore on the way in. Saved once
/// per visit rather than on every drill-target completion, the same
/// "meaningful lifecycle boundary" policy `results::setup` uses for song
/// bests (see `profile.rs`'s module doc comment).
pub fn save_drill_progress(drill: Res<DrillState>, mut profile: ResMut<PlayerProfile>) {
    profile.drills = stats_to_profile(&drill.stats);
    crate::profile::save_profile(&profile);
}

/// Keeps the "Drill: ..." readout in step with on/off state and streak.
pub fn update_drill_label(
    drill: Res<DrillState>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<DrillLabel>>,
) {
    if !drill.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(String::from(if drill.enabled {
            loc.msg_args("bending-drill-on", &[("streak", drill.streak.to_string())])
        } else {
            loc.msg("bending-drill-off")
        }));
    }
}

#[cfg(test)]
mod tests;
