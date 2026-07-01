// SPDX-License-Identifier: MIT

use bevy::audio::AudioSource;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::dialogs::file_dialog::{DialogMode, OpenFileDialog};
use crate::settings::AudioSettings;
use super::{
    AppState, grid_height,
    HOLE_COL_W, HEADER_H, ROW_H, BEAT_W, ROWS, NOTE_PAD,
    EDITOR_BG, HOLE_BOX, LABEL, PANEL_BG, BTN_BG, FIELD_BG, GHOST_OK,
    SAVE_PURPOSE, LOAD_PURPOSE, MUSIC_PURPOSE,
};
use super::state::{EditorState, Scroll, Field, FIELDS, HARP_KEYS};
use super::playback::{Playhead, EditorAudio, EditorProgressFill, PlayheadLine, start_playback};
use super::harpchart::safe_path_segment;
use super::interaction::apply_modifier;

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
pub(super) struct EditorRoot;

#[derive(Component)]
pub(super) struct GridArea;

#[derive(Component)]
pub(super) struct GridContent;

#[derive(Component)]
pub(super) struct GridItem;

#[derive(Component)]
pub(super) struct NoteView(pub(super) u32);

#[derive(Component)]
pub(super) struct MoveGhost;

#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum ModButton {
    Blow,
    Draw,
    Bend,
    Overblow,
    Overdraw,
    Wah,
    Vibrato,
    Delete,
}

#[derive(Component)]
pub(super) struct BendDot;

#[derive(Component)]
pub(super) struct MetaFieldBox(pub(super) Field);

#[derive(Component)]
pub(super) struct MetaFieldText(pub(super) Field);

#[derive(Component)]
pub(super) struct StatusMsg;

// ── Lifecycle systems ─────────────────────────────────────────────────────────

pub(super) fn init_state(mut commands: Commands, existing: Option<Res<EditorState>>) {
    if existing.is_none() {
        commands.insert_resource(EditorState::default());
    }
    commands.insert_resource(Playhead::default());
    commands.insert_resource(Scroll::default());
}

pub(super) fn force_grid_rebuild(mut state: ResMut<EditorState>) {
    state.set_changed();
}

pub(super) fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, With<EditorRoot>>,
    audio: Query<Entity, With<EditorAudio>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    for e in &audio {
        commands.entity(e).despawn();
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

pub(super) fn setup(mut commands: Commands) {
    commands
        .spawn((
            EditorRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(EDITOR_BG),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(5.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    EditorProgressFill,
                    Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(Color::srgb(0.35, 0.75, 1.0)),
                ));
            });

            root.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(grid_height()),
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|row| {
                spawn_hole_column(row);
                row.spawn((
                    GridArea,
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(grid_height()),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ))
                .with_children(|ga| {
                    ga.spawn((
                        GridContent,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            height: Val::Px(grid_height()),
                            ..default()
                        },
                    ))
                    .with_children(|content| {
                        content.spawn((
                            MoveGhost,
                            ZIndex(2),
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(BEAT_W - 2.0),
                                height: Val::Px(ROW_H - 2.0 * NOTE_PAD),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(GHOST_OK.with_alpha(0.30)),
                            BorderColor::all(GHOST_OK),
                            Visibility::Hidden,
                            Pickable::IGNORE,
                        ));
                        content.spawn((
                            PlayheadLine,
                            ZIndex(3),
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(0.0),
                                width: Val::Px(2.0),
                                height: Val::Px(grid_height()),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.95, 0.30, 0.30)),
                            Visibility::Hidden,
                            Pickable::IGNORE,
                        ));
                    });
                });
            });

            spawn_mod_panel(root);
            spawn_meta_form(root);

            root.spawn((
                StatusMsg,
                Text::new(""),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgb(1.0, 0.40, 0.15)),
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                    ..default()
                },
            ));
        });
}

fn spawn_hole_column(row: &mut ChildSpawnerCommands) {
    row.spawn(Node {
        width: Val::Px(HOLE_COL_W),
        height: Val::Px(grid_height()),
        flex_direction: FlexDirection::Column,
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|col| {
        col.spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(HEADER_H),
            ..default()
        });
        for hole in 1..=ROWS {
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
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(LABEL),
                ));
                r.spawn((
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(HOLE_BOX),
                    BorderColor::all(Color::srgb(0.45, 0.45, 0.55)),
                ));
            });
        }
    });
}

fn spawn_mod_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(52.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ))
    .with_children(|panel| {
        transport_button(
            panel,
            "\u{2190} Back",
            Color::srgb(0.22, 0.22, 0.28),
            |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
                next.set(AppState::Menu);
            },
        );
        panel_separator(panel);
        spawn_transport(panel);
        panel_separator(panel);
        mod_button(panel, ModButton::Blow, "Blow");
        mod_button(panel, ModButton::Draw, "Draw");
        panel_separator(panel);
        mod_button(panel, ModButton::Bend, "Bend");
        mod_button(panel, ModButton::Overblow, "Overblow");
        mod_button(panel, ModButton::Overdraw, "Overdraw");
        mod_button(panel, ModButton::Wah, "Wah");
        mod_button(panel, ModButton::Vibrato, "Vibrato");
        panel.spawn(Node { flex_grow: 1.0, ..default() });
        mod_button(panel, ModButton::Delete, "Delete");
    });
}

pub(super) fn mod_button(panel: &mut ChildSpawnerCommands, kind: ModButton, label: &str) {
    panel
        .spawn((
            Button,
            kind,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        ))
        .observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            apply_modifier(&mut state, kind);
        })
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
            if kind == ModButton::Bend {
                b.spawn((
                    BendDot,
                    Node {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        margin: UiRect::left(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.90, 0.20, 0.20)),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ));
            }
        });
}

fn panel_separator(panel: &mut ChildSpawnerCommands) {
    panel.spawn((
        Node {
            width: Val::Px(1.0),
            height: Val::Px(28.0),
            margin: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.30, 0.30, 0.40)),
    ));
}

fn spawn_transport(panel: &mut ChildSpawnerCommands) {
    transport_button(
        panel,
        "\u{25B6} Play",
        Color::srgb(0.20, 0.40, 0.24),
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            start_playback(&state, &mut sources, &settings, &playing, &mut playhead, &mut commands);
        },
    );
    transport_button(
        panel,
        "\u{25A0} Stop",
        Color::srgb(0.36, 0.20, 0.20),
        |_: On<Pointer<Click>>,
         playing: Query<Entity, With<EditorAudio>>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            for e in &playing {
                commands.entity(e).despawn();
            }
            playhead.playing = false;
        },
    );
    transport_button(
        panel,
        "\u{1F4BE} Save",
        Color::srgb(0.18, 0.28, 0.45),
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         mut open: MessageWriter<OpenFileDialog>| {
            let default_name = format!(
                "{}.harpchart",
                safe_path_segment(if state.name.is_empty() { "chart" } else { &state.name })
            );
            open.write(OpenFileDialog {
                purpose: SAVE_PURPOSE,
                title: "Save chart".to_string(),
                extensions: vec!["harpchart".into()],
                start_dir: Some(std::path::PathBuf::from("assets/songs")),
                mode: DialogMode::Save { default_name },
            });
        },
    );
    transport_button(
        panel,
        "\u{1F4C2} Load",
        Color::srgb(0.24, 0.30, 0.20),
        |_: On<Pointer<Click>>, mut open: MessageWriter<OpenFileDialog>| {
            open.write(OpenFileDialog {
                purpose: LOAD_PURPOSE,
                title: "Load chart".to_string(),
                extensions: vec!["harpchart".into()],
                start_dir: Some(std::path::PathBuf::from("assets/songs")),
                mode: DialogMode::Open,
            });
        },
    );
}

pub(super) fn transport_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    label: &str,
    bg: Color,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
    panel
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        ))
        .observe(on_click)
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_meta_form(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    })
    .with_children(|form| {
        for (field, label) in FIELDS {
            form.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
                    Node { width: Val::Px(150.0), ..default() },
                    Text::new(format!("{label}:")),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(LABEL),
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
                    BackgroundColor(FIELD_BG),
                    BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                ));

                if field == Field::Key {
                    btn.observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                        let idx = HARP_KEYS
                            .iter()
                            .position(|&k| k == state.key.as_str())
                            .unwrap_or(0);
                        state.key = HARP_KEYS[(idx + 1) % HARP_KEYS.len()].into();
                    });
                } else {
                    btn.observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                        state.focus = Some(field);
                    });
                }

                btn.with_children(|b| {
                    b.spawn((
                        MetaFieldText(field),
                        Text::new(String::new()),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextColor(Color::WHITE),
                        Pickable::IGNORE,
                    ));
                });
                drop(btn);

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
                    ))
                    .observe(|_: On<Pointer<Click>>, mut open: MessageWriter<OpenFileDialog>| {
                        open.write(OpenFileDialog {
                            purpose: MUSIC_PURPOSE,
                            title: "Select background music".to_string(),
                            extensions: vec!["ogg".into()],
                            start_dir: dirs::home_dir(),
                            mode: DialogMode::Open,
                        });
                    })
                    .with_children(|b| {
                        b.spawn((
                            Text::new("\u{1F4C2} Browse"),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                            TextColor(Color::WHITE),
                            Pickable::IGNORE,
                        ));
                    });
                }
            });
        }
    });
}
