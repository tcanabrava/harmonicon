// SPDX-License-Identifier: MIT
use super::super::bowed_visuals::FingerboardView;
use super::*;
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, spawn_combobox};
#[derive(Component)]
pub(crate) struct StringChoices;

pub(crate) fn update(
    mut commands: Commands,
    trainer: Res<NoteTrainer>,
    loc: Res<Localization>,
    mut views: Query<&mut FingerboardView>,
    controls: Query<(Entity, &ChildOf), With<StringChoices>>,
    roots: Query<Entity, With<GameplayRoot>>,
    mut last: Local<Option<(Entity, usize, TechniqueKind)>>,
) {
    let Instrument::Bowed(setup) = &trainer.instrument else { return };
    for mut view in &mut views {
        view.targets = trainer
            .practice
            .drill()
            .and_then(|drill| match drill.position {
                PlayPosition::Fingerboard(at) => Some(at),
                _ => None,
            })
            .into_iter()
            .collect();
        view.heard = trainer.practice.heard_midi.into_iter().collect();
    }
    let Ok((host, _)) = controls.single() else { return };
    if *last == Some((host, trainer.practice.index, trainer.kind)) {
        return;
    }
    let Ok(root) = roots.single() else { return };
    *last = Some((host, trainer.practice.index, trainer.kind));
    let Some(drill) = trainer.practice.drill() else { return };
    let choices: Vec<_> = trainer
        .practice
        .drills
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.target == drill.target)
        .filter_map(|(index, candidate)| match candidate.position {
            PlayPosition::Fingerboard(at) => Some((index, at)),
            _ => None,
        })
        .collect();
    let labels: Vec<_> = choices
        .iter()
        .map(|(_, at)| {
            loc.msg_args(
                "bowed-string-option",
                &[
                    ("string", at.string.to_string()),
                    (
                        "note",
                        setup
                            .open_pitch(at.string)
                            .map(|pitch| note_name(f32::from(pitch)))
                            .unwrap_or_default(),
                    ),
                ],
            )
            .to_string()
        })
        .collect();
    let selected =
        choices.iter().position(|(index, _)| *index == trainer.practice.index).unwrap_or(0);
    commands.entity(host).despawn_children();
    if !labels.is_empty() {
        spawn_combobox(
            &mut commands,
            host,
            root,
            &loc.msg("bowed-string-choice"),
            &labels,
            &labels[selected],
            move |ev: On<ComboboxSelect>, mut trainer: ResMut<NoteTrainer>| {
                if let Some(&(index, _)) = choices.get(ev.index) {
                    trainer.practice.index = index;
                    trainer.practice.step_by(0);
                    trainer.practice.auto_advance = false;
                    trainer.practice.restored = true;
                }
            },
        );
    }
}
