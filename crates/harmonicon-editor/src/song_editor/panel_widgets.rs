// SPDX-License-Identifier: MIT

//! Reusable mod-panel button builders — one `spawn` helper per button
//! "shape" (a themed toggle, a themed action button, a distinctly-colored
//! transport button, ...), shared by `mod_panel`'s two-strip assembly.
//! Component type declarations for the buttons these spawn (`ModButton`,
//! `ModeButton`, `TimelineToolButton`, `ModButtonLabel`,
//! `BendDot`) live in `super::ui`, alongside every other song-editor
//! component type.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;

use super::interaction::apply_modifier;
use super::ranges::normalize_range;
use super::state::{EditorState, TimelineDrag, TimelineSelection, TimelineTool};
use super::timeline::request_confirm;
use super::ui::{BendDot, ModButton, ModButtonLabel, ModeButton, TimelineToolButton};
use harmonicon_platform::localization::{Localization, LocalizedStr};
use harmonicon_platform::settings::ActionButtonStyle;
use harmonicon_platform::theme::SongEditorColors;
use harmonicon_ui::dialogs::button::make_interactive;
use harmonicon_ui::dialogs::confirm_dialog::OpenConfirmDialog;
use harmonicon_ui::dialogs::tooltip::Tooltip;

/// The display text for an action button under `style` — icon alone, icon
/// beside the label, or the label alone. Shared by every button shape in
/// this file, and by `ModButtonLabel::base` (`panel::update_mod_panel`'s
/// live Wah/Vibrato Hz suffix is appended on top of whatever this returns,
/// so it stays correct under every style without that system needing to
/// know about icons or `ActionButtonStyle` itself).
pub(super) fn button_content_text(style: ActionButtonStyle, icon: &str, label: &str) -> String {
    match style {
        ActionButtonStyle::IconOnly => icon.to_string(),
        ActionButtonStyle::TextBesideIcon => format!("{icon} {label}"),
        ActionButtonStyle::TextOnly => label.to_string(),
    }
}

/// The bordered, padded, tooltipped button every panel button in this file
/// is: no label and no click handler yet, so each shape below adds its own.
pub(super) fn spawn_shell<'a>(
    panel: &'a mut ChildSpawnerCommands,
    bg: Color,
    tooltip: LocalizedStr,
) -> EntityCommands<'a> {
    let mut ec = panel.spawn_empty();
    ec.apply_scene(bsn! {
        WidgetButton
        TabIndex(0)
        Node {
            padding: {UiRect::axes(Val::Px(14.0), Val::Px(8.0))},
            align_items: {AlignItems::Center},
            justify_content: {JustifyContent::Center},
            border: {UiRect::all(Val::Px(1.0))},
        }
        ~{BorderColor::all(Color::srgb(0.30, 0.30, 0.40))}
        Tooltip({String::from(tooltip)})
    });
    make_interactive(&mut ec, bg);
    ec
}

/// A shell's single-line white label.
pub(super) fn spawn_label<'a>(
    button: &'a mut ChildSpawnerCommands,
    text: String,
) -> EntityCommands<'a> {
    let mut ec = button.spawn_empty();
    ec.apply_scene(bsn! {
        Text({text})
        TextFont { font_size: {FontSize::Px(14.0)} }
        TextColor({Color::WHITE})
        ~{Pickable::IGNORE}
    });
    ec
}

/// A shell labelled via [`button_content_text`], observing one click handler.
fn spawn_button_shell<'a, M: 'static>(
    panel: &'a mut ChildSpawnerCommands,
    bg: Color,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    icon: &str,
    style: ActionButtonStyle,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) -> EntityCommands<'a> {
    let text = button_content_text(style, icon, &label);
    let mut ec = spawn_shell(panel, bg, tooltip);
    ec.observe(on_click).with_children(|b| {
        spawn_label(b, text);
    });
    ec
}

pub(super) fn mode_button<M: 'static>(
    panel: &mut ChildSpawnerCommands,
    kind: ModeButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    icon: &str,
    style: ActionButtonStyle,
    colors: SongEditorColors,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) {
    spawn_button_shell(panel, colors.btn_bg, label, tooltip, icon, style, on_click).insert(kind);
}

/// An Erase/Remove timeline-tool toggle button — see `TimelineToolButton`.
pub(super) fn timeline_tool_button(
    panel: &mut ChildSpawnerCommands,
    kind: TimelineToolButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    icon: &str,
    style: ActionButtonStyle,
    colors: SongEditorColors,
) {
    let on_click = move |_: On<Activate>,
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

        state.timeline_tool =
            if state.timeline_tool == kind.0 { TimelineTool::None } else { kind.0 };
        sel.drag = None;
        state.timeline_split = None;
    };
    spawn_button_shell(panel, colors.btn_bg, label, tooltip, icon, style, on_click).insert(kind);
}

/// A note-technique toggle. Wah/Vibrato/Depth mark their label so
/// `panel::update_mod_panel` can append a live value; Bend gets the dot it
/// lights while a bend is armed.
pub(super) fn mod_button(
    panel: &mut ChildSpawnerCommands,
    kind: ModButton,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    icon: &str,
    style: ActionButtonStyle,
    colors: SongEditorColors,
) {
    let base = button_content_text(style, icon, &label);
    let mut ec = spawn_shell(panel, colors.btn_bg, tooltip);
    ec.insert(kind)
        .observe(move |_: On<Activate>, mut state: ResMut<EditorState>| {
            apply_modifier(&mut state, kind);
        })
        .with_children(|b| {
            let mut text = spawn_label(b, base.clone());
            if matches!(kind, ModButton::Wah | ModButton::Vibrato | ModButton::Depth) {
                text.insert(ModButtonLabel { kind, base });
            }
            if kind == ModButton::Bend {
                b.spawn_empty().apply_scene(bsn! {
                    BendDot
                    Node {
                        width: {Val::Px(10.0)},
                        height: {Val::Px(10.0)},
                        margin: {UiRect::left(Val::Px(6.0))},
                    }
                    BackgroundColor({Color::srgb(0.90, 0.20, 0.20)})
                    ~{Visibility::Hidden}
                    ~{Pickable::IGNORE}
                });
            }
        });
}

pub(super) fn panel_separator(panel: &mut ChildSpawnerCommands) {
    // A horizontal rule: the toolbar is a vertical column (see
    // `mod_panel::spawn_mod_panel`), so a group divider runs across it, not
    // down it. This was a 1x28 vertical tick back when the panel was a
    // horizontal strip.
    panel.spawn_empty().apply_scene(bsn! {
        Node {
            width: {Val::Percent(100.0)},
            height: {Val::Px(1.0)},
            margin: {UiRect::vertical(Val::Px(4.0))},
        }
        BackgroundColor({Color::srgb(0.30, 0.30, 0.40)})
    });
}

/// Returns the spawned button's `EntityCommands` (unlike `mode_button`,
/// which always inserts its own `kind` marker) so the rare caller that
/// needs to attach something extra — e.g. `mod_panel`'s Undo/Redo buttons,
/// dimmed by `panel::update_undo_redo_buttons` — can `.insert(...)` onto
/// it; every other caller just ignores the return value, as before.
pub(super) fn transport_button<'a, M: 'static>(
    panel: &'a mut ChildSpawnerCommands,
    label: LocalizedStr,
    tooltip: LocalizedStr,
    icon: &str,
    style: ActionButtonStyle,
    bg: Color,
    on_click: impl bevy::ecs::system::IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) -> EntityCommands<'a> {
    spawn_button_shell(panel, bg, label, tooltip, icon, style, on_click)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_only_shows_just_the_icon() {
        assert_eq!(
            button_content_text(ActionButtonStyle::IconOnly, "\u{21B6}", "Undo"),
            "\u{21B6}"
        );
    }

    #[test]
    fn text_beside_icon_shows_both() {
        assert_eq!(
            button_content_text(ActionButtonStyle::TextBesideIcon, "\u{21B6}", "Undo"),
            "\u{21B6} Undo"
        );
    }

    #[test]
    fn text_only_shows_just_the_label() {
        assert_eq!(button_content_text(ActionButtonStyle::TextOnly, "\u{21B6}", "Undo"), "Undo");
    }
}
