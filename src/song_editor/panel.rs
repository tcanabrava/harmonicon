// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::practice::PracticeState;
use super::state::{Dir, EditorState, Expr, Field, HarmonicaKind, Mode, Pitch};
use super::ui::{
    BendDot, EditModeGroup, HarmonicaKindText, MetaFieldBox, MetaFieldText, ModButton,
    ModButtonLabel, ModeButton, PerformModeGroup, StatusMsg,
};
use crate::localization::LocalizationExt;
use crate::theme::LoadedTheme;
use bevy_fluent::prelude::Localization;

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
            ModButton::Slide => n.pitch == Pitch::Slide,
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

/// Shows Bend/Overblow/Overdraw for a diatonic chart and Slide for a
/// chromatic one — never both, since the two harmonicas don't share
/// techniques. Mirrors [`update_mode_visibility`]'s `Node::display` approach.
pub(super) fn update_technique_button_visibility(
    state: Res<EditorState>,
    mut buttons: Query<(&ModButton, &mut Node)>,
) {
    let diatonic_only = matches!(
        state.harmonica_kind,
        HarmonicaKind::Diatonic
    );
    for (kind, mut node) in &mut buttons {
        let visible = match kind {
            ModButton::Bend | ModButton::Overblow | ModButton::Overdraw => diatonic_only,
            ModButton::Slide => !diatonic_only,
            _ => continue,
        };
        node.display = if visible { Display::Flex } else { Display::None };
    }
}

/// Keeps the harmonica-kind toggle's label in sync with `state.harmonica_kind`.
pub(super) fn update_harmonica_kind_text(
    state: Res<EditorState>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<HarmonicaKindText>>,
) {
    let key = match state.harmonica_kind {
        HarmonicaKind::Diatonic => "editor-harmonica-diatonic",
        HarmonicaKind::Chromatic => "editor-harmonica-chromatic",
    };
    let label = String::from(loc.msg(key));
    for mut text in &mut texts {
        **text = label.clone();
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::state::GridNote;

    fn note(dir: Dir, pitch: Pitch, expr: Expr) -> GridNote {
        GridNote {
            id: 1,
            hole: 4,
            tick: 0,
            len: 1,
            dir,
            pitch,
            expr,
        }
    }

    // ── update_mod_panel ─────────────────────────────────────────────────────

    #[test]
    fn update_mod_panel_reflects_the_selected_notes_direction_pitch_and_expr_rate() {
        let mut world = World::new();
        let state = EditorState {
            notes: vec![note(Dir::Blow, Pitch::Bend(0.5), Expr::Vibrato(5.0))],
            selected: Some(1),
            ..Default::default()
        };
        world.insert_resource(state);
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let blow = world
            .spawn((ModButton::Blow, BackgroundColor(colors.btn_bg)))
            .id();
        let draw = world
            .spawn((ModButton::Draw, BackgroundColor(colors.btn_bg)))
            .id();
        let bend_dot = world.spawn((BendDot, Visibility::Hidden)).id();
        let vibrato_label = world
            .spawn((
                ModButtonLabel {
                    kind: ModButton::Vibrato,
                    base: "Vibrato".into(),
                },
                Text::new("Vibrato"),
            ))
            .id();
        let wah_label = world
            .spawn((
                ModButtonLabel {
                    kind: ModButton::Wah,
                    base: "Wah".into(),
                },
                Text::new("Wah"),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<BackgroundColor>(blow).unwrap().0,
            colors.btn_active,
            "the selected note's direction button should highlight"
        );
        assert_eq!(
            world.get::<BackgroundColor>(draw).unwrap().0,
            colors.btn_bg,
            "the other direction button should not"
        );
        assert_eq!(
            *world.get::<Visibility>(bend_dot).unwrap(),
            Visibility::Inherited,
            "a bent note shows the bend dot"
        );
        assert_eq!(world.get::<Text>(vibrato_label).unwrap().0, "Vibrato 5Hz");
        assert_eq!(
            world.get::<Text>(wah_label).unwrap().0,
            "Wah",
            "a mismatched expr kind keeps just the base label"
        );
    }

    #[test]
    fn update_mod_panel_hides_the_bend_dot_and_deactivates_every_button_when_nothing_selected() {
        let mut world = World::new();
        world.insert_resource(EditorState::default());
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let blow = world
            .spawn((ModButton::Blow, BackgroundColor(colors.btn_active)))
            .id();
        let bend_dot = world.spawn((BendDot, Visibility::Inherited)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);

        assert_eq!(world.get::<BackgroundColor>(blow).unwrap().0, colors.btn_bg);
        assert_eq!(*world.get::<Visibility>(bend_dot).unwrap(), Visibility::Hidden);
    }

    // ── update_meta_fields ────────────────────────────────────────────────────

    #[test]
    fn update_meta_fields_formats_the_key_field_specially_and_marks_focus_elsewhere() {
        let mut world = World::new();
        let state = EditorState {
            key: "G".into(),
            tempo: "140".into(),
            focus: Some(Field::Tempo),
            ..Default::default()
        };
        world.insert_resource(state);
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let key_text = world.spawn((MetaFieldText(Field::Key), Text::new(""))).id();
        let tempo_text = world
            .spawn((MetaFieldText(Field::Tempo), Text::new("")))
            .id();
        let tempo_box = world
            .spawn((MetaFieldBox(Field::Tempo), BackgroundColor(colors.field_bg)))
            .id();
        let key_box = world
            .spawn((MetaFieldBox(Field::Key), BackgroundColor(colors.field_bg)))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_meta_fields);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<Text>(key_text).unwrap().0,
            "\u{2039}  G  \u{203A}"
        );
        assert_eq!(
            world.get::<Text>(tempo_text).unwrap().0,
            "140_",
            "the focused field gets a trailing cursor"
        );
        assert_eq!(
            world.get::<BackgroundColor>(tempo_box).unwrap().0,
            colors.field_bg_focus
        );
        assert_eq!(
            world.get::<BackgroundColor>(key_box).unwrap().0,
            colors.field_bg,
            "Key never highlights as focused, even if it somehow were"
        );
    }

    // ── update_mode_buttons ───────────────────────────────────────────────────

    #[test]
    fn update_mode_buttons_highlights_the_active_mode_and_lock_state() {
        let mut world = World::new();
        let state = EditorState {
            mode: Mode::Perform,
            ..Default::default()
        };
        world.insert_resource(state);
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let edit = world
            .spawn((ModeButton::Edit, BackgroundColor(colors.btn_active)))
            .id();
        let perform = world
            .spawn((ModeButton::Perform, BackgroundColor(colors.btn_bg)))
            .id();
        // Perform mode is always locked, even without the user's own toggle.
        let lock = world
            .spawn((ModeButton::Lock, BackgroundColor(colors.btn_bg)))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mode_buttons);
        schedule.run(&mut world);

        assert_eq!(world.get::<BackgroundColor>(edit).unwrap().0, colors.btn_bg);
        assert_eq!(
            world.get::<BackgroundColor>(perform).unwrap().0,
            colors.btn_active
        );
        assert_eq!(
            world.get::<BackgroundColor>(lock).unwrap().0,
            colors.btn_active,
            "Perform mode forces Lock active regardless of user_locked"
        );
    }

    // ── update_mode_visibility ────────────────────────────────────────────────

    #[test]
    fn update_mode_visibility_shows_only_the_current_modes_group() {
        let mut world = World::new();
        let state = EditorState {
            mode: Mode::Perform,
            ..Default::default()
        };
        world.insert_resource(state);

        let edit_group = world.spawn((EditModeGroup, Node::default())).id();
        let perform_group = world.spawn((PerformModeGroup, Node::default())).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mode_visibility);
        schedule.run(&mut world);

        assert_eq!(world.get::<Node>(edit_group).unwrap().display, Display::None);
        assert_eq!(
            world.get::<Node>(perform_group).unwrap().display,
            Display::Flex
        );
    }

    // ── update_technique_button_visibility ────────────────────────────────────

    #[test]
    fn update_technique_button_visibility_shows_bend_family_for_diatonic_and_slide_for_chromatic() {
        let mut world = World::new();
        let state = EditorState {
            harmonica_kind: HarmonicaKind::Chromatic,
            ..Default::default()
        };
        world.insert_resource(state);

        let bend = world.spawn((ModButton::Bend, Node::default())).id();
        let slide = world.spawn((ModButton::Slide, Node::default())).id();
        // Untouched by either branch — must be left exactly as spawned.
        let blow = world
            .spawn((
                ModButton::Blow,
                Node {
                    display: Display::Grid,
                    ..default()
                },
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_technique_button_visibility);
        schedule.run(&mut world);

        assert_eq!(world.get::<Node>(bend).unwrap().display, Display::None);
        assert_eq!(world.get::<Node>(slide).unwrap().display, Display::Flex);
        assert_eq!(
            world.get::<Node>(blow).unwrap().display,
            Display::Grid,
            "buttons outside the bend/slide family are never touched"
        );
    }

    // ── update_harmonica_kind_text ────────────────────────────────────────────

    #[test]
    fn update_harmonica_kind_text_keys_off_the_current_harmonica_kind() {
        let mut world = World::new();
        let state = EditorState {
            harmonica_kind: HarmonicaKind::Chromatic,
            ..Default::default()
        };
        world.insert_resource(state);
        world.insert_resource(Localization::default());

        let label = world.spawn((HarmonicaKindText, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_harmonica_kind_text);
        schedule.run(&mut world);

        // No FTL bundle is loaded, so `loc.msg` falls back to the key itself
        // — enough to confirm the right key was chosen for this kind.
        assert_eq!(
            world.get::<Text>(label).unwrap().0,
            "editor-harmonica-chromatic"
        );
    }

    // ── update_status_bar ─────────────────────────────────────────────────────

    #[test]
    fn update_status_bar_prefers_the_drag_message_over_the_practice_message() {
        let mut world = World::new();
        let loc = Localization::default();
        let state = EditorState {
            drag_msg: loc.msg("editor-drag-msg"),
            ..Default::default()
        };
        world.insert_resource(state);
        // `PracticeState` has private fields not reachable from here, so a
        // `..Default::default()` struct literal isn't an option — only the
        // in-module `#[cfg(test)]` helpers get that.
        #[allow(clippy::field_reassign_with_default)]
        let practice = {
            let mut p = PracticeState::default();
            p.msg = loc.msg("editor-practice-msg");
            p
        };
        world.insert_resource(practice);

        let status = world.spawn((StatusMsg, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_status_bar);
        schedule.run(&mut world);

        assert_eq!(world.get::<Text>(status).unwrap().0, "editor-drag-msg");
    }

    #[test]
    fn update_status_bar_falls_back_to_the_practice_message_when_no_drag_is_in_progress() {
        let mut world = World::new();
        let loc = Localization::default();
        world.insert_resource(EditorState::default());
        #[allow(clippy::field_reassign_with_default)]
        let practice = {
            let mut p = PracticeState::default();
            p.msg = loc.msg("editor-practice-msg");
            p
        };
        world.insert_resource(practice);

        let status = world.spawn((StatusMsg, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_status_bar);
        schedule.run(&mut world);

        assert_eq!(world.get::<Text>(status).unwrap().0, "editor-practice-msg");
    }
}
