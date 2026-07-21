// SPDX-License-Identifier: MIT

//! Reusable mod-panel button builders — one `spawn` helper per button
//! "shape" (a themed toggle, a themed action button, a distinctly-colored
//! transport button, ...), shared by `mod_panel`'s two-strip assembly.
//! Component type declarations for the buttons these spawn (`ModButton`,
//! `ModeButton`, `TimelineToolButton`, `ModButtonLabel`,
//! `BendDot`) live in `super::ui`, alongside every other song-editor
//! component type.

use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::interaction::apply_modifier;
use super::state::{EditorState, TimelineDrag, TimelineSelection, TimelineTool, normalize_range};
use super::timeline::request_confirm;
use super::ui::{BendDot, ModButton, ModButtonLabel, ModeButton, TimelineToolButton};
use crate::dialogs::confirm_dialog::OpenConfirmDialog;
use crate::dialogs::tooltip::Tooltip;
use crate::localization::LocalizedStr;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

/// The shared button shell every panel button in this file builds on: a
/// padded, bordered button with a tooltip and a single-line white label,
/// observing one click handler. `mode_button`/`transport_button` are plain
/// wrappers over this (the only two shapes here with no per-button extras).
/// `mod_button`/`timeline_tool_button` need extra
/// per-button children (`BendDot`, a swappable label, a `kind`-dependent
/// observer body) that don't fit this shape cleanly, so they stay separate
/// rather than forcing a less-readable shared abstraction onto them.
fn spawn_button_shell<'a, M: 'static>(
    panel: &'a mut ChildSpawnerCommands,
    bg: Color,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) -> EntityCommands<'a> {
    let mut ec = panel.spawn((
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
        Tooltip(String::from(tooltip)),
    ));
    ec.observe(on_click).with_children(|b| {
        b.spawn((
            Text::new(String::from(label)),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ));
    });
    ec
}

pub(super) fn mode_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    kind: ModeButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    colors: SongEditorColors,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
    spawn_button_shell(panel, colors.btn_bg, label, tooltip, on_click).insert(kind);
}

/// An Erase/Remove timeline-tool toggle button — see `TimelineToolButton`.
pub(super) fn timeline_tool_button(
    panel: &mut ChildSpawnerCommands,
    kind: TimelineToolButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
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
            Tooltip(String::from(tooltip)),
        ))
        .observe(
            move |_: On<Pointer<Click>>,
                  loc: Res<Localization>,
                  mut state: ResMut<EditorState>,
                  mut sel: ResMut<TimelineSelection>,
                  mut open: MessageWriter<OpenConfirmDialog>| {
                if let Some(TimelineDrag { start, end, .. }) = sel.drag {
                    let (s, e) = normalize_range(start, end);
                    if kind == TimelineToolButton(TimelineTool::Erase) {
                        state.timeline_tool = TimelineTool::Erase;
                        request_confirm(&mut state, &loc, &mut open, s, e);
                    } else if kind == TimelineToolButton(TimelineTool::Remove) {
                        state.timeline_tool = TimelineTool::Remove;
                        request_confirm(&mut state, &loc, &mut open, s, e);
                    }
                };

                state.timeline_tool = if state.timeline_tool == kind.0 {
                    TimelineTool::None
                } else {
                    kind.0
                };
                sel.drag = None;
                state.timeline_split = None;
            },
        )
        .with_children(|b| {
            b.spawn((
                Text::new(String::from(label)),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
}

pub(super) fn mod_button(
    panel: &mut ChildSpawnerCommands,
    kind: ModButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
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
            Tooltip(String::from(tooltip)),
        ))
        .observe(
            move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                apply_modifier(&mut state, kind);
            },
        )
        .with_children(|b| {
            let base = String::from(label);
            let mut text = b.spawn((
                Text::new(base.clone()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
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

pub(super) fn panel_separator(panel: &mut ChildSpawnerCommands) {
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

pub(super) fn transport_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    bg: Color,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Pointer<Click>, (), M>,
) {
    spawn_button_shell(panel, bg, label, tooltip, on_click);
}
