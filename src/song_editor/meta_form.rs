// SPDX-License-Identifier: MIT

//! The chart metadata form (harmonica kind/key/position/tempo/... fields,
//! the MIDI-track import row) and the hole-column spawning it's visually
//! paired with in the setup layout (both are per-hole/per-chart "reference"
//! panels, unlike the interactive note grid or mod panel).

use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::state::{
    ContentKind, EditorState, FIELDS, Field, HARP_KEYS, HarmonicaKind, PASS_CRITERIA_KINDS,
    POSITIONS, PROGRESSIONS, TECHNIQUE_NAMES, cycle_next,
};
use super::ui::{
    ContentKindText, HarmonicaKindText, HoleColumnContent, MetaFieldBox, MetaFieldText,
    MidiTrackComboboxSlot,
};
use super::{HEADER_H, MIDI_PURPOSE, MUSIC_PURPOSE, ROW_H, SILENCE_ROW_H, grid_height};
use crate::dialogs::file_dialog::{DialogMode, OpenFileDialog};
use crate::dialogs::tooltip::Tooltip;
use crate::localization::LocalizationExt;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

pub(super) fn spawn_hole_column(
    row: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
    hole_count: u8,
    loc: &Localization,
) {
    row.spawn((
        HoleColumnContent,
        Node {
            width: Val::Px(super::HOLE_COL_W),
            height: Val::Px(grid_height(hole_count)),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..default()
        },
    ))
    .with_children(|col| {
        spawn_hole_column_rows(col, colors, hole_count, loc);
    });
}

/// Respawns the hole column's contents (called from `ui::setup` initially,
/// and from `ui::sync_hole_column` whenever the harmonica's hole count
/// changes).
pub(super) fn spawn_hole_column_rows(
    col: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
    hole_count: u8,
    loc: &Localization,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Px(HEADER_H),
        ..default()
    });
    for hole in 1..=hole_count {
        col.spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(ROW_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Text::new(format!("{hole:02}")),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(colors.label),
            ));
            r.spawn((
                Node {
                    width: Val::Px(20.0),
                    height: Val::Px(20.0),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(colors.hole_box),
                BorderColor::all(Color::srgb(0.45, 0.45, 0.55)),
            ));
        });
    }
    // Label for the silence track's background strip (spawned in
    // `grid::rebuild_grid`) — keeps this column's total height matching
    // `grid_height` so the hole rows on the right stay aligned with it.
    col.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(SILENCE_ROW_H),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Tooltip(String::from(loc.msg("editor-silence-track-tooltip"))),
    ))
    .with_children(|r| {
        r.spawn((
            Text::new(loc.msg("editor-silence-track-label").to_string()),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(colors.label),
            Pickable::IGNORE,
        ));
    });
}

/// Label width within a form column — fixed so a column's field boxes all
/// line up at the same x position, same reasoning the pre-two-column layout
/// used a fixed label width for.
const FORM_LABEL_W: f32 = 110.0;

/// A form column: one of the two side-by-side stacks [`spawn_meta_form`]
/// splits its 8 rows across.
fn spawn_form_column(root: &mut ChildSpawnerCommands, build: impl FnOnce(&mut ChildSpawnerCommands)) {
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        flex_grow: 1.0,
        ..default()
    })
    .with_children(build);
}

/// A labelled click-to-cycle button row: `<label>:  [ current value ]` — the
/// shared shape [`spawn_content_kind_row`]/[`spawn_harmonica_kind_row`] both
/// build (the Key/Position/lesson pass-criteria/technique/progression
/// cycle fields are a separate case — they already share their scaffold via
/// [`spawn_field_row`], just branching on which `Field` they are). `marker`
/// tags the value text so its own `update_*_text` system can find it.
fn spawn_cycle_row<T: Component, M: 'static>(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    label_key: &str,
    tooltip_key: &str,
    marker: T,
    on_click: impl IntoObserverSystem<Pointer<Click>, (), M> + Clone + Sync + 'static,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn((
            Node {
                width: Val::Px(FORM_LABEL_W),
                ..default()
            },
            Text::new(format!("{}:", loc.msg(label_key))),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(colors.label),
        ));
        line.spawn((
            Button,
            Node {
                width: Val::Px(240.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(colors.field_bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
            Tooltip(String::from(loc.msg(tooltip_key))),
        ))
        .observe(on_click)
        .with_children(|b| {
            b.spawn((
                marker,
                Text::new(String::new()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
    });
}

/// The "Record Song"/"Record Lesson" toggle — switching `content_kind` has
/// no side effects to reconcile (unlike harmonica kind, which must sanitize
/// existing notes), so the observer just flips it directly rather than
/// calling a dedicated `EditorState` method.
fn spawn_content_kind_row(col: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    spawn_cycle_row(
        col,
        loc,
        colors,
        "editor-field-content-kind",
        "editor-content-kind-toggle-tooltip",
        ContentKindText,
        |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            state.content_kind = match state.content_kind {
                ContentKind::Song => ContentKind::Lesson,
                ContentKind::Lesson => ContentKind::Song,
            };
        },
    );
}

fn spawn_harmonica_kind_row(col: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    spawn_cycle_row(
        col,
        loc,
        colors,
        "editor-field-harmonica",
        "editor-harmonica-toggle-tooltip",
        HarmonicaKindText,
        |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            let next = match state.harmonica_kind {
                HarmonicaKind::Diatonic => HarmonicaKind::Chromatic,
                HarmonicaKind::Chromatic => HarmonicaKind::Diatonic,
            };
            state.set_harmonica_kind(next);
        },
    );
}

pub(super) fn spawn_field_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    field: Field,
    label: &str,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn((
            Node {
                width: Val::Px(FORM_LABEL_W),
                ..default()
            },
            Text::new(format!("{}:", loc.msg(label))),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(colors.label),
        ));

        let mut btn = line.spawn((
            Button,
            MetaFieldBox(field),
            Node {
                width: Val::Px(240.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(colors.field_bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        ));

        if field == Field::Key {
            btn.insert(Tooltip(String::from(loc.msg("editor-field-key-tooltip"))))
                .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                    state.key = cycle_next(&HARP_KEYS, &state.key);
                });
        } else if field == Field::Position {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-position-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.position = cycle_next(&POSITIONS, &state.position);
            });
        } else if field == Field::LessonPassCriteria {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-lesson-pass-criteria-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.lesson_pass_criteria =
                    cycle_next(&PASS_CRITERIA_KINDS, &state.lesson_pass_criteria);
            });
        } else if field == Field::LessonTechnique {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-lesson-technique-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.lesson_technique = cycle_next(&TECHNIQUE_NAMES, &state.lesson_technique);
            });
        } else if field == Field::LessonProgression {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-lesson-progression-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.lesson_progression = cycle_next(&PROGRESSIONS, &state.lesson_progression);
            });
        } else {
            btn.observe(
                move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                    state.focus = Some(field);
                },
            );
        }

        btn.with_children(|b| {
            b.spawn((
                MetaFieldText(field),
                Text::new(String::new()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });

        if field == Field::Music {
            line.spawn((
                Button,
                Node {
                    height: Val::Px(26.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.18, 0.24, 0.36)),
                BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                Tooltip(String::from(loc.msg("editor-browse-tooltip"))),
            ))
            .observe(
                |_: On<Pointer<Click>>,
                 loc: Res<Localization>,
                 mut open: MessageWriter<OpenFileDialog>| {
                    open.write(OpenFileDialog {
                        purpose: MUSIC_PURPOSE,
                        title: String::from(loc.msg("dialog-select-music")),
                        extensions: vec!["ogg".into()],
                        start_dir: dirs::home_dir(),
                        mode: DialogMode::Open,
                    });
                },
            )
            .with_children(|b| {
                b.spawn((
                    Text::new(String::from(loc.msg("editor-browse"))),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                ));
            });
        }
    });
}

fn spawn_midi_track_row(col: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn((
            Node {
                width: Val::Px(FORM_LABEL_W),
                ..default()
            },
            Text::new(format!("{}:", loc.msg("editor-field-midi-track"))),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(colors.label),
        ));
        line.spawn((
            Button,
            Node {
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.24, 0.30, 0.20)),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
            Tooltip(String::from(loc.msg("editor-import-midi-tooltip"))),
        ))
        .observe(
            |_: On<Pointer<Click>>,
             loc: Res<Localization>,
             mut open: MessageWriter<OpenFileDialog>| {
                open.write(OpenFileDialog {
                    purpose: MIDI_PURPOSE,
                    title: String::from(loc.msg("dialog-select-midi")),
                    extensions: vec!["mid".into(), "midi".into()],
                    start_dir: dirs::home_dir(),
                    mode: DialogMode::Open,
                });
            },
        )
        .with_children(|b| {
            b.spawn((
                Text::new(String::from(loc.msg("editor-import-midi"))),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
        line.spawn((
            MidiTrackComboboxSlot,
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ));
    });
}

/// The chart metadata form: two side-by-side columns instead of one
/// full-width row per field — with 8 rows (harmonica kind, [`FIELDS`]'s 6,
/// and the MIDI-track row), stacking them all in one column routinely ran
/// taller than a default-sized window. Split evenly (`FIELDS.len() / 2`, so
/// each column gets at least 4 rows for today's 8): harmonica kind + the
/// first half of `FIELDS` in the left column, the second half + the
/// MIDI-track row in the right — halving the form's height for the same
/// content.
pub(super) fn spawn_meta_form(root: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    const MID: usize = FIELDS.len() / 2;
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(24.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    })
    .with_children(|form| {
        spawn_form_column(form, |col| {
            spawn_content_kind_row(col, loc, colors);
            spawn_harmonica_kind_row(col, loc, colors);
            for &(field, label) in &FIELDS[..MID] {
                spawn_field_row(col, loc, colors, field, label);
            }
        });
        spawn_form_column(form, |col| {
            for &(field, label) in &FIELDS[MID..] {
                spawn_field_row(col, loc, colors, field, label);
            }
            spawn_midi_track_row(col, loc, colors);
        });
    });
}
