// SPDX-License-Identifier: MIT

//! The "Note Trainer" choice: which instrument to practise. The harmonica
//! opens the Bending Trainer; the guitar, the flute and the piano open the
//! Note Trainer on that instrument.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::AppState;
use harmonicon_core::instrument::InstrumentKind;
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
    for kind in InstrumentKind::ALL.into_iter().filter(|kind| *kind != InstrumentKind::Harmonica) {
        spawn_button(
            &mut commands,
            root,
            &loc.msg(&format!("song-instrument-{}", kind.slug())),
            move |_: On<Activate>,
                  mine: Res<harmonicon_platform::settings::MyInstruments>,
                  mut trainer: ResMut<NoteTrainer>,
                  mut state: ResMut<NextState<AppState>>| {
                trainer.choose(mine.trainer_instrument(kind));
                state.set(AppState::NoteTrainer);
            },
        );
    }
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}
