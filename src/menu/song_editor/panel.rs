// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::practice::PracticeState;
use super::state::{Dir, EditorState, Expr, Field, Mode, Pitch};
use super::ui::{
    BendDot, EditModeGroup, MetaFieldBox, MetaFieldText, ModButton, ModButtonLabel, ModeButton,
    PerformModeGroup, StatusMsg,
};
use crate::theme::LoadedTheme;

pub(super) fn update_mod_panel(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ModButton, &mut BackgroundColor)>,
    mut dot: Query<&mut Visibility, With<BendDot>>,
    mut labels: Query<(&ModButtonLabel, &mut Text)>,
) {
    let colors = theme.song_editor_colors();
    let selected = state.selected_note().copied();
    for (kind, mut bg) in &mut buttons {
        let active = selected.is_some_and(|n| match kind {
            ModButton::Blow => n.dir == Dir::Blow,
            ModButton::Draw => n.dir == Dir::Draw,
            ModButton::Bend => matches!(n.pitch, Pitch::Bend(_)),
            ModButton::Overblow => n.pitch == Pitch::Overblow,
            ModButton::Overdraw => n.pitch == Pitch::Overdraw,
            ModButton::Wah => matches!(n.expr, Expr::Wah(_)),
            ModButton::Vibrato => matches!(n.expr, Expr::Vibrato(_)),
            ModButton::Delete => false,
        });
        bg.0 = if active {
            colors.btn_active
        } else {
            colors.btn_bg
        };
    }
    let bent = selected.is_some_and(|n| matches!(n.pitch, Pitch::Bend(_)));
    for mut vis in &mut dot {
        *vis = if bent {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    // Show the selected note's configured rate next to Wah/Vibrato (e.g.
    // "Vibrato 5Hz") so cycling the rate with repeated clicks is legible.
    for (label, mut text) in &mut labels {
        let hz = selected.and_then(|n| match (label.kind, n.expr) {
            (ModButton::Vibrato, Expr::Vibrato(hz)) => Some(hz),
            (ModButton::Wah, Expr::Wah(hz)) => Some(hz),
            _ => None,
        });
        **text = match hz {
            Some(hz) => format!("{} {hz:.0}Hz", label.base),
            None => label.base.clone(),
        };
    }
}

pub(super) fn update_meta_fields(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut texts: Query<(&MetaFieldText, &mut Text)>,
    mut boxes: Query<(&MetaFieldBox, &mut BackgroundColor)>,
) {
    let colors = theme.song_editor_colors();
    for (tag, mut text) in &mut texts {
        **text = if tag.0 == Field::Key {
            format!("\u{2039}  {}  \u{203A}", state.key)
        } else {
            let mut s = state.field_text(tag.0).to_string();
            if state.focus == Some(tag.0) {
                s.push('_');
            }
            s
        };
    }
    for (tag, mut bg) in &mut boxes {
        bg.0 = if tag.0 != Field::Key && state.focus == Some(tag.0) {
            colors.field_bg_focus
        } else {
            colors.field_bg
        };
    }
}

/// Highlights whichever of Edit/Perform is the current mode, and Lock when
/// `state.locked()` — which includes the forced-lock that Perform mode always
/// applies, not just the user's own toggle.
pub(super) fn update_mode_buttons(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ModeButton, &mut BackgroundColor)>,
) {
    let colors = theme.song_editor_colors();
    for (kind, mut bg) in &mut buttons {
        let active = match kind {
            ModeButton::Edit => state.mode == Mode::Edit,
            ModeButton::Perform => state.mode == Mode::Perform,
            ModeButton::Lock => state.locked(),
        };
        bg.0 = if active {
            colors.btn_active
        } else {
            colors.btn_bg
        };
    }
}

/// Shows the note-editing button cluster in [`Mode::Edit`] and the
/// playback/practice cluster in [`Mode::Perform`] — never both. Toggles
/// `Node::display`, not `Visibility`: `Visibility::Hidden` only skips
/// rendering and still reserves the hidden group's full layout width, which
/// would push the visible group off to the side instead of freeing its place.
pub(super) fn update_mode_visibility(
    state: Res<EditorState>,
    mut edit_group: Query<&mut Node, (With<EditModeGroup>, Without<PerformModeGroup>)>,
    mut perform_group: Query<&mut Node, (With<PerformModeGroup>, Without<EditModeGroup>)>,
) {
    for mut node in &mut edit_group {
        node.display = if state.mode == Mode::Edit {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut perform_group {
        node.display = if state.mode == Mode::Perform {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(super) fn update_status_bar(
    state: Res<EditorState>,
    practice: Res<PracticeState>,
    mut texts: Query<&mut Text, With<StatusMsg>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    // Drag messages take priority (they're ephemeral and action-specific).
    // Practice messages fill the bar while no drag is in progress.
    **text = if !state.drag_msg.is_empty() {
        state.drag_msg.to_string()
    } else {
        practice.msg.to_string()
    };
}
