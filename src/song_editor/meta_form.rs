// SPDX-License-Identifier: MIT

//! The chart metadata form (harmonica kind/key/position/tempo/... fields,
//! the MIDI-track import row) and the hole-column spawning it's visually
//! paired with in the setup layout (both are per-hole/per-chart "reference"
//! panels, unlike the interactive note grid or mod panel).

use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::state::{EditorState, FIELDS, Field, HARP_KEYS, HarmonicaKind, POSITIONS};
use super::ui::{HarmonicaKindText, HoleColumnContent, MetaFieldBox, MetaFieldText, MidiTrackComboboxSlot};
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

/// The chart metadata form: a single `flex_wrap: Wrap` row of compact
/// "label: box" clusters (harmonica kind, then each of [`FIELDS`], then the
/// MIDI-track row) instead of one full-width row per field — with 8 fields
/// each claiming a full row, that stacked layout alone routinely ran taller
/// than a default-sized window. Wrapping the same clusters onto as many
/// lines as the window's width actually allows (rather than always
/// reserving one) uses far less vertical space, the same fix already
/// applied to the mod panel's own button row (`mod_panel::spawn_mod_panel`).
pub(super) fn spawn_meta_form(root: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(16.0),
        row_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    })
    .with_children(|form| {
        form.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|line| {
            line.spawn((
                Text::new(format!("{}:", loc.msg("editor-field-harmonica"))),
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
                Tooltip(String::from(loc.msg("editor-harmonica-toggle-tooltip"))),
            ))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                let next = match state.harmonica_kind {
                    HarmonicaKind::Diatonic => HarmonicaKind::Chromatic,
                    HarmonicaKind::Chromatic => HarmonicaKind::Diatonic,
                };
                state.set_harmonica_kind(next);
            })
            .with_children(|b| {
                b.spawn((
                    HarmonicaKindText,
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

        for (field, label) in FIELDS {
            form.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
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
                            let idx = HARP_KEYS
                                .iter()
                                .position(|&k| k == state.key.as_str())
                                .unwrap_or(0);
                            state.key = HARP_KEYS[(idx + 1) % HARP_KEYS.len()].into();
                        });
                } else if field == Field::Position {
                    btn.insert(Tooltip(String::from(
                        loc.msg("editor-field-position-tooltip"),
                    )))
                    .observe(
                        |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                            let idx = POSITIONS
                                .iter()
                                .position(|&p| p == state.position.as_str())
                                .unwrap_or(0);
                            state.position = POSITIONS[(idx + 1) % POSITIONS.len()].into();
                        },
                    );
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

        form.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|line| {
            line.spawn((
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
    });
}
