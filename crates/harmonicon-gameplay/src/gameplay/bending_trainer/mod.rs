// SPDX-License-Identifier: MIT

//! Standalone harmonica bending practice with a live tuner, drill, and metronome.
//! The selected key owns a transposed Richter harp shared by trainer systems.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::AppState;
use harmonicon_app::profile::{DrillRecord, PlayerProfile};
use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::{PITCH_RANGE_MARGIN_SEMITONES, PitchRange};
use harmonicon_core::harmonica::{Harmonica, HoleNotes, hole_notes, richter_harp};
use harmonicon_core::midi::{NOTE_NAMES, note_to_midi};
use harmonicon_core::wav::encode_wav;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::BendingTrainerSettings;
use harmonicon_ui::dialogs::algo_picker::{algo_labels, attach_algo_tooltip, on_algo_selected};
use harmonicon_ui::dialogs::button;
use harmonicon_ui::dialogs::button::BaseButtonColor;
use harmonicon_ui::dialogs::combobox;
use harmonicon_ui::dialogs::combobox::ComboboxSelect;
use harmonicon_ui::dialogs::tooltip::Tooltip;

use std::collections::HashSet;

use super::harmonica_overlay::{
    CELL_DEFAULT, CELL_LIT, DiagramCellTarget, HarpOverlayCell, Row,
    spawn_harmonica_overlay_selectable,
};
use super::metronome_overlay::{MetronomeTempo, spawn_metronome};
use super::{ActivePitches, GameplayClock, GameplayRoot};
use harmonicon_ui::dialogs::page_chrome::{header_scene, spawn_back_button, title_column_scene};

const MIN_BPM: f32 = 40.0;
const MAX_BPM: f32 = 220.0;
const BPM_STEP: f32 = 5.0;

/// The key the trainer's diagram is currently built for, and the Richter
/// harp in that key. The harp is built once when the key is set rather than
/// by every per-frame system that needs it, and the fields are private so
/// the two can never disagree.
#[derive(Resource)]
pub struct TrainerKey {
    name: String,
    harp: Harmonica,
}

impl TrainerKey {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), harp: richter_harp(name) }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn harp(&self) -> &Harmonica {
        &self.harp
    }

    /// Switches key, rebuilding the harp to match.
    pub fn set(&mut self, name: &str) {
        if self.name != name {
            *self = Self::new(name);
        }
    }
}

impl Default for TrainerKey {
    fn default() -> Self {
        Self::new("C")
    }
}

/// The 12 chromatic keys, as combobox option labels — `NOTE_NAMES` itself,
/// stringified.
fn key_labels() -> Vec<String> {
    NOTE_NAMES.iter().map(|s| s.to_string()).collect()
}

/// A combobox `on_select` that writes straight to [`TrainerKey`] — every
/// system that reacts to a key change (`rebuild_overlay`,
/// `update_pitch_range`) already keys off `TrainerKey::is_changed()`, so
/// picking a new key from the dropdown behaves exactly like the old
/// prev/next stepper did.
fn on_key_selected(ev: On<ComboboxSelect>, mut key: ResMut<TrainerKey>) {
    if key.name() != ev.value {
        key.set(&ev.value);
    }
}

/// Wraps the harmonica diagram so it can be despawned + rebuilt on key change.
#[derive(Component)]
pub struct OverlayHost;

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
    fn label_key(self, hole: u8) -> &'static str {
        match self {
            Technique::Blow => "bending-technique-blow",
            Technique::Draw => "bending-technique-draw",
            Technique::Bend1 => "bending-technique-bend-half",
            Technique::Bend2 => "bending-technique-bend-whole",
            Technique::Bend3 => "bending-technique-bend-three-half",
            Technique::Over if hole <= 6 => "bending-technique-overblow",
            Technique::Over => "bending-technique-overdraw",
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
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
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

/// Maps a diagram [`Row`] to the [`Technique`] it represents — the diagram
/// distinguishes which *wing* a bend/over sits on (`BlowBend`/`DrawBend`,
/// `Overblow`/`Overdraw`, since that determines which reed it's read off),
/// but `Technique` resolves that from the hole number instead
/// (`Technique::note`), so several `Row`s collapse to one `Technique`.
/// `None` only for a `Row::*Bend` index outside 0..=2, which never actually
/// appears in [`super::harmonica_overlay`]'s `ROWS` table.
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
/// — the trainer's target picker.
/// Shared across every selectable cell (see `spawn_harmonica_overlay_selectable`);
/// looks up which cell fired via `DiagramCellTarget` on the clicked entity
/// rather than a per-cell closure.
///
/// Under [`DrillScope::Custom`] the same click also toggles the cell's
/// membership in the drill's own pool, which is what makes that scope a
/// *set* the player builds rather than just whatever is selected right now.
/// The diagram is the only sensible place to pick cells, and it already
/// takes clicks — so a second interaction (a modifier chord, a separate
/// edit mode) would have cost discoverability for nothing. Removing a cell
/// leaves the target alone: taking something out of the pool is not a
/// request to go practice it.
// not-a-widget-button: harmonica-diagram cells are plain Nodes in a grid,
// not buttons. The diagram as a whole is one Tab stop and its keyboard path
// is arrow keys plus Enter/Space (`diagram::navigate_diagram`), per
// WAI-ARIA's grid pattern — not fifty individual Tab stops.
fn on_diagram_cell_clicked(
    ev: On<PointerClick>,
    cells: Query<&DiagramCellTarget>,
    mut target: ResMut<TrainerTarget>,
    mut drill: ResMut<DrillState>,
) {
    let Ok(cell) = cells.get(ev.entity) else {
        return;
    };
    let Some(technique) = row_to_technique(cell.row) else {
        return;
    };
    choose_cell(&mut drill, &mut target, TrainerTarget { hole: cell.hole, technique });
}

/// What choosing a diagram cell does, shared by a click and by Enter/Space
/// on the focused diagram (`diagram::navigate_diagram`) so the two can't
/// drift apart.
pub(super) fn choose_cell(drill: &mut DrillState, target: &mut TrainerTarget, cell: TrainerTarget) {
    if drill.scope == DrillScope::Custom && !drill.custom.insert((cell.hole, cell.technique)) {
        drill.custom.remove(&(cell.hole, cell.technique));
        return;
    }
    if *target != cell {
        *target = cell;
    }
}

/// Yellow-borders whichever diagram cell matches the current [`TrainerTarget`]
/// — the visible counterpart of [`on_diagram_cell_clicked`] — and, under
/// [`DrillScope::Custom`], dim-ambers every other cell in the custom pool so
/// the set the player is assembling is visible on the diagram itself rather
/// than only as a count in the scope readout.
///
/// Repaints on target/scope changes or after the diagram is rebuilt.
pub fn update_selected_cell_border(
    target: Res<TrainerTarget>,
    drill: Res<DrillState>,
    mut cells: Query<(Ref<DiagramCellTarget>, &mut BorderColor)>,
) {
    if !target.is_changed() && !drill.is_changed() && !cells.iter().any(|(cell, _)| cell.is_added())
    {
        return;
    }
    const SELECTED: Color = Color::srgb(0.95, 0.85, 0.20);
    const IN_CUSTOM_SCOPE: Color = Color::srgb(0.55, 0.45, 0.16);
    let show_custom = drill.scope == DrillScope::Custom;
    for (cell, mut border) in &mut cells {
        let technique = row_to_technique(cell.row);
        let color = if cell.hole == target.hole && technique == Some(target.technique) {
            SELECTED
        } else if show_custom && technique.is_some_and(|t| drill.custom.contains(&(cell.hole, t))) {
            IN_CUSTOM_SCOPE
        } else {
            Color::NONE
        };
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

#[derive(Resource, Default)]
pub struct NaturalCheck {
    requested: bool,
    hold_secs: f32,
    confirmed: bool,
    /// Per-frame deviation from the table, in cents, of the samples accepted
    /// so far — averaged into the reed's observed centre when the check
    /// confirms (`feedback::observed_center_cents`).
    samples: Vec<f32>,
}

#[derive(Component)]
pub struct NaturalCheckLabel;

/// The drill's "on/off" readout, plus a running streak/weak-spot summary.
#[derive(Component)]
pub struct DrillLabel;

/// The Drill toggle button itself — tagged so [`update_drill_button_visual`]
/// can highlight it while the drill is running, since the adjacent
/// [`DrillLabel`] text alone is easy to miss (a toggle should look pressed,
/// not just say so nearby).
#[derive(Component)]
pub struct DrillToggleButton;

/// Practical "how do I actually play this" text for a technique on a given
/// hole. Bends and overs go a different physical direction depending on
/// which side of the harp the hole is on, so both are needed to be accurate:
/// holes 1\u{2013}6 bend (and overblow) by drawing, holes 7\u{2013}10 by blowing.
fn technique_hint_key(technique: Technique, hole: u8) -> &'static str {
    match technique {
        Technique::Blow => "bending-technique-hint-blow",
        Technique::Draw => "bending-technique-hint-draw",
        Technique::Bend1 if hole <= 6 => "bending-technique-hint-draw-bend-half",
        Technique::Bend2 if hole <= 6 => "bending-technique-hint-draw-bend-whole",
        Technique::Bend3 if hole <= 6 => "bending-technique-hint-draw-bend-three-half",
        Technique::Bend1 => "bending-technique-hint-blow-bend-half",
        Technique::Bend2 => "bending-technique-hint-blow-bend-whole",
        Technique::Bend3 => "bending-technique-hint-blow-bend-three-half",
        Technique::Over => match hole {
            1 | 4 | 5 | 6 => "bending-technique-hint-overblow",
            7..=10 => "bending-technique-hint-overdraw",
            _ => "bending-technique-hint-over-unsupported",
        },
    }
}

fn technique_hint(loc: &Localization, technique: Technique, hole: u8) -> String {
    String::from(loc.msg_args(technique_hint_key(technique, hole), &[("hole", hole.to_string())]))
}

/// The pitch detector's search range for `key`'s transposed Richter harp,
/// widened by a semitone margin — the trainer's own key-derived range, kept
/// separate from a loaded chart's (see `setup_scoring_config` in `mod.rs`).
fn pitch_range_for_harp(harp: &Harmonica) -> PitchRange {
    harp.frequency_range()
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
    settings: Res<BendingTrainerSettings>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    loc: Res<Localization>,
) {
    clock.set_free(0.0);
    *pitch_range = pitch_range_for_harp(key.harp());
    tempo.meter = harmonicon_ui::music_score::MusicScoreMeter::default();
    // The last song's pickup has nothing to do with a drill.
    tempo.lead_beats = 0.0;
    // Keep whatever BPM was last set; default to a comfortable practice tempo.
    if tempo.bpm < MIN_BPM || tempo.bpm > MAX_BPM {
        tempo.bpm = 90.0;
    }
    // Restore drill hit-rates from the last session — see `save_drill_progress`,
    // which persists them on the way out.
    drill.stats = stats_from_profile(&profile.drills);

    let root_id = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            GameplayRoot,
        ))
        .id();

    // Header: title top-left, Back button top-right — the same shared
    // header shape every menu page uses (`menu::scene::header_scene`/
    // `title_column_scene`/`spawn_back_button`), composed directly rather
    // than through `spawn_menu_root` since this screen isn't a menu page
    // and doesn't want its background image/scroll-area/`MenuRoot` cleanup
    // tag. Not separately tagged `GameplayRoot` — despawning `root_id` on
    // exit (`cleanup_gameplay`) already recurses into every child.
    let title_column =
        commands.spawn_scene(title_column_scene(String::from(loc.msg("bending-trainer")))).id();
    let header = commands.spawn_scene(header_scene()).id();
    commands.entity(header).add_child(title_column);
    commands.entity(root_id).add_child(header);
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>,
         mut next_state: ResMut<NextState<AppState>>,
         mut ret_play: ResMut<harmonicon_app::app::ReturnToPlay>| {
            ret_play.0 = true;
            next_state.set(AppState::Menu);
        },
    );

    // Read once here for the first frame's layout; `apply_trainer_orientation`
    // follows it live from then on.
    let orientation = windows
        .single()
        .map(|window| layout::orientation_for(window.width(), window.height()))
        .unwrap_or(layout::TrainerOrientation::Landscape);
    layout::spawn_strip(&mut commands, root_id, &loc, key.name(), &audio, &tempo);
    layout::spawn_body(
        &mut commands,
        root_id,
        &loc,
        key.name(),
        *target,
        &audio,
        &settings,
        orientation,
    );
    commands.entity(root_id).with_children(|root| {
        root.spawn_empty().apply_scene(bsn! {
            Node {
                align_self: {AlignSelf::Center},
                padding: {UiRect::bottom(Val::Px(10.0))},
            }
            Text({String::from(loc.msg("bending-hint"))})
            TextFont { font_size: {FontSize::Px(14.0)} }
            TextColor({Color::srgb(0.55, 0.55, 0.65)})
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
    if !key.is_changed() && !loc.is_changed() {
        return;
    }
    let harp = key.harp();
    for (host, children) in &hosts {
        if let Some(children) = children {
            for &c in children {
                commands.entity(c).despawn();
            }
        }
        commands.entity(host).with_children(|h| {
            spawn_harmonica_overlay_selectable(h, harp, on_diagram_cell_clicked, &loc);
        });
    }
}

/// Re-derive the pitch detector's range when the key changes.
pub fn update_pitch_range(key: Res<TrainerKey>, mut pitch_range: ResMut<PitchRange>) {
    if !key.is_changed() {
        return;
    }
    *pitch_range = pitch_range_for_harp(key.harp());
}

/// Esc returns to the menu — specifically the Play page, where "Bending
/// Trainer" lives, rather than `MenuPage`'s own default of Main.
pub fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut ret_play: ResMut<harmonicon_app::app::ReturnToPlay>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        ret_play.0 = true;
        next_state.set(AppState::Menu);
    }
}

/// "Target: Hole 2 · ½-step bend" — or a note that the current harp can't
/// actually produce there, so the reader knows why Listen did nothing.
fn target_label_text(loc: &Localization, hole: u8, technique: Technique) -> String {
    let technique = loc.msg(technique.label_key(hole));
    String::from(loc.msg_args(
        "bending-target-label",
        &[("hole", hole.to_string()), ("technique", technique.to_string())],
    ))
}

/// Keep the "Target: ..." readout in step with the chosen hole/technique.
pub fn update_target_label(
    target: Res<TrainerTarget>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<TargetLabel>>,
) {
    if !target.is_changed() && !loc.is_changed() {
        return;
    }
    let label = target_label_text(&loc, target.hole, target.technique);
    for mut text in &mut labels {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}

/// Keep the how-to-play hint in step with the chosen hole/technique.
pub fn update_hint_label(
    target: Res<TrainerTarget>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<HintLabel>>,
) {
    if !target.is_changed() && !loc.is_changed() {
        return;
    }
    let hint = technique_hint(&loc, target.technique, target.hole);
    for mut text in &mut labels {
        if text.0 != hint {
            text.0.clone_from(&hint);
        }
    }
}

mod advanced;
mod diagram;
mod drill;
mod feedback;
mod gesture;
mod layout;
#[cfg(test)]
mod tests;
mod trace;

pub use advanced::*;
pub use diagram::*;
pub use drill::*;
pub use feedback::*;
pub use gesture::*;
pub use layout::*;
pub use trace::*;
