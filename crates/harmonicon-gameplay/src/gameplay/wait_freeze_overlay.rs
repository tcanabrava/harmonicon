// SPDX-License-Identifier: MIT

//! Coaching card at the hit line while gameplay waits for an unhit note.
//! It shows the target and the nearest heard pitch using the judge's tab mapping.

use bevy::prelude::*;

use harmonicon_app::app::AppState;
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::SongNotes;
use super::hud::tab_label;
use super::judge::{heard_tab, nearest_attacked};
use super::state::{ActivePitches, HoleTab, PlayedHarp, ValidHarpNotes};

/// Set by `tick_clock`: `Some(index into SongNotes::notes)` for the note
/// currently holding gameplay frozen, `None` when nothing is. Also lets
/// `tick_clock` pause/resume the music sink only on the actual freeze/unfreeze
/// transition instead of every single frame while frozen.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct WaitFreezeState(pub Option<usize>);

/// The card's root; hidden whenever [`WaitFreezeState`] is `None`.
#[derive(Component, Default, Clone)]
pub struct WaitFreezePrompt;

/// "Play 8↓" — set once per freeze.
#[derive(Component, Default, Clone)]
pub struct WaitFreezeTarget;

/// "hearing 4↑" / "listening…" — refreshed every frame while frozen.
#[derive(Component, Default, Clone)]
pub struct WaitFreezeHeard;

/// Spawns the (initially hidden) card as a child of `parent`, its bottom
/// edge `bottom` above the parent's — each mode passes the offset that puts
/// it just clear of its own hit band. Harmless to spawn in every mode: Jam
/// Session never populates `SongNotes`, so `WaitFreezeState` never becomes
/// `Some` there and the card just never shows.
pub fn spawn_wait_freeze_prompt(commands: &mut Commands, parent: Entity, bottom: Val) {
    // Not `GameplayRoot`: a child of a mode's own root, which the cleanup
    // sweep already despawns recursively (see the same note on
    // `beat_guides`).
    let root = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                left: {Val::Percent(0.0)},
                width: {Val::Percent(100.0)},
                bottom: {bottom},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
            }
            ~{Visibility::Hidden}
            ~{Pickable::IGNORE}
            WaitFreezePrompt
        })
        .id();
    commands.entity(parent).add_child(root);

    commands.entity(root).with_children(|card| {
        card.spawn_empty().apply_scene(bsn! {
            Node {
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
                row_gap: {Val::Px(2.0)},
                padding: {UiRect::axes(Val::Px(18.0), Val::Px(8.0))},
                border: {UiRect::all(Val::Px(1.0))},
            }
            BackgroundColor({Color::srgba(0.05, 0.05, 0.08, 0.92)})
            ~{BorderColor::all(Color::srgba(1.0, 0.85, 0.35, 0.55))}
            Children [
                Text("")
                TextFont { font_size: {FontSize::Px(26.0)} }
                TextColor({Color::srgb(1.0, 0.85, 0.35)})
                WaitFreezeTarget
                --
                Text("")
                TextFont { font_size: {FontSize::Px(14.0)} }
                TextColor({Color::srgb(0.78, 0.80, 0.88)})
                WaitFreezeHeard
            ]
        });
    });
}

/// Shows/hides the card and sets its target line on each freeze/unfreeze.
fn sync_wait_freeze_prompt(
    state: Res<WaitFreezeState>,
    song_notes: Res<SongNotes>,
    loc: Res<Localization>,
    mut roots: Query<&mut Visibility, With<WaitFreezePrompt>>,
    mut targets: Query<&mut Text, With<WaitFreezeTarget>>,
) {
    if !state.is_changed() && !song_notes.is_changed() && !loc.is_changed() {
        return;
    }
    let target = state.0.and_then(|i| song_notes.notes.get(i)).map(|note| {
        String::from(loc.msg_args(
            "gameplay-wait-play",
            &[("tab", tab_label(HoleTab { hole: note.hole, is_blow: note.is_blow }))],
        ))
    });
    for mut vis in &mut roots {
        let wanted = if target.is_some() { Visibility::Visible } else { Visibility::Hidden };
        if *vis != wanted {
            *vis = wanted;
        }
    }
    if let Some(label) = target {
        for mut text in &mut targets {
            if text.0 != label {
                text.0.clone_from(&label);
            }
        }
    }
}

/// The live half: what the mic hears, named as a tab, refreshed every frame
/// while frozen. Of several sounding pitches, the one nearest the target is
/// named — the same rule the judge uses to blame a wrong note, so the two
/// never disagree about what "you played" means.
fn update_wait_freeze_heard(
    state: Res<WaitFreezeState>,
    song_notes: Res<SongNotes>,
    active: Res<ActivePitches>,
    valid: Res<ValidHarpNotes>,
    harp: Res<PlayedHarp>,
    loc: Res<Localization>,
    mut heard_lines: Query<&mut Text, With<WaitFreezeHeard>>,
) {
    if !state.is_changed()
        && !song_notes.is_changed()
        && !active.is_changed()
        && !valid.is_changed()
        && !harp.is_changed()
        && !loc.is_changed()
    {
        return;
    }
    let Some(note) = state.0.and_then(|i| song_notes.notes.get(i)) else {
        return;
    };
    let sounding = active.0.iter().map(|p| p.midi).filter(|m| valid.0.contains(m));
    let heard = note
        .expected_pitch
        .and_then(|expected| nearest_attacked(sounding, expected))
        .and_then(|pitch| heard_tab(pitch, &harp));
    let label = match heard {
        Some(tab) => {
            String::from(loc.msg_args("gameplay-wait-hearing", &[("tab", tab_label(tab))]))
        }
        None => String::from(loc.msg("gameplay-wait-listening")),
    };
    for mut text in &mut heard_lines {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}

pub struct WaitFreezePlugin;

impl Plugin for WaitFreezePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaitFreezeState>().add_systems(
            Update,
            (sync_wait_freeze_prompt, update_wait_freeze_heard)
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
    }
}
