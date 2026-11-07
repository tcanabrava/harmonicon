// SPDX-License-Identifier: MIT
//! Explicit pitch and physical guidance controls, without guitar fret/capo tools.
use super::state::{EditorState, Mode};
use bevy::prelude::*;
use harmonicon_core::instrument::Instrument;
use harmonicon_core::instrument::bowed_strings::{BowArticulation, BowDirection};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, ComboboxValue, spawn_combobox};

#[derive(Component)]
pub(super) struct BowedControls;
#[derive(Component, Clone, Copy)]
pub(super) enum Field {
    Pitch,
    Finger,
    Hand,
    Direction,
    Articulation,
}

fn index(state: &EditorState, field: Field, low: u8) -> usize {
    let note = state.selected_note();
    let at = note.and_then(super::bowed_note::at);
    let bow =
        note.and_then(|note| note.performance.bowed()).or(state.bowed.bow).unwrap_or_default();
    match field {
        Field::Pitch => at
            .map(|at| at.note)
            .or(state.bowed.pitch)
            .map_or(0, |pitch| usize::from(pitch.saturating_sub(low)) + 1),
        Field::Finger => at.and_then(|at| at.finger).map_or(0, |finger| usize::from(finger) + 1),
        Field::Hand => usize::from(at.and_then(|at| at.hand_position).unwrap_or(0)),
        Field::Direction => match bow.direction {
            None => 0,
            Some(BowDirection::Down) => 1,
            Some(BowDirection::Up) => 2,
        },
        Field::Articulation => match bow.articulation {
            BowArticulation::Detache => 0,
            BowArticulation::Legato => 1,
            BowArticulation::Staccato => 2,
        },
    }
}
fn options(loc: &Localization, field: Field, range: (u8, u8)) -> Vec<String> {
    let unset = || loc.msg("bowed-unspecified").to_string();
    match field {
        Field::Pitch => std::iter::once(loc.msg("bowed-open-clicked").to_string())
            .chain(
                (range.0..=range.1)
                    .map(|pitch| harmonicon_core::midi::midi_to_note(i32::from(pitch))),
            )
            .collect(),
        Field::Finger => {
            std::iter::once(unset()).chain((0..=4).map(|finger| finger.to_string())).collect()
        }
        Field::Hand => {
            std::iter::once(unset()).chain((1..=7).map(|hand| hand.to_string())).collect()
        }
        Field::Direction => ["bowed-unspecified", "bowed-down", "bowed-up"]
            .map(|key| loc.msg(key).to_string())
            .to_vec(),
        Field::Articulation => ["bowed-detache", "bowed-legato", "bowed-staccato"]
            .map(|key| loc.msg(key).to_string())
            .to_vec(),
    }
}

pub(super) fn sync(
    mut commands: Commands,
    state: Res<EditorState>,
    loc: Res<Localization>,
    slots: Query<(Entity, Option<&Children>), With<BowedControls>>,
    roots: Query<Entity, With<super::ui::EditorRoot>>,
    mut nodes: Query<&mut Node, With<BowedControls>>,
    mut fields: Query<(&Field, &mut ComboboxValue)>,
    mut last: Local<Option<(Entity, Vec<u8>)>>,
) {
    use harmonicon_core::instrument::InstrumentModel;
    let Ok((slot, children)) = slots.single() else { return };
    let instrument = state.effective_instrument();
    let setup = match &instrument {
        Instrument::Bowed(strings) => Some(strings),
        _ => None,
    };
    if let Ok(mut node) = nodes.get_mut(slot) {
        node.display = if setup.is_some() { Display::Flex } else { Display::None };
    }
    let Some(setup) = setup else { return };
    let Some(range) = instrument.range() else { return };
    if children.is_some()
        && last.as_ref() == Some(&(slot, setup.strings.clone()))
        && !loc.is_changed()
    {
        for (field, mut value) in &mut fields {
            let options = options(&loc, *field, range);
            if let Some(label) = options.get(index(&state, *field, range.0))
                && value.0 != *label
            {
                value.0.clone_from(label);
            }
        }
        return;
    }
    let Ok(root) = roots.single() else { return };
    *last = Some((slot, setup.strings.clone()));
    commands.entity(slot).despawn_children();
    for (field, label) in [
        (Field::Pitch, "bowed-pitch"),
        (Field::Finger, "bowed-finger"),
        (Field::Hand, "bowed-hand"),
        (Field::Direction, "bowed-direction"),
        (Field::Articulation, "bowed-articulation"),
    ] {
        let choices = options(&loc, field, range);
        let current = choices[index(&state, field, range.0)].clone();
        let combo = spawn_combobox(
            &mut commands,
            slot,
            root,
            &loc.msg(label),
            &choices,
            &current,
            move |ev: On<ComboboxSelect>,
                  mut state: ResMut<EditorState>,
                  mut feedback: ResMut<super::save_feedback::SaveFeedback>,
                  loc: Res<Localization>| {
                if state.mode != Mode::Edit {
                    return;
                }
                let ok = match field {
                    Field::Pitch => {
                        if ev.index == 0 {
                            super::bowed_note::set_open(&mut state)
                        } else {
                            super::bowed_note::set_pitch(&mut state, range.0 + (ev.index - 1) as u8)
                        }
                    }
                    Field::Finger => {
                        !state.selected.is_empty()
                            && super::bowed_note::set_hints(
                                &mut state,
                                Some(ev.index.checked_sub(1).map(|index| index as u8)),
                                None,
                            )
                    }
                    Field::Hand => {
                        !state.selected.is_empty()
                            && super::bowed_note::set_hints(
                                &mut state,
                                None,
                                Some((ev.index > 0).then_some(ev.index as u8)),
                            )
                    }
                    Field::Direction | Field::Articulation => {
                        let mut bow = state
                            .selected_note()
                            .and_then(|note| note.performance.bowed())
                            .or(state.bowed.bow)
                            .unwrap_or_default();
                        match field {
                            Field::Direction => {
                                bow.direction = match ev.index {
                                    1 => Some(BowDirection::Down),
                                    2 => Some(BowDirection::Up),
                                    _ => None,
                                }
                            }
                            Field::Articulation => {
                                bow.articulation = match ev.index {
                                    1 => BowArticulation::Legato,
                                    2 => BowArticulation::Staccato,
                                    _ => BowArticulation::Detache,
                                }
                            }
                            _ => {}
                        }
                        let ids = state.selected.clone();
                        for note in state.notes.iter_mut().filter(|note| ids.contains(&note.id)) {
                            note.performance =
                                harmonicon_core::note_performance::PerformanceInstructions::Bowed(
                                    bow,
                                );
                        }
                        state.bowed.bow = Some(bow);
                        true
                    }
                };
                if !ok {
                    feedback.set(loc.msg("bowed-invalid"));
                }
            },
        );
        commands.entity(combo).insert(field);
    }
    commands.entity(slot).with_child((
        Text::new(loc.msg("bowed-editor-help").to_string()),
        TextFont { font_size: FontSize::Px(13.0), ..default() },
    ));
}
