// SPDX-License-Identifier: MIT

//! The "Note Trainer" choice: which instrument to practise. The harmonica
//! opens the Bending Trainer; the guitar, the flute and the piano open the
//! Note Trainer on that instrument.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::AppState;
use harmonicon_core::instrument::flute::Flute;
use harmonicon_core::instrument::guitar::{Guitar, GuitarBody};
use harmonicon_gameplay::gameplay::note_trainer::NoteTrainer;
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
        |_: On<Activate>,
         mut trainer: ResMut<NoteTrainer>,
         mut state: ResMut<NextState<AppState>>| {
            trainer.choose(Guitar::standard(GuitarBody::Electric).into());
            state.set(AppState::NoteTrainer);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("note-trainer-flute"),
        |_: On<Activate>,
         mut trainer: ResMut<NoteTrainer>,
         mut state: ResMut<NextState<AppState>>| {
            trainer.choose(Flute::default().into());
            state.set(AppState::NoteTrainer);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("note-trainer-piano"),
        |_: On<Activate>,
         mut trainer: ResMut<NoteTrainer>,
         mut state: ResMut<NextState<AppState>>| {
            trainer.choose(harmonicon_core::instrument::piano::Piano::grand().into());
            state.set(AppState::NoteTrainer);
        },
    );
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}
