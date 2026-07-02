// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::{BTN_ACTIVE, BTN_BG, FIELD_BG, FIELD_BG_FOCUS};
use super::practice::PracticeState;
use super::state::{Dir, EditorState, Expr, Field, Pitch};
use super::ui::{BendDot, MetaFieldBox, MetaFieldText, ModButton, StatusMsg};

pub(super) fn update_mod_panel(
    state: Res<EditorState>,
    mut buttons: Query<(&ModButton, &mut BackgroundColor)>,
    mut dot: Query<&mut Visibility, With<BendDot>>,
) {
    let selected = state.selected_note().copied();
    for (kind, mut bg) in &mut buttons {
        let active = selected.is_some_and(|n| match kind {
            ModButton::Blow     => n.dir == Dir::Blow,
            ModButton::Draw     => n.dir == Dir::Draw,
            ModButton::Bend     => matches!(n.pitch, Pitch::Bend(_)),
            ModButton::Overblow => n.pitch == Pitch::Overblow,
            ModButton::Overdraw => n.pitch == Pitch::Overdraw,
            ModButton::Wah      => n.expr == Expr::Wah,
            ModButton::Vibrato  => n.expr == Expr::Vibrato,
            ModButton::Delete   => false,
        });
        bg.0 = if active { BTN_ACTIVE } else { BTN_BG };
    }
    let bent = selected.is_some_and(|n| matches!(n.pitch, Pitch::Bend(_)));
    for mut vis in &mut dot {
        *vis = if bent { Visibility::Inherited } else { Visibility::Hidden };
    }
}

pub(super) fn update_meta_fields(
    state: Res<EditorState>,
    mut texts: Query<(&MetaFieldText, &mut Text)>,
    mut boxes: Query<(&MetaFieldBox, &mut BackgroundColor)>,
) {
    for (tag, mut text) in &mut texts {
        **text = if tag.0 == Field::Key {
            format!("\u{2039}  {}  \u{203A}", state.key)
        } else {
            let mut s = state.field_text(tag.0).to_string();
            if state.focus == Some(tag.0) { s.push('_'); }
            s
        };
    }
    for (tag, mut bg) in &mut boxes {
        bg.0 = if tag.0 != Field::Key && state.focus == Some(tag.0) {
            FIELD_BG_FOCUS
        } else {
            FIELD_BG
        };
    }
}

pub(super) fn update_status_bar(
    state:    Res<EditorState>,
    practice: Res<PracticeState>,
    mut texts: Query<&mut Text, With<StatusMsg>>,
) {
    let Ok(mut text) = texts.single_mut() else { return };
    // Drag messages take priority (they're ephemeral and action-specific).
    // Practice messages fill the bar while no drag is in progress.
    **text = if !state.drag_msg.is_empty() {
        state.drag_msg.clone()
    } else {
        practice.msg.clone()
    };
}
