// SPDX-License-Identifier: MIT

//! The "Note Trainer" choice: which instrument to practise. The harmonica
//! opens the Bending Trainer; the guitar opens its own Note Trainer.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::AppState;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root};

pub(crate) fn setup_note_trainer_menu(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let (root, header, _page_root) =
        spawn_menu_root(&mut commands, &loc.msg("note-trainer"), None, &theme, "NoteTrainerMenu");
    spawn_button(
        &mut commands,
        root,
        &loc.msg("note-trainer-harmonica"),
        |_: On<Activate>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::BendingTrainer)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("note-trainer-guitar"),
        |_: On<Activate>, mut state: ResMut<NextState<AppState>>| state.set(AppState::NoteTrainer),
    );
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}
