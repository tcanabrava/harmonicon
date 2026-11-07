// SPDX-License-Identifier: MIT
//! Clarinet setup preserves sounding notes and rejects an invalid range change.
#[cfg(test)]
use super::instrument_ui::EditorInstrumentKind;
use super::state::{EditorState, InstrumentSelection, Mode};
use bevy::prelude::*;
use harmonicon_core::instrument::clarinet::{Clarinet, ClarinetKey};
use harmonicon_core::instrument::{Instrument, InstrumentModel};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, ComboboxValue, spawn_combobox};

#[derive(Component)]
pub(super) struct ClarinetSetup;

#[derive(Component)]
pub(super) struct PitchConvention;

pub(super) fn sync_hint(
    state: Res<EditorState>,
    loc: Res<Localization>,
    mut hints: Query<(&mut Node, &mut Text), With<PitchConvention>>,
) {
    let visible = matches!(state.effective_instrument(), Instrument::Clarinet(_));
    for (mut node, mut text) in &mut hints {
        node.display = if visible { Display::Flex } else { Display::None };
        let label = loc.msg("clarinet-editor-help").to_string();
        if text.0 != label {
            text.0 = label;
        }
    }
}

pub(super) fn configure(state: &mut EditorState, key: ClarinetKey) -> bool {
    if state.mode != Mode::Edit || !matches!(state.effective_instrument(), Instrument::Clarinet(_))
    {
        return false;
    }
    let instrument: Instrument = Clarinet { key }.into();
    let mut notes = state.notes.clone();
    for note in &mut notes {
        let Some(pitch) = note.target.sounding_pitch(&state.effective_instrument()) else {
            return false;
        };
        let Some(position) = instrument.positions_for(pitch.round() as u8).into_iter().next()
        else {
            return false;
        };
        note.target = harmonicon_core::note_target::NoteTarget::Positioned(position);
    }
    state.instrument = InstrumentSelection::Configured { instrument, harmonica_origin: None };
    state.notes = notes;
    super::piano_note::reset_window(state);
    true
}

pub(super) fn sync(
    mut commands: Commands,
    state: Res<EditorState>,
    loc: Res<Localization>,
    slots: Query<(Entity, Option<&Children>), With<ClarinetSetup>>,
    roots: Query<Entity, With<super::ui::EditorRoot>>,
    mut nodes: Query<&mut Node, With<ClarinetSetup>>,
    mut last: Local<Option<ClarinetKey>>,
    mut values: Query<&mut ComboboxValue>,
) {
    let Ok((slot, children)) = slots.single() else { return };
    let current = match state.effective_instrument() {
        Instrument::Clarinet(setup) => Some(setup.key),
        _ => None,
    };
    if let Ok(mut node) = nodes.get_mut(slot) {
        node.display = if current.is_some() { Display::Flex } else { Display::None };
    }
    let Some(key) = current else { return };
    let labels = vec![loc.msg("clarinet-bb").to_string(), loc.msg("clarinet-a").to_string()];
    let label = &labels[usize::from(key == ClarinetKey::A)];
    if let Some(children) = children.filter(|_| *last == current && !loc.is_changed()) {
        for child in children {
            if let Ok(mut value) = values.get_mut(*child)
                && value.0 != *label
            {
                value.0.clone_from(label);
            }
        }
        return;
    }
    let Ok(root) = roots.single() else { return };
    *last = current;
    commands.entity(slot).despawn_children();
    spawn_combobox(
        &mut commands,
        slot,
        root,
        &loc.msg("clarinet-setup"),
        &labels,
        label,
        |ev: On<ComboboxSelect>,
         mut state: ResMut<EditorState>,
         mut feedback: ResMut<super::save_feedback::SaveFeedback>,
         loc: Res<Localization>,
         mut values: Query<&mut ComboboxValue>| {
            let key = if ev.index == 1 { ClarinetKey::A } else { ClarinetKey::Bb };
            if !configure(&mut state, key) {
                feedback.set(loc.msg("clarinet-setup-rejected"));
                if let Instrument::Clarinet(setup) = state.effective_instrument()
                    && let Ok(mut value) = values.get_mut(ev.combobox)
                {
                    value.0 = loc
                        .msg(if setup.key == ClarinetKey::Bb {
                            "clarinet-bb"
                        } else {
                            "clarinet-a"
                        })
                        .to_string();
                }
            }
        },
    );
    commands.entity(slot).with_child((
        Text::new(loc.msg("clarinet-editor-help").to_string()),
        TextFont { font_size: FontSize::Px(13.0), ..default() },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::{PlayPosition, wind::FingeringAssignment};
    #[test]
    fn setup_change_preserves_sound_and_rejects_a_note_above_the_new_range() {
        let mut state = super::super::document_picker::fresh(
            super::super::instrument_ui::InstrumentKind::Clarinet.selection(),
        );
        assert_eq!(state.display_rows(), 33);
        assert_eq!(state.row_offset, 14);
        super::super::flute_note::place(&mut state, 97 - 58, 0, 12);
        assert_eq!(state.notes.len(), 1);
        assert!(configure(&mut state, ClarinetKey::A));
        assert_eq!(
            state.effective_instrument().pitch_of(&state.notes[0].target.position().unwrap()),
            Some(58)
        );
        assert_eq!(state.row_offset, 15);
        assert!(configure(&mut state, ClarinetKey::Bb));
        state.notes[0].target = harmonicon_core::note_target::NoteTarget::Positioned(
            PlayPosition::Fingering(FingeringAssignment { note: 82, alternate: 0 }),
        );
        let before = state.instrument.clone();
        assert!(!configure(&mut state, ClarinetKey::A));
        assert_eq!(state.instrument, before);
    }
    #[test]
    fn document_round_trip_preserves_key_and_sounding_targets() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/clarinet.harpchart"
        ))
        .unwrap();
        let mut state = EditorState::default();
        super::super::harpchart::load_harpchart(
            &source,
            &mut state,
            &mut super::super::state::Scroll::default(),
        );
        assert_eq!(state.notes.len(), 14);
        assert_eq!(state.display_rows(), 33);
        assert!(state.notes.iter().all(|note| state.row_visible(note.row())));
        let mut saved =
            serde_json::from_str(&super::super::harpchart::serialize_harpchart(&state)).unwrap();
        harmonicon_song::song::validate_and_migrate_chart(&mut saved).unwrap();
        let before: harmonicon_core::chart::HarpChart = serde_json::from_value(source).unwrap();
        let after: harmonicon_core::chart::HarpChart = serde_json::from_value(saved).unwrap();
        assert_eq!(before.instrument(), after.instrument());
        assert_eq!(
            before.track.iter().map(|item| before.event_pitch(&item.events[0])).collect::<Vec<_>>(),
            after.track.iter().map(|item| after.event_pitch(&item.events[0])).collect::<Vec<_>>()
        );
    }
}
