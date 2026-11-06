// SPDX-License-Identifier: MIT
use super::super::wind_visuals::{self, ClarinetDiagram};
use super::*;
use harmonicon_core::instrument::clarinet::{Clarinet, ClarinetKey};
use harmonicon_platform::settings::MyInstruments;
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, spawn_combobox};

pub(super) fn spawn_setup(
    commands: &mut Commands,
    root: Entity,
    setup: Clarinet,
    loc: &Localization,
) {
    let controls = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
            ..default()
        })
        .id();
    commands.entity(root).add_child(controls);
    let labels = vec![loc.msg("clarinet-bb").to_string(), loc.msg("clarinet-a").to_string()];
    spawn_combobox(
        commands,
        controls,
        root,
        &loc.msg("clarinet-setup"),
        &labels,
        &labels[usize::from(setup.key == ClarinetKey::A)],
        move |ev: On<ComboboxSelect>,
              mut commands: Commands,
              mut trainer: ResMut<NoteTrainer>,
              mut mine: ResMut<MyInstruments>| {
            let key = if ev.index == 1 { ClarinetKey::A } else { ClarinetKey::Bb };
            mine.clarinet_key = Some(key);
            if let Instrument::Clarinet(old) = trainer.instrument {
                if old.key == key {
                    return;
                }
                let index = trainer.practice.index;
                let auto = trainer.practice.auto_advance;
                let landed = trainer.practice.landed;
                trainer.instrument = Clarinet { key }.into();
                trainer.practice = Practice::new(drills(&trainer.instrument, trainer.kind));
                trainer.practice.index = index.min(trainer.practice.drills.len().saturating_sub(1));
                trainer.practice.auto_advance = auto;
                trainer.practice.landed = landed;
                trainer.practice.restored = true;
                commands.entity(root).despawn();
                commands.run_system_cached(layout::setup);
            }
        },
    );
}

pub(crate) fn update(
    commands: Commands,
    trainer: Res<NoteTrainer>,
    loc: Res<Localization>,
    mut slots: Query<(Entity, &mut ClarinetDiagram)>,
) {
    let target = trainer.practice.drill().and_then(|drill| match drill.position {
        PlayPosition::Fingering(at) => Some(at),
        _ => None,
    });
    for (_, mut slot) in &mut slots {
        if slot.last != target {
            slot.last = target;
            slot.initialized = false;
        }
    }
    wind_visuals::rebuild(commands, loc, slots);
}
