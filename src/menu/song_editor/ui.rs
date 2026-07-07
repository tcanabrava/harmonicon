// SPDX-License-Identifier: MIT

use bevy::audio::AudioSource;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::window::WindowResized;

use bevy_fluent::prelude::Localization;
use crate::dialogs::file_dialog::{DialogMode, OpenFileDialog};
use crate::localization::{LocalizationExt, LocalizedStr};
use crate::settings::AudioSettings;
use crate::theme::{LoadedTheme, SongEditorColors};
use super::{
    AppState, grid_height,
    HOLE_COL_W, HEADER_H, ROW_H, BEAT_W, ROWS, NOTE_PAD,
    SAVE_PURPOSE, LOAD_PURPOSE, MUSIC_PURPOSE,
};
use super::state::{EditorState, Mode, Scroll, Field, FIELDS, HARP_KEYS, POSITIONS};
use super::playback::{Playhead, EditorAudio, EditorProgressFill, PlayheadLine, start_playback, toggle_pause};
use super::harpchart::safe_path_segment;
use super::interaction::apply_modifier;
use super::practice::{start_practice, stop_practice, PracticeState};

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

/// The always-visible Edit/Perform mode switch and Lock toggle. Unlike
/// [`ModButton`], these don't go through `apply_modifier` — they change
/// `EditorState::mode`/`user_locked` directly, and switching to Edit also
/// needs to stop any running playback/practice, which needs more than just
/// `&mut EditorState` gives `apply_modifier`.
#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum ModeButton {
    Edit,
    Perform,
    Lock,
}

/// Wraps the note-editing button cluster (Blow, Draw, Bend, ...), shown only
/// in [`Mode::Edit`]. See `update_mode_visibility`.
#[derive(Component)]
pub(super) struct EditModeGroup;

/// Wraps the playback/practice button cluster (Play, Pause, Stop, Practice),
/// shown only in [`Mode::Perform`]. See `update_mode_visibility`.
#[derive(Component)]
pub(super) struct PerformModeGroup;

#[derive(Component)]
pub(super) struct BendDot;

/// Marks a mod button's label text so [`super::panel::update_mod_panel`] can
/// append the selected note's configured rate (e.g. "Vibrato 5Hz"). `base` is
/// the localized label cached at spawn time, since the per-frame update only
/// has the note's numeric state, not the `Localization` resource.
#[derive(Component)]
pub(super) struct ModButtonLabel {
    pub(super) kind: ModButton,
    pub(super) base: String,
}

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

/// `rebuild_grid` only runs when `EditorState` changes, but the number of
/// visible columns depends on the window width too — without this, resizing
/// the window would leave the grid showing its old column count until some
/// unrelated edit happens to touch `EditorState` and trigger a rebuild.
pub(super) fn rebuild_grid_on_resize(
    mut resized: MessageReader<WindowResized>,
    mut state: ResMut<EditorState>,
) {
    if resized.read().next().is_some() {
        state.set_changed();
    }
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

pub(super) fn setup(
    mut commands: Commands,
    loc: Res<Localization>,
    theme: Res<LoadedTheme>,
    state: Res<EditorState>,
) {
    let colors = theme.song_editor_colors();
    let mode = state.mode;
    commands
        .spawn((
            EditorRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(colors.editor_bg),
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
                spawn_hole_column(row, colors);
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
                            BackgroundColor(colors.ghost_ok.with_alpha(0.30)),
                            BorderColor::all(colors.ghost_ok),
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

            spawn_mod_panel(root, &loc, colors, mode);
            spawn_meta_form(root, &loc, colors);

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

fn spawn_hole_column(row: &mut ChildSpawnerCommands, colors: SongEditorColors) {
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
    });
}

fn spawn_mod_panel(root: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors, mode: Mode) {
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
        BackgroundColor(colors.panel_bg),
    ))
    .with_children(|panel| {
        transport_button(
            panel,
            loc.msg("back"),
            Color::srgb(0.22, 0.22, 0.28),
            |_: On<Pointer<Click>>, mut next: ResMut<NextState<AppState>>| {
                next.set(AppState::Menu);
            },
        );
        panel_separator(panel);

        // Edit/Perform/Lock: always visible, regardless of which mode-group
        // below is currently shown.
        mode_button(
            panel,
            ModeButton::Edit,
            loc.msg("editor-mode-edit"),
            colors,
            |_: On<Pointer<Click>>,
             mut state: ResMut<EditorState>,
             playing: Query<Entity, With<EditorAudio>>,
             mut practice: ResMut<PracticeState>,
             mut playhead: ResMut<Playhead>,
             mut commands: Commands| {
                state.mode = Mode::Edit;
                // Leaving Perform mode hides Play/Pause/Stop/Practice, so
                // nothing would be left to stop anything that's running.
                stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
            },
        );
        mode_button(
            panel,
            ModeButton::Perform,
            loc.msg("editor-mode-perform"),
            colors,
            |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.mode = Mode::Perform;
            },
        );
        mode_button(
            panel,
            ModeButton::Lock,
            loc.msg("editor-lock"),
            colors,
            |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.user_locked = !state.user_locked;
            },
        );
        panel_separator(panel);

        spawn_file_buttons(panel, loc);
        panel_separator(panel);

        panel
            .spawn((
                EditModeGroup,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    flex_grow: 1.0,
                    // `Display::None`, not `Visibility::Hidden` — Visibility
                    // only skips rendering, it still reserves this group's
                    // full layout width, which pushed the other group off to
                    // the right instead of freeing its place.
                    display: if mode == Mode::Edit { Display::Flex } else { Display::None },
                    ..default()
                },
            ))
            .with_children(|g| {
                mod_button(g, ModButton::Blow,     loc.msg("mod-blow"),     colors);
                mod_button(g, ModButton::Draw,     loc.msg("mod-draw"),     colors);
                panel_separator(g);
                mod_button(g, ModButton::Bend,     loc.msg("mod-bend"),     colors);
                mod_button(g, ModButton::Overblow, loc.msg("mod-overblow"), colors);
                mod_button(g, ModButton::Overdraw, loc.msg("mod-overdraw"), colors);
                mod_button(g, ModButton::Wah,      loc.msg("mod-wah"),      colors);
                mod_button(g, ModButton::Vibrato,  loc.msg("mod-vibrato"),  colors);
                g.spawn(Node { flex_grow: 1.0, ..default() });
                mod_button(g, ModButton::Delete,   loc.msg("mod-delete"),   colors);
            });

        panel
            .spawn((
                PerformModeGroup,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    flex_grow: 1.0,
                    display: if mode == Mode::Perform { Display::Flex } else { Display::None },
                    ..default()
                },
            ))
            .with_children(|g| {
                spawn_playback_buttons(g, loc);
            });
    });
}

pub(super) fn mode_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    kind: ModeButton,
    label: LocalizedStr,
    colors: SongEditorColors,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
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
            BackgroundColor(colors.btn_bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        ))
        .observe(on_click)
        .with_children(|b| {
            b.spawn((
                Text::new(String::from(label)),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
}

pub(super) fn mod_button(
    panel: &mut ChildSpawnerCommands,
    kind: ModButton,
    label: LocalizedStr,
    colors: SongEditorColors,
) {
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
            BackgroundColor(colors.btn_bg),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        ))
        .observe(move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            apply_modifier(&mut state, kind);
        })
        .with_children(|b| {
            let base = String::from(label);
            let mut text = b.spawn((
                Text::new(base.clone()),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
            if matches!(kind, ModButton::Wah | ModButton::Vibrato) {
                text.insert(ModButtonLabel { kind, base });
            }
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

/// Chart file I/O — always visible, in both Edit and Perform mode.
fn spawn_file_buttons(panel: &mut ChildSpawnerCommands, loc: &Localization) {
    transport_button(
        panel,
        loc.msg("editor-save"),
        Color::srgb(0.18, 0.28, 0.45),
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         loc: Res<Localization>,
         mut open: MessageWriter<OpenFileDialog>| {
            let default_name = format!(
                "{}.harpchart",
                safe_path_segment(if state.name.is_empty() { "chart" } else { &state.name })
            );
            open.write(OpenFileDialog {
                purpose: SAVE_PURPOSE,
                title: String::from(loc.msg("dialog-save-chart")),
                extensions: vec!["harpchart".into()],
                start_dir: Some(std::path::PathBuf::from("assets/songs")),
                mode: DialogMode::Save { default_name },
            });
        },
    );
    transport_button(
        panel,
        loc.msg("editor-load"),
        Color::srgb(0.24, 0.30, 0.20),
        |_: On<Pointer<Click>>,
         loc: Res<Localization>,
         mut open: MessageWriter<OpenFileDialog>| {
            open.write(OpenFileDialog {
                purpose: LOAD_PURPOSE,
                title: String::from(loc.msg("dialog-load-chart")),
                extensions: vec!["harpchart".into()],
                start_dir: Some(std::path::PathBuf::from("assets/songs")),
                mode: DialogMode::Open,
            });
        },
    );
}

/// Play/Pause/Stop/Practice — only shown in [`Mode::Perform`] (wrapped in
/// [`PerformModeGroup`] by the caller).
fn spawn_playback_buttons(panel: &mut ChildSpawnerCommands, loc: &Localization) {
    transport_button(
        panel,
        loc.msg("editor-play"),
        Color::srgb(0.20, 0.40, 0.24),
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         sinks: Query<&AudioSink, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            // Paused, not stopped: resume in place rather than restarting.
            if playhead.playing && playhead.paused {
                toggle_pause(&mut playhead, &sinks);
                return;
            }
            practice.reset(); // exit practice mode before starting preview playback
            start_playback(&state, &mut sources, &settings, &playing, &mut playhead, &mut commands);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-pause"),
        Color::srgb(0.36, 0.32, 0.16),
        |_: On<Pointer<Click>>,
         mut playhead: ResMut<Playhead>,
         sinks: Query<&AudioSink, With<EditorAudio>>| {
            toggle_pause(&mut playhead, &sinks);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-stop"),
        Color::srgb(0.36, 0.20, 0.20),
        |_: On<Pointer<Click>>,
         playing: Query<Entity, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands| {
            stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
        },
    );
    transport_button(
        panel,
        loc.msg("editor-practice"),
        Color::srgb(0.25, 0.18, 0.42),
        |_: On<Pointer<Click>>,
         state: Res<EditorState>,
         mut sources: ResMut<Assets<AudioSource>>,
         settings: Res<AudioSettings>,
         playing: Query<Entity, With<EditorAudio>>,
         mut practice: ResMut<PracticeState>,
         mut playhead: ResMut<Playhead>,
         mut commands: Commands,
         loc: Res<Localization>,
         sinks: Query<&AudioSink, With<EditorAudio>>| {
            // Paused, not stopped: resume in place rather than stopping.
            if practice.active && playhead.paused {
                toggle_pause(&mut playhead, &sinks);
                return;
            }
            if practice.active {
                stop_practice(&playing, &mut practice, &mut playhead, &mut commands);
            } else {
                start_practice(
                    &state, &mut sources, &settings,
                    &playing, &mut practice, &mut playhead, &mut commands, &loc,
                );
            }
        },
    );
}

pub(super) fn transport_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    label: LocalizedStr,
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
                Text::new(String::from(label)),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
}

fn spawn_meta_form(root: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
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
                    Text::new(format!("{}:", loc.msg(label))),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
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
                    btn.observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                        let idx = HARP_KEYS
                            .iter()
                            .position(|&k| k == state.key.as_str())
                            .unwrap_or(0);
                        state.key = HARP_KEYS[(idx + 1) % HARP_KEYS.len()].into();
                    });
                } else if field == Field::Position {
                    btn.observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                        let idx = POSITIONS
                            .iter()
                            .position(|&p| p == state.position.as_str())
                            .unwrap_or(0);
                        state.position = POSITIONS[(idx + 1) % POSITIONS.len()].into();
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
                    .observe(|_: On<Pointer<Click>>,
                               loc: Res<Localization>,
                               mut open: MessageWriter<OpenFileDialog>| {
                        open.write(OpenFileDialog {
                            purpose: MUSIC_PURPOSE,
                            title: String::from(loc.msg("dialog-select-music")),
                            extensions: vec!["ogg".into()],
                            start_dir: dirs::home_dir(),
                            mode: DialogMode::Open,
                        });
                    })
                    .with_children(|b| {
                        b.spawn((
                            Text::new(String::from(loc.msg("editor-browse"))),
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
