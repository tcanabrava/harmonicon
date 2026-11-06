// SPDX-License-Identifier: MIT

//! The "Note Trainer" choice: which instrument to practise. The harmonica
//! opens the Bending Trainer; the guitar, the flute and the piano open the
//! Note Trainer on that instrument.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_app::app::AppState;
use harmonicon_core::instrument::flute::Flute;
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
    for kind in harmonicon_core::instrument::fretted_strings::FrettedKind::ALL {
        spawn_button(
            &mut commands,
            root,
            &loc.msg(&format!("note-trainer-{}", kind.instrument_kind().slug())),
            move |_: On<Activate>,
                  mine: Res<harmonicon_platform::settings::MyInstruments>,
                  mut trainer: ResMut<NoteTrainer>,
                  mut state: ResMut<NextState<AppState>>| {
                let mut setup = kind.default_setup();
                if let Some(preset) = mine
                    .fretted_tuning(kind)
                    .and_then(|id| setup.tuning_presets().iter().find(|preset| preset.id == id))
                {
                    setup.strings = preset.strings.to_vec();
                }
                setup.capo = mine.fretted_capo(kind).unwrap_or(0).min(setup.frets);
                trainer.choose(setup.into());
                state.set(AppState::NoteTrainer);
            },
        );
    }
    spawn_button(
        &mut commands,
        root,
        &loc.msg("note-trainer-flute"),
        |_: On<Activate>,
         mine: Res<harmonicon_platform::settings::MyInstruments>,
         mut trainer: ResMut<NoteTrainer>,
         mut state: ResMut<NextState<AppState>>| {
            trainer.choose(Flute::new(mine.flute_foot.unwrap_or_default()).into());
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
    spawn_button(
        &mut commands,
        root,
        &loc.msg("instrument-clarinet"),
        |_: On<Activate>,
         mine: Res<harmonicon_platform::settings::MyInstruments>,
         mut trainer: ResMut<NoteTrainer>,
         mut state: ResMut<NextState<AppState>>| {
            trainer.choose(
                harmonicon_core::instrument::clarinet::Clarinet {
                    key: mine.clarinet_key.unwrap_or_default(),
                }
                .into(),
            );
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
