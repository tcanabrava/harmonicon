// SPDX-License-Identifier: MIT
//! Guitar setup edits are atomic, undoable, and retain authored fret positions.
use super::save_feedback::SaveFeedback;
use super::state::{EditorState, InstrumentSelection, Mode};
use bevy::prelude::*;
use bevy::text::{EditableText, TextEdit};
use bevy::ui_widgets::Activate;
use harmonicon_core::instrument::{Instrument, PlayPosition, guitar::Guitar};
use harmonicon_core::note_target::NoteTarget;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_ui::dialogs::{
    button,
    text_input::{TextInputCommitted, spawn_text_input},
};

#[derive(Component)]
pub(super) struct GuitarSetup;

fn current(state: &EditorState) -> Option<Guitar> {
    match state.effective_instrument() {
        Instrument::Guitar(guitar) => Some(guitar),
        _ => None,
    }
}

/// Retuning changes sounding pitches, not tab positions. Reject the entire
/// edit if it would remove an occupied string or invalidate any existing note.
fn configure(state: &mut EditorState, guitar: Guitar) -> bool {
    if state.mode != Mode::Edit
        || current(state).is_none()
        || !(1..=12).contains(&guitar.strings.len())
        || guitar
            .strings
            .iter()
            .any(|&pitch| pitch < 12 || u16::from(pitch) + u16::from(guitar.frets) > 127)
    {
        return false;
    }
    let instrument = Instrument::Guitar(guitar);
    let mut notes = state.notes.clone();
    for note in &mut notes {
        let Some(at @ PlayPosition::Fret(_)) = note.target.position() else { return false };
        note.target = NoteTarget::Positioned(at);
    }
    if notes.iter().any(|note| !super::guitar_note::fits(&instrument, &notes, note, &[])) {
        return false;
    }
    state.instrument = InstrumentSelection::Configured { instrument, harmonica_origin: None };
    state.notes = notes;
    true
}
fn parse_pitch(text: &str) -> Option<u8> {
    // The chart format uses a single-digit octave. Validate before calling
    // the MIDI parser, whose octave arithmetic assumes a bounded number.
    let text = text.trim();
    let bytes = text.as_bytes();
    if !matches!(bytes, [b'A'..=b'G', b'0'..=b'9'] | [b'A'..=b'G', b'#' | b'b', b'0'..=b'9']) {
        return None;
    }
    harmonicon_core::midi::note_to_midi(text).and_then(|m| u8::try_from(m).ok())
}

fn report(ok: bool, loc: &Localization, feedback: &mut SaveFeedback) {
    if !ok {
        feedback.set(loc.msg("editor-guitar-setup-refused"));
    }
}

fn resize(guitar: &mut Guitar, count: usize) -> bool {
    if !(1..=12).contains(&count) {
        return false;
    }
    while guitar.strings.len() < count {
        let Some(pitch) = guitar.strings.last().and_then(|p| p.checked_sub(5)) else {
            return false;
        };
        guitar.strings.push(pitch.max(12));
    }
    guitar.strings.truncate(count);
    true
}

pub(super) fn sync(
    mut commands: Commands,
    state: Res<EditorState>,
    loc: Res<Localization>,
    theme: Res<LoadedTheme>,
    hosts: Query<(Entity, Option<&Children>), With<GuitarSetup>>,
    mut last: Local<Option<(Entity, Guitar)>>,
) {
    let Ok((host, children)) = hosts.single() else { return };
    let guitar = current(&state);
    commands.entity(host).insert(Node {
        display: if guitar.is_some() { Display::Flex } else { Display::None },
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        ..default()
    });
    let Some(guitar) = guitar else {
        *last = None;
        return;
    };
    if last.as_ref() == Some(&(host, guitar.clone())) && !loc.is_changed() && !theme.is_changed() {
        return;
    }
    *last = Some((host, guitar.clone()));
    if let Some(children) = children {
        for &child in children {
            commands.entity(child).despawn();
        }
    }
    let colors = theme.song_editor_colors();
    commands.entity(host).with_children(|p| {
        p.spawn((Text::new(loc.msg("editor-guitar-setup").to_string()), TextColor(colors.label)));
        p.spawn((
            Text::new(loc.msg("editor-guitar-setup-help").to_string()),
            TextFont { font_size: FontSize::Px(13.0), ..default() },
            TextColor(colors.label),
        ));
        p.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            for (index, key) in [
                "guitar-tuning-standard",
                "guitar-tuning-drop-d",
                "guitar-tuning-half-step-down",
                "guitar-tuning-dadgad",
                "guitar-tuning-open-g",
                "guitar-tuning-open-d",
            ]
            .into_iter()
            .enumerate()
            {
                row.spawn_empty().apply_scene(button::small(
                    &loc.msg(key),
                    move |_: On<Activate>,
                          mut state: ResMut<EditorState>,
                          loc: Res<Localization>,
                          mut feedback: ResMut<SaveFeedback>| {
                        let Some(mut guitar) = current(&state) else { return };
                        guitar.strings =
                            harmonicon_core::instrument::guitar::TUNINGS[index].strings.to_vec();
                        report(configure(&mut state, guitar), &loc, &mut feedback);
                    },
                ));
            }
        });
        p.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
            .with_children(|row| {
                row.spawn((
                    Text::new(
                        loc.msg_args(
                            "editor-guitar-string-count",
                            &[("count", guitar.strings.len().to_string())],
                        )
                        .to_string(),
                    ),
                    TextColor(colors.label),
                ));
                for (key, delta) in
                    [("editor-guitar-add-string", 1isize), ("editor-guitar-remove-string", -1)]
                {
                    row.spawn_empty().apply_scene(button::small(
                        &loc.msg(key),
                        move |_: On<Activate>,
                              mut state: ResMut<EditorState>,
                              loc: Res<Localization>,
                              mut feedback: ResMut<SaveFeedback>| {
                            let Some(mut guitar) = current(&state) else { return };
                            let count = guitar.strings.len().saturating_add_signed(delta);
                            let ok = resize(&mut guitar, count) && configure(&mut state, guitar);
                            report(ok, &loc, &mut feedback);
                        },
                    ));
                }
            });
    });
    // Each string gets its own editable scientific pitch (including octave).
    for (index, &pitch) in guitar.strings.iter().enumerate() {
        let row = commands
            .spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
            .id();
        commands.entity(host).add_child(row);
        commands.entity(row).with_children(|p| {
            p.spawn((
                Text::new(
                    loc.msg_args(
                        "editor-guitar-string-number",
                        &[("number", (index + 1).to_string())],
                    )
                    .to_string(),
                ),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(colors.label),
            ));
        });
        spawn_text_input(
            &mut commands,
            row,
            &harmonicon_core::midi::midi_to_note(i32::from(pitch)),
            80.0,
            colors.hole_box,
            colors.label,
            move |ev: On<TextInputCommitted>,
                  mut state: ResMut<EditorState>,
                  loc: Res<Localization>,
                  mut feedback: ResMut<SaveFeedback>,
                  mut inputs: Query<&mut EditableText>| {
                let Some(mut guitar) = current(&state) else { return };
                let original = guitar.strings.get(index).copied();
                let parsed = parse_pitch(&ev.value);
                let ok = if let Some(pitch) = parsed {
                    if let Some(string) = guitar.strings.get_mut(index) {
                        *string = pitch;
                        configure(&mut state, guitar)
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !ok
                    && let Some(original) = original
                    && let Ok(mut input) = inputs.get_mut(ev.input)
                {
                    input
                        .editor_mut()
                        .set_text(&harmonicon_core::midi::midi_to_note(i32::from(original)));
                    input.queue_edit(TextEdit::TextEnd(false));
                }
                report(ok, &loc, &mut feedback);
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::guitar::GuitarBody;
    fn state() -> EditorState {
        super::super::document_picker::fresh(
            super::super::instrument_ui::InstrumentKind::Guitar.selection(),
        )
    }
    #[test]
    fn pitch_input_accepts_octaves_and_refuses_unbounded_or_incomplete_values() {
        assert_eq!(parse_pitch(" F#3 "), Some(54));
        assert_eq!(parse_pitch("Bb2"), Some(46));
        for text in ["", "E", "—", "C2147483647", "C-1", "H4"] {
            assert_eq!(parse_pitch(text), None);
        }
    }

    #[test]
    fn custom_tuning_keeps_frets_and_round_trips() {
        let mut state = state();
        super::super::guitar_note::place(&mut state, 6, 0, 12);
        let mut guitar = current(&state).unwrap();
        guitar.strings[5] = 38;
        assert!(configure(&mut state, guitar.clone()));
        assert_eq!(state.notes[0].target.sounding_pitch(&Instrument::Guitar(guitar)), Some(38.0));
        let chart: serde_json::Value =
            serde_json::from_str(&super::super::harpchart::serialize_harpchart(&state)).unwrap();
        assert_eq!(chart["instrument"]["strings"][5], "D2");
        super::super::harpchart::validated_harpchart(&chart.to_string()).unwrap();
        let mut loaded = EditorState::default();
        super::super::harpchart::load_harpchart(
            &chart,
            &mut loaded,
            &mut super::super::state::Scroll::default(),
        );
        assert_eq!(loaded.effective_instrument(), state.effective_instrument());
        assert_eq!(
            loaded.notes[0].target.sounding_pitch(&loaded.effective_instrument()),
            Some(38.0)
        );
    }
    #[test]
    fn tuning_is_restored_with_notes_by_undo() {
        let mut app = App::new();
        app.insert_resource(state())
            .init_resource::<super::super::record::RecordState>()
            .init_resource::<super::super::undo::UndoHistory>()
            .add_message::<super::super::state::DocumentCommand>()
            .add_systems(
                Update,
                (super::super::undo::track_changes, super::super::undo::apply_document_commands)
                    .chain(),
            );
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<EditorState>();
            let mut guitar = current(&state).unwrap();
            guitar.strings[0] = 62;
            assert!(configure(&mut state, guitar));
        }
        app.update();
        app.world_mut().write_message(super::super::state::DocumentCommand::Undo);
        app.update();
        assert_eq!(current(app.world().resource::<EditorState>()).unwrap().strings[0], 64);
    }

    #[test]
    fn string_counts_support_extended_guitars_without_deleting_notes() {
        for count in [7, 10, 12] {
            let mut state = state();
            let mut guitar = Guitar::standard(GuitarBody::Electric);
            assert!(resize(&mut guitar, count));
            assert!(configure(&mut state, guitar));
            assert_eq!(state.row_count(), count as u8);
            super::super::guitar_note::place(&mut state, count as u8, 0, 12);
            let before = state.notes.clone();
            let mut guitar = current(&state).unwrap();
            assert!(resize(&mut guitar, 6));
            assert!(!configure(&mut state, guitar));
            assert_eq!(state.notes, before);
            assert_eq!(state.row_count(), count as u8);
        }
    }
}
