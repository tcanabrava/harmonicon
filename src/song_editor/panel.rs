// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use super::practice::PracticeState;
use super::record::RecordState;
use super::state::{ContentKind, Dir, EditorState, Expr, Field, HarmonicaKind, Mode, Pitch};
use super::ui::{
    BendDot, ContentKindText, EditModeGroup, HarmonicaKindText, MetaFieldBox, MetaFieldText,
    ModButton, ModButtonLabel, ModeButton, PlayModeGroup, RecordModeGroup, StatusMsg,
    TimelineToolButton,
};
use crate::localization::LocalizationExt;
use crate::theme::LoadedTheme;
use bevy_fluent::prelude::Localization;

/// Whether `kind`'s button should read as "on" for a note carrying `dir`/
/// `pitch`/`expr` — shared by the selected-note case (an existing
/// `GridNote`'s own fields) and the nothing-selected case (`EditorState`'s
/// `sticky_dir`/`sticky_pitch`/`sticky_expr`, previewing what a *new* note
/// would get), so the two can't drift out of sync with each other.
fn mod_button_active(kind: ModButton, dir: Dir, pitch: Pitch, expr: Expr) -> bool {
    match kind {
        ModButton::Blow => dir == Dir::Blow,
        ModButton::Draw => dir == Dir::Draw,
        ModButton::Bend => matches!(pitch, Pitch::Bend(_)),
        ModButton::Overblow => pitch == Pitch::Overblow,
        ModButton::Overdraw => pitch == Pitch::Overdraw,
        ModButton::Slide => pitch == Pitch::Slide,
        ModButton::Wah => matches!(expr, Expr::Wah(_)),
        ModButton::Vibrato => matches!(expr, Expr::Vibrato(_)),
        ModButton::Delete => false,
    }
}

pub(super) fn update_mod_panel(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ModButton, &mut BackgroundColor)>,
    mut dot: Query<&mut Visibility, With<BendDot>>,
    mut labels: Query<(&ModButtonLabel, &mut Text)>,
) {
    let colors = theme.song_editor_colors();
    let selected = state.selected_note().copied();
    // The selected note's own fields take priority when there is one — a
    // sticky setting armed from an earlier, now-deselected note shouldn't
    // visually compete with what's actually selected right now. With
    // nothing selected, the sticky fields preview what a newly *added*
    // note would get, exactly matching `select_or_add`.
    let (dir, pitch, expr) = match selected {
        Some(n) => (n.dir, n.pitch, n.expr),
        None => (state.sticky_dir, state.sticky_pitch, state.sticky_expr),
    };
    for (kind, mut bg) in &mut buttons {
        let active = mod_button_active(*kind, dir, pitch, expr);
        bg.0 = if active {
            colors.btn_active
        } else {
            colors.btn_bg
        };
    }
    let bent = matches!(pitch, Pitch::Bend(_));
    for mut vis in &mut dot {
        *vis = if bent {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    // Show the selected (or, with nothing selected, sticky-armed) rate next
    // to Wah/Vibrato (e.g. "Vibrato 5Hz") so cycling the rate with repeated
    // clicks is legible.
    for (label, mut text) in &mut labels {
        let hz = match (label.kind, expr) {
            (ModButton::Vibrato, Expr::Vibrato(hz)) => Some(hz),
            (ModButton::Wah, Expr::Wah(hz)) => Some(hz),
            _ => None,
        };
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

/// Highlights whichever of Edit/Record/Play is the current mode, and Lock
/// when `state.locked()` — which includes the forced-lock that the Record
/// and Play modes always apply, not just the user's own toggle.
pub(super) fn update_mode_buttons(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&ModeButton, &mut BackgroundColor)>,
) {
    let colors = theme.song_editor_colors();
    for (kind, mut bg) in &mut buttons {
        let active = match kind {
            ModeButton::Edit => state.mode == Mode::Edit,
            ModeButton::Record => state.mode == Mode::Record,
            ModeButton::Play => state.mode == Mode::Play,
            ModeButton::Lock => state.locked(),
        };
        bg.0 = if active {
            colors.btn_active
        } else {
            colors.btn_bg
        };
    }
}

pub(super) fn update_timeline_tool_buttons(
    state: Res<EditorState>,
    theme: Res<LoadedTheme>,
    mut buttons: Query<(&TimelineToolButton, &mut BackgroundColor)>,
) {
    let colors = theme.song_editor_colors();
    for (kind, mut bg) in &mut buttons {
        bg.0 = if kind.0 == state.timeline_tool {
            colors.btn_active
        } else {
            colors.btn_bg
        };
    }
}

/// Shows exactly the current mode's button cluster — note editing in
/// [`Mode::Edit`], the recording transport in [`Mode::Record`], the
/// playback/practice transport in [`Mode::Play`] — never more than one.
/// Toggles `Node::display`, not `Visibility`: `Visibility::Hidden` only
/// skips rendering and still reserves the hidden group's full layout width,
/// which would push the visible group off to the side instead of freeing
/// its place.
pub(super) fn update_mode_visibility(
    state: Res<EditorState>,
    mut edit_group: Query<
        &mut Node,
        (
            With<EditModeGroup>,
            Without<RecordModeGroup>,
            Without<PlayModeGroup>,
        ),
    >,
    mut record_group: Query<
        &mut Node,
        (
            With<RecordModeGroup>,
            Without<EditModeGroup>,
            Without<PlayModeGroup>,
        ),
    >,
    mut play_group: Query<
        &mut Node,
        (
            With<PlayModeGroup>,
            Without<EditModeGroup>,
            Without<RecordModeGroup>,
        ),
    >,
) {
    let display = |on: bool| if on { Display::Flex } else { Display::None };
    for mut node in &mut edit_group {
        node.display = display(state.mode == Mode::Edit);
    }
    for mut node in &mut record_group {
        node.display = display(state.mode == Mode::Record);
    }
    for mut node in &mut play_group {
        node.display = display(state.mode == Mode::Play);
    }
}

/// Shows Bend/Overblow/Overdraw for a diatonic chart and Slide for a
/// chromatic one — never both, since the two harmonicas don't share
/// techniques. Mirrors [`update_mode_visibility`]'s `Node::display` approach.
pub(super) fn update_technique_button_visibility(
    state: Res<EditorState>,
    mut buttons: Query<(&ModButton, &mut Node)>,
) {
    let diatonic_only = matches!(state.harmonica_kind, HarmonicaKind::Diatonic);
    for (kind, mut node) in &mut buttons {
        let visible = match kind {
            ModButton::Bend | ModButton::Overblow | ModButton::Overdraw => diatonic_only,
            ModButton::Slide => !diatonic_only,
            _ => continue,
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
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

/// Keeps the record-mode toggle's label in sync with `state.content_kind`.
pub(super) fn update_content_kind_text(
    state: Res<EditorState>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<ContentKindText>>,
) {
    let key = match state.content_kind {
        ContentKind::Song => "editor-content-kind-song",
        ContentKind::Lesson => "editor-content-kind-lesson",
    };
    let label = String::from(loc.msg(key));
    for mut text in &mut texts {
        **text = label.clone();
    }
}

pub(super) fn update_status_bar(
    state: Res<EditorState>,
    practice: Res<PracticeState>,
    record: Res<RecordState>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<StatusMsg>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    // Drag messages take priority (they're ephemeral and action-specific);
    // a live recording comes next (it's actively running, unlike the
    // practice message which just sits there between hits); practice
    // messages fill the bar otherwise.
    **text = if !state.drag_msg.is_empty() {
        state.drag_msg.to_string()
    } else if record.active {
        loc.msg_args(
            "editor-record-status",
            &[("count", record.note_count.to_string())],
        )
        .to_string()
    } else {
        practice.msg.to_string()
    };
}

#[cfg(test)]
mod tests {
    use super::super::state::GridNote;
    use super::*;

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
            selected: vec![1],
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
    fn update_mod_panel_falls_back_to_sticky_defaults_when_nothing_selected() {
        let mut world = World::new();
        world.insert_resource(EditorState::default());
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        // `EditorState::default()`'s `sticky_dir` is `Dir::Blow` — a note
        // added right now would be Blow, so the Blow button should already
        // read "on" even though nothing is selected. Every pitch/expr
        // button stays off, since `sticky_pitch`/`sticky_expr` default to
        // their own "off" variants.
        let blow = world
            .spawn((ModButton::Blow, BackgroundColor(colors.btn_bg)))
            .id();
        let draw = world
            .spawn((ModButton::Draw, BackgroundColor(colors.btn_active)))
            .id();
        let bend = world
            .spawn((ModButton::Bend, BackgroundColor(colors.btn_active)))
            .id();
        let bend_dot = world.spawn((BendDot, Visibility::Inherited)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mod_panel);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<BackgroundColor>(blow).unwrap().0,
            colors.btn_active
        );
        assert_eq!(world.get::<BackgroundColor>(draw).unwrap().0, colors.btn_bg);
        assert_eq!(world.get::<BackgroundColor>(bend).unwrap().0, colors.btn_bg);
        assert_eq!(
            *world.get::<Visibility>(bend_dot).unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn update_mod_panel_reflects_an_armed_sticky_modifier_when_nothing_selected() {
        let mut world = World::new();
        world.insert_resource(EditorState {
            sticky_dir: Dir::Draw,
            sticky_pitch: Pitch::Bend(1.0),
            sticky_expr: Expr::Wah(3.0),
            ..Default::default()
        });
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let draw = world
            .spawn((ModButton::Draw, BackgroundColor(colors.btn_bg)))
            .id();
        let bend = world
            .spawn((ModButton::Bend, BackgroundColor(colors.btn_bg)))
            .id();
        let wah = world
            .spawn((ModButton::Wah, BackgroundColor(colors.btn_bg)))
            .id();
        let bend_dot = world.spawn((BendDot, Visibility::Hidden)).id();
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
            world.get::<BackgroundColor>(draw).unwrap().0,
            colors.btn_active
        );
        assert_eq!(
            world.get::<BackgroundColor>(bend).unwrap().0,
            colors.btn_active
        );
        assert_eq!(
            world.get::<BackgroundColor>(wah).unwrap().0,
            colors.btn_active
        );
        assert_eq!(
            *world.get::<Visibility>(bend_dot).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(world.get::<Text>(wah_label).unwrap().0, "Wah 3Hz");
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
            mode: Mode::Play,
            ..Default::default()
        };
        world.insert_resource(state);
        world.insert_resource(LoadedTheme::default());
        let colors = LoadedTheme::default().song_editor_colors();

        let edit = world
            .spawn((ModeButton::Edit, BackgroundColor(colors.btn_active)))
            .id();
        let record = world
            .spawn((ModeButton::Record, BackgroundColor(colors.btn_active)))
            .id();
        let play = world
            .spawn((ModeButton::Play, BackgroundColor(colors.btn_bg)))
            .id();
        // Play mode is always locked, even without the user's own toggle.
        let lock = world
            .spawn((ModeButton::Lock, BackgroundColor(colors.btn_bg)))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mode_buttons);
        schedule.run(&mut world);

        assert_eq!(world.get::<BackgroundColor>(edit).unwrap().0, colors.btn_bg);
        assert_eq!(
            world.get::<BackgroundColor>(record).unwrap().0,
            colors.btn_bg
        );
        assert_eq!(
            world.get::<BackgroundColor>(play).unwrap().0,
            colors.btn_active
        );
        assert_eq!(
            world.get::<BackgroundColor>(lock).unwrap().0,
            colors.btn_active,
            "Play mode forces Lock active regardless of user_locked"
        );
    }

    // ── update_mode_visibility ────────────────────────────────────────────────

    #[test]
    fn update_mode_visibility_shows_only_the_current_modes_group() {
        let mut world = World::new();
        let state = EditorState {
            mode: Mode::Play,
            ..Default::default()
        };
        world.insert_resource(state);

        let edit_group = world.spawn((EditModeGroup, Node::default())).id();
        let record_group = world.spawn((RecordModeGroup, Node::default())).id();
        let play_group = world.spawn((PlayModeGroup, Node::default())).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_mode_visibility);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<Node>(edit_group).unwrap().display,
            Display::None
        );
        assert_eq!(
            world.get::<Node>(record_group).unwrap().display,
            Display::None
        );
        assert_eq!(
            world.get::<Node>(play_group).unwrap().display,
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
        world.insert_resource(RecordState::default());
        world.insert_resource(loc);

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
        world.insert_resource(RecordState::default());
        world.insert_resource(loc);

        let status = world.spawn((StatusMsg, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_status_bar);
        schedule.run(&mut world);

        assert_eq!(world.get::<Text>(status).unwrap().0, "editor-practice-msg");
    }

    #[test]
    fn update_status_bar_prefers_the_recording_message_over_the_practice_message() {
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
        // `RecordState::open` is private to the `record` module, so build
        // this via field mutation (`active` is `pub(super)`) rather than a
        // struct literal, which would need every field visible here.
        #[allow(clippy::field_reassign_with_default)]
        let record = {
            let mut r = RecordState::default();
            r.active = true;
            r
        };
        world.insert_resource(record);
        world.insert_resource(loc);

        let status = world.spawn((StatusMsg, Text::new(""))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_status_bar);
        schedule.run(&mut world);

        // `Localization::default()` has no bundle loaded, so `loc.msg_args`
        // falls back to returning the key itself — just confirm it picked
        // the recording branch over the (also-set) practice message.
        assert_eq!(world.get::<Text>(status).unwrap().0, "editor-record-status");
    }
}
