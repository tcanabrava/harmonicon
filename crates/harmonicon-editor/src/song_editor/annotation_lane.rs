// SPDX-License-Identifier: MIT

//! Compact phrase metadata rendered between the beat ruler and waveform.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::SongEditorColors;
use harmonicon_ui::dialogs::tooltip::Tooltip;

use super::state::{EditorState, PhraseAnnotation};
use super::ui::GridItem;
use super::{ANNOTATION_H, ANNOTATION_TOP, HEADER_H, TICK_W};

const MAX_WIDTH: f32 = 150.0;

pub(super) fn label(annotation: &PhraseAnnotation) -> String {
    let mut parts = Vec::new();
    if let Some(section) = annotation.section.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("§ {section}"));
    }
    if let Some(lyric) = annotation.lyric.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("\"{lyric}\""));
    }
    if let Some(chord) = annotation.chord.as_deref().filter(|s| !s.is_empty()) {
        parts.push(format!("♬ {chord}"));
    }
    if annotation.call {
        parts.push("↩".to_string());
    }
    if annotation.split {
        parts.push("TB".to_string());
    }
    if let Some(groove) = annotation.groove.as_deref().filter(|s| !s.is_empty()) {
        parts.push(groove.to_string());
    }
    parts.join(" · ")
}

pub(super) fn width(tick: usize, next_tick: Option<usize>) -> f32 {
    let x = tick as f32 * TICK_W;
    let next_x = next_tick.map_or(x + MAX_WIDTH + 3.0, |next| next as f32 * TICK_W);
    (next_x - x - 3.0).clamp(4.0, MAX_WIDTH)
}

pub(super) fn spawn(
    commands: &mut Commands,
    items: &mut Vec<Entity>,
    state: &EditorState,
    first_tick: usize,
    last_tick: usize,
    colors: SongEditorColors,
    loc: &Localization,
) {
    let mut visible = state
        .phrase_annotations
        .range(first_tick..last_tick)
        .filter_map(|(&tick, annotation)| {
            let text = label(annotation);
            (!text.is_empty()).then_some((tick, annotation, text))
        })
        .peekable();
    while let Some((tick, annotation, text)) = visible.next() {
        let x = tick as f32 * TICK_W;
        let next_tick = visible.peek().map(|(tick, _, _)| *tick);
        let tooltip = loc.msg_args(
            "editor-phrase-marker-tooltip",
            &[("tick", tick.to_string()), ("details", text.clone())],
        );
        // A real button so a click opens the phrase editor through
        // `Activate` — and no `TabIndex`, since markers are respawned on
        // every rebuild and the toolbar is the keyboard route to the same
        // editor. Clicking also selects every note of the phrase, so a
        // following drag or Delete acts on the whole phrase.
        items.push(
            commands
                .spawn_empty()
                .apply_scene(bsn! {
                    WidgetButton
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Px(x)},
                        top: {Val::Px(ANNOTATION_TOP)},
                        width: {Val::Px(width(tick, next_tick))},
                        height: {Val::Px(ANNOTATION_H)},
                        padding: {UiRect::horizontal(Val::Px(3.0))},
                        overflow: {Overflow::clip()},
                    }
                    BackgroundColor({colors.accent.with_alpha(0.18)})
                    Text({text})
                    TextFont { font_size: {FontSize::Px(11.0)} }
                    TextColor({colors.label})
                    Tooltip({String::from(tooltip)})
                })
                .insert(GridItem)
                .observe(move |_: On<Activate>, mut state: ResMut<EditorState>| {
                    state.open_phrase_editor(tick);
                })
                .id(),
        );
        if annotation.section.is_some() {
            items.push(
                commands
                    .spawn((
                        GridItem,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x),
                            top: Val::Px(ANNOTATION_TOP),
                            width: Val::Px(2.0),
                            height: Val::Px(HEADER_H - ANNOTATION_TOP),
                            ..default()
                        },
                        BackgroundColor(colors.accent),
                        Pickable::IGNORE,
                    ))
                    .id(),
            );
        }
    }
}
