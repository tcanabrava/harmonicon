// SPDX-License-Identifier: MIT
//! Author fretting-hand instructions on the selected note.
use bevy::prelude::*;
use harmonicon_core::chart::FretPosition;
use harmonicon_core::instrument::{Instrument, PlayPosition, guitar::StringTechnique};
use harmonicon_core::note_performance::{Barre, PerformanceInstructions};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, ComboboxValue, spawn_combobox};

use super::state::{EditorState, GridNote, Mode};

#[derive(Component)]
pub(super) struct FrettingControls;

#[derive(Component, Clone, Copy)]
pub(super) enum Field {
    Finger,
    Barre,
}

fn placement(note: &GridNote) -> Option<FretPosition> {
    let Some(PlayPosition::Fret(at)) = note.target.position() else { return None };
    (at.fret > 0 && at.technique != StringTechnique::Harmonic)
        .then_some(FretPosition { string: at.string, fret: at.fret })
}

fn spans(at: FretPosition, count: u8) -> Vec<Barre> {
    (1..=at.string)
        .flat_map(|first| {
            (first.saturating_add(1).max(at.string)..=count).filter_map(move |last| {
                (first < last).then_some(Barre {
                    fret: at.fret,
                    first_string: first,
                    last_string: last,
                })
            })
        })
        .collect()
}

fn apply(state: &mut EditorState, field: Field, index: usize) {
    if state.mode != Mode::Edit {
        return;
    }
    let Instrument::Fretted(model) = state.effective_instrument() else { return };
    let count = model.neck().string_count();
    let Some(id) = state.selected.last().copied() else { return };
    let Some(note) = state.notes.iter_mut().find(|note| note.id == id) else { return };
    let Some(at) = placement(note) else { return };
    let mut hint = note
        .performance
        .fretting()
        .filter(|hint| hint.valid_for(at, count.into()))
        .unwrap_or_default();
    match field {
        Field::Finger => {
            if index > 4 {
                return;
            }
            hint.finger = (index != 0).then_some(index as u8);
            if hint.finger != Some(1) {
                hint.barre = None;
            }
        }
        Field::Barre => {
            hint.barre = match index.checked_sub(1) {
                Some(index) => {
                    let Some(barre) = spans(at, count).get(index).copied() else { return };
                    hint.finger = Some(1);
                    Some(barre)
                }
                None => None,
            };
        }
    }
    note.performance = PerformanceInstructions::from_fretting(Some(hint));
}

fn choices(loc: &Localization, field: Field, at: FretPosition, count: u8) -> Vec<String> {
    match field {
        Field::Finger => [
            "bowed-unspecified",
            "editor-finger-index",
            "editor-finger-middle",
            "editor-finger-ring",
            "editor-finger-little",
        ]
        .map(|key| loc.msg(key).to_string())
        .to_vec(),
        Field::Barre => std::iter::once(loc.msg("editor-no-barre").to_string())
            .chain(spans(at, count).into_iter().map(Barre::label))
            .collect(),
    }
}

pub(super) fn sync(
    mut commands: Commands,
    state: Res<EditorState>,
    loc: Res<Localization>,
    roots: Query<Entity, With<super::ui::EditorRoot>>,
    mut slots: Query<(Entity, &mut Node), With<FrettingControls>>,
    mut fields: Query<(&Field, &mut ComboboxValue)>,
    mut prior: Local<Option<(Entity, FretPosition, u8, Entity)>>,
) {
    let Ok((slot, mut node)) = slots.single_mut() else { return };
    if !state.is_changed() && !loc.is_changed() && prior.is_some_and(|entry| entry.0 == slot) {
        return;
    }
    let selection = state.selected_note().and_then(|note| {
        let Instrument::Fretted(model) = state.effective_instrument() else { return None };
        let at = placement(note)?;
        (state.mode == Mode::Edit).then_some((
            at,
            model.neck().string_count(),
            note.performance.fretting(),
        ))
    });
    let Some((at, count, hint)) = selection else {
        node.display = Display::None;
        if let Some((previous_slot, _, _, host)) = prior.take().filter(|entry| entry.0 == slot) {
            commands.entity(previous_slot).despawn_related::<Children>();
            commands.entity(host).despawn();
        }
        return;
    };
    node.display = Display::Flex;
    let hint = hint.filter(|hint| hint.valid_for(at, count.into())).unwrap_or_default();
    let index = |field| match field {
        Field::Finger => usize::from(hint.finger.unwrap_or(u8::from(hint.barre.is_some()))),
        Field::Barre => hint
            .barre
            .and_then(|barre| spans(at, count).iter().position(|candidate| *candidate == barre))
            .map_or(0, |index| index + 1),
    };
    if prior.is_some_and(|entry| (entry.0, entry.1, entry.2) == (slot, at, count))
        && !loc.is_changed()
    {
        for (field, mut value) in &mut fields {
            let label = &choices(&loc, *field, at, count)[index(*field)];
            if value.0 != *label {
                value.0.clone_from(label);
            }
        }
        return;
    }
    let Ok(root) = roots.single() else { return };
    if let Some((_, _, _, host)) = prior.take().filter(|entry| entry.0 == slot) {
        commands.entity(host).despawn();
    }
    commands.entity(slot).despawn_related::<Children>();
    let host = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(host);
    *prior = Some((slot, at, count, host));
    for (field, key) in [(Field::Finger, "editor-fretting-finger"), (Field::Barre, "editor-barre")]
    {
        let options = choices(&loc, field, at, count);
        let combo = spawn_combobox(
            &mut commands,
            slot,
            host,
            &loc.msg(key),
            &options,
            &options[index(field)],
            move |ev: On<ComboboxSelect>, mut state: ResMut<EditorState>| {
                apply(&mut state, field, ev.index)
            },
        );
        commands.entity(combo).insert(field);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::guitar::{Guitar, GuitarBody};

    #[test]
    fn applying_a_barre_to_a_selected_note_keeps_its_pitch_and_fret() {
        let mut state = EditorState {
            instrument: super::super::state::InstrumentSelection::Configured {
                instrument: Guitar::standard(GuitarBody::Electric).into(),
                harmonica_origin: None,
            },
            sticky_fret: 1,
            ..Default::default()
        };
        super::super::guitar_note::place(&mut state, 1, 0, 12);
        let note = *state.selected_note().unwrap();
        let at = placement(&note).unwrap();
        let index = spans(at, 6).iter().position(|span| span.last_string == 6).unwrap() + 1;
        apply(&mut state, Field::Barre, index);
        let selected = state.selected_note().unwrap();
        assert_eq!(selected.target, note.target);
        let hint = selected.performance.fretting().unwrap();
        assert_eq!(hint.finger, Some(1));
        assert_eq!(hint.barre.unwrap().label(), "CI (1–6)");
        apply(&mut state, Field::Finger, 3);
        let hint = state.selected_note().unwrap().performance.fretting().unwrap();
        assert_eq!(hint.finger, Some(3));
        assert!(hint.barre.is_none());
    }
}
