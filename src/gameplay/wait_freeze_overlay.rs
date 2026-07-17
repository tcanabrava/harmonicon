// SPDX-License-Identifier: MIT

//! The "play this to continue" prompt shown while `pause_menu::WaitForNoteMode`
//! has frozen gameplay at an unhit note. Without it, a frozen clock and a
//! silent, motionless note highway look exactly like the game has hung —
//! this just tells the player what's being waited on.

use bevy::prelude::*;

use crate::app::AppState;

use super::{GameplayRoot, SongNotes};

/// Set by `tick_clock`: `Some(index into SongNotes::notes)` for the note
/// currently holding gameplay frozen, `None` when nothing is. Also lets
/// `tick_clock` pause/resume the music sink only on the actual freeze/unfreeze
/// transition instead of every single frame while frozen.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct WaitFreezeState(pub Option<usize>);

/// The prompt's text node; hidden whenever [`WaitFreezeState`] is `None`.
#[derive(Component, Default, Clone)]
pub struct WaitFreezePrompt;

/// Spawns the (initially hidden) prompt. Tagged `GameplayRoot` so it's torn
/// down with the rest of the scene. Harmless to spawn in every mode — Jam
/// Session never populates `SongNotes`, so `WaitFreezeState` never becomes
/// `Some` there and the prompt just never shows.
pub fn spawn_wait_freeze_prompt(commands: &mut Commands) {
    commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                top: {Val::Percent(28.0)},
                width: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
            }
            GlobalZIndex(90)
            GameplayRoot
            Children [
                (
                    Text({""})
                    TextFont { font_size: {FontSize::Px(28.0)} }
                    TextColor({Color::srgb(1.0, 0.85, 0.35)})
                    WaitFreezePrompt
                )
            ]
        })
        .insert(Visibility::Hidden);
}

/// Labels the prompt from the note `WaitFreezeState` points at: which hole,
/// and blow (↑) or draw (↓) — the same arrows `gameplay_2d`'s hole map uses.
fn sync_wait_freeze_prompt(
    state: Res<WaitFreezeState>,
    song_notes: Res<SongNotes>,
    mut prompts: Query<(&mut Text, &mut Visibility), With<WaitFreezePrompt>>,
) {
    if !state.is_changed() {
        return;
    }
    let label = state.0.and_then(|i| song_notes.notes.get(i)).map(|note| {
        format!(
            "Play Hole {} {}",
            note.hole,
            if note.is_blow { "\u{2191}" } else { "\u{2193}" }
        )
    });
    for (mut text, mut vis) in &mut prompts {
        match &label {
            Some(s) => {
                *text = Text::new(s.clone());
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

pub struct WaitFreezePlugin;

impl Plugin for WaitFreezePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaitFreezeState>().add_systems(
            Update,
            sync_wait_freeze_prompt.run_if(in_state(AppState::Playing)),
        );
    }
}
