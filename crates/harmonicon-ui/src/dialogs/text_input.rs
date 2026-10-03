// SPDX-License-Identifier: MIT

//! Minimal numeric, single-line, and multiline text boxes.
//! `bevy_ui_widgets`' `TextInputPlugin` — already registered
//! app-wide via `UiWidgetsPlugins` — supplies click-to-focus, cursor
//! rendering, and keyboard editing for any entity carrying
//! `bevy_ui_widgets::TextInput` and `bevy_text::EditableText`; this module adds:
//! a bordered box, digit-only filtering for the numeric variant, and
//! clamped/committed value reporting on Enter or losing focus, the same
//! shape `dialogs::combobox::ComboboxSelect` reports a pick.

use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::{FocusLost, InputFocus, tab_navigation::TabIndex};
use bevy::prelude::*;
use bevy::text::{EditableText, EditableTextFilter, TextCursorStyle, TextEdit};
use bevy::ui_widgets::TextInput;

// ── Numeric input ────────────────────────────────────────────────────────────

/// Fired when a numeric input commits a new value (Enter, or losing focus)
/// — already parsed and clamped to the range it was spawned with.
#[derive(Clone, Copy, Debug, EntityEvent)]
pub struct NumericInputCommitted {
    #[event_target]
    pub input: Entity,
    pub value: f32,
}

/// The valid range + last-committed value for a numeric input — carried on
/// the entity itself so [`commit_on_enter`] can validate whichever input
/// happens to be focused without the caller threading its own range through.
#[derive(Component, Clone, Copy)]
struct NumericInputState {
    min: f32,
    max: f32,
    last_value: f32,
}

/// Spawns a bordered numeric text box showing `value` (clamped to
/// `min..=max`) as a child of `parent`. `on_commit` fires with the parsed,
/// clamped value on Enter or on losing focus — non-numeric or empty input
/// reverts to the last committed value rather than being accepted, and the
/// displayed text is corrected to match whatever actually committed.
pub fn spawn_numeric_input<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    value: f32,
    min: f32,
    max: f32,
    bg: Color,
    border: Color,
    on_commit: impl IntoObserverSystem<NumericInputCommitted, M>,
) -> Entity {
    let value = value.clamp(min, max);
    let input = commands
        .spawn((
            Node {
                width: Val::Px(70.0),
                border: UiRect::all(Val::Px(1.5)),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            TextInput,
            BorderColor::all(border),
            BackgroundColor(bg),
            EditableText {
                visible_width: Some(3.5),
                max_characters: Some(3),
                ..EditableText::new(format!("{value:.0}"))
            },
            EditableTextFilter::new(|c: char| c.is_ascii_digit()),
            TextLayout::no_wrap(),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            TextColor(Color::WHITE),
            TextCursorStyle { color: Color::WHITE, ..default() },
            TabIndex(0),
            NumericInputState { min, max, last_value: value },
        ))
        .id();
    commands.entity(input).observe(on_commit);
    commands.entity(input).observe(commit_on_blur);
    commands.entity(parent).add_child(input);
    input
}

/// Parses/clamps the current buffer, corrects the displayed text if it
/// didn't already match, and fires [`NumericInputCommitted`] — shared by
/// both commit paths ([`commit_on_enter`], [`commit_on_blur`]).
fn commit(
    entity: Entity,
    text: &mut EditableText,
    state: &mut NumericInputState,
    commands: &mut Commands,
) {
    let raw = text.value().to_string();
    let value = raw
        .trim()
        .parse::<f32>()
        .map(|v| v.clamp(state.min, state.max))
        .unwrap_or(state.last_value);
    state.last_value = value;
    let formatted = format!("{value:.0}");
    if raw != formatted {
        text.editor_mut().set_text(&formatted);
        text.queue_edit(TextEdit::TextEnd(false));
    }
    commands.trigger(NumericInputCommitted { input: entity, value });
}

fn commit_on_blur(
    trigger: On<FocusLost>,
    mut inputs: Query<(&mut EditableText, &mut NumericInputState)>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    let Ok((mut text, mut state)) = inputs.get_mut(entity) else {
        return;
    };
    commit(entity, &mut text, &mut state, &mut commands);
}

/// Commits whichever numeric input currently has focus when Enter is
/// pressed. `EditableText`'s own keyboard handling only treats Enter as
/// "insert a newline", and only when `allow_newlines` is set — which this
/// widget never does — so nothing else in `bevy_ui_widgets` reacts to Enter
/// on it (mirrors the crate's own `text_input` example's `text_submission`
/// system).
fn commit_on_enter(
    input_focus: Res<InputFocus>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inputs: Query<(Entity, &mut EditableText, &mut NumericInputState)>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }
    let Some(focused) = input_focus.get() else {
        return;
    };
    let Ok((entity, mut text, mut state)) = inputs.get_mut(focused) else {
        return;
    };
    commit(entity, &mut text, &mut state, &mut commands);
}

// ── Plain text input ─────────────────────────────────────────────────────────

/// Fired when a text input commits its current buffer (Enter, or losing
/// focus) — verbatim, no parsing/validation (unlike [`NumericInputCommitted`]).
#[derive(Clone, Debug, EntityEvent)]
pub struct TextInputCommitted {
    #[event_target]
    pub input: Entity,
    pub value: String,
}

/// Marks an entity as one of this widget's inputs, so [`commit_text_on_enter`]
/// can find whichever one is focused. Unlike the numeric widget's
/// `NumericInputState`, there's no range/last-value to carry — any string
/// commits as-is.
#[derive(Component, Clone, Copy, Default)]
struct TextInputState {
    commit_on_enter: bool,
}

/// Spawns a bordered plain-text box showing `value`, `width` px wide, as a
/// child of `parent`. `on_commit` fires with the buffer's current text on
/// Enter or on losing focus.
pub fn spawn_text_input<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    value: &str,
    width: f32,
    bg: Color,
    border: Color,
    on_commit: impl IntoObserverSystem<TextInputCommitted, M>,
) -> Entity {
    let input = commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            TextInput,
            BorderColor::all(border),
            BackgroundColor(bg),
            EditableText::new(value),
            TextLayout::no_wrap(),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(Color::WHITE),
            TextCursorStyle { color: Color::WHITE, ..default() },
            TabIndex(0),
            TextInputState { commit_on_enter: true },
        ))
        .id();
    commands.entity(input).observe(on_commit);
    commands.entity(input).observe(commit_text_on_blur);
    commands.entity(parent).add_child(input);
    input
}

/// Spawns a four-line, word-wrapped text box. Enter inserts a newline;
/// losing focus commits the buffer.
pub fn spawn_multiline_text_input<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    value: &str,
    width: f32,
    bg: Color,
    border: Color,
    on_commit: impl IntoObserverSystem<TextInputCommitted, M>,
) -> Entity {
    let mut editable = EditableText::new(value);
    editable.visible_lines = Some(4.0);
    editable.allow_newlines = true;
    editable.queue_edit(TextEdit::TextStart(false));
    let input = commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(72.0),
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            TextInput,
            BorderColor::all(border),
            BackgroundColor(bg),
            editable,
            TextLayout::default(),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(Color::WHITE),
            TextCursorStyle { color: Color::WHITE, ..default() },
            TabIndex(0),
            TextInputState { commit_on_enter: false },
        ))
        .id();
    commands.entity(input).observe(on_commit);
    commands.entity(input).observe(commit_text_on_blur);
    commands.entity(parent).add_child(input);
    input
}

fn commit_text(entity: Entity, text: &EditableText, commands: &mut Commands) {
    commands.trigger(TextInputCommitted { input: entity, value: text.value().to_string() });
}

fn commit_text_on_blur(
    trigger: On<FocusLost>,
    inputs: Query<&EditableText, With<TextInputState>>,
    mut commands: Commands,
) {
    let entity = trigger.entity;
    let Ok(text) = inputs.get(entity) else {
        return;
    };
    commit_text(entity, text, &mut commands);
}

/// Commits whichever text input currently has focus when Enter is pressed
/// — see [`commit_on_enter`] (the numeric sibling) for why nothing else
/// reacts to Enter on this widget.
fn commit_text_on_enter(
    input_focus: Res<InputFocus>,
    keyboard: Res<ButtonInput<KeyCode>>,
    inputs: Query<(Entity, &EditableText, &TextInputState)>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::Enter) {
        return;
    }
    let Some(focused) = input_focus.get() else {
        return;
    };
    let Ok((entity, text, state)) = inputs.get(focused) else {
        return;
    };
    if !state.commit_on_enter {
        return;
    }
    commit_text(entity, text, &mut commands);
}

pub struct TextInputPlugin;

impl Plugin for TextInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (commit_on_enter, commit_text_on_enter));
    }
}
