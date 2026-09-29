// SPDX-License-Identifier: MIT

//! The click-to-edit popover for a phrase's section, chord, groove and lyric —
//! the text half of the annotation lane (`annotation_lane` draws the
//! markers; clicking one opens this beneath it).
//!
//! Why a popover, and why *this* popover shape:
//!
//! - Section/chord/groove/lyric are text, so unlike every other per-note or
//!   per-phrase control they can't live on the toolbar as buttons.
//! - `bevy_ui_widgets::Popover` positions off its ECS parent, and a lane
//!   marker is a `GridItem` — despawned and respawned by every
//!   `rebuild_grid`. A text field childed to one would lose focus
//!   mid-typing. So this is a **persistent** entity (spawned once, like the
//!   resize grips and `timeline_overlay::TimelineSurface`), positioned per
//!   frame by [`update_phrase_editor`] rather than by `Popover`.
//! - It's a child of `GridArea`, not `GridContent`: `GridArea` is the clip
//!   container and doesn't scroll, so the position is
//!   `tick * TICK_W - Scroll::px` in the area's own coordinates, clamped to
//!   its width — no global-transform conversion, and no risk of the panel
//!   being scrolled off with the grid or clipped by the grid's right edge.
//!
//! [`EditorState::phrase_editor`] holds the onset tick being edited. It is
//! a UI preference like `snap_mode` — not chart content, not undo-tracked.
//! Every write goes through `EditorState::set_annotation(tick, …)`, which
//! refuses to annotate a tick no note starts on, and the popover closes
//! itself the moment its onset loses its notes (an undo, a delete) — an
//! annotation with no phrase is exactly what `metadata_sync` drops as an
//! orphan, so typing into one would be typing into a bin.

use bevy::input_focus::InputFocus;
use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::{EditableText, TextEdit};
use bevy::ui::ComputedNode;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::{LoadedTheme, SongEditorColors};
use harmonicon_ui::dialogs::button::make_interactive;
use harmonicon_ui::dialogs::text_input::{TextInputCommitted, spawn_text_input};
use harmonicon_ui::dialogs::tooltip::Tooltip;

use super::metronome::MeterClockCache;
use super::state::{EditorState, Field, Scroll};
use super::timeline::describe_tick;
use super::ui::GridArea;
use super::{ANNOTATION_H, ANNOTATION_TOP, TICK_W};

/// The popover's root — a persistent child of `GridArea`, `Display::None`
/// while closed.
#[derive(Component)]
pub(super) struct PhraseEditor;

/// One of the four text boxes, tagged with the annotation field it edits.
#[derive(Component)]
pub(super) struct PhraseEditorBox(pub(super) Field);

/// The "Phrase at 3.2" heading, rewritten as the edited onset changes.
#[derive(Component)]
pub(super) struct PhraseEditorTitle;

pub(super) const WIDTH: f32 = 280.0;
const LABEL_W: f32 = 58.0;
const INPUT_W: f32 = 190.0;
const GAP_BELOW_LANE: f32 = 4.0;

/// The four annotation fields the popover edits, in display order, with
/// their own short labels — the Details form's "Section at selected note"
/// wording is for a form with no other context, and wraps to three lines
/// in a panel this narrow.
const FIELDS: [(Field, &str); 4] = [
    (Field::Section, "editor-phrase-editor-section"),
    (Field::Chord, "editor-phrase-editor-chord"),
    (Field::Groove, "editor-phrase-editor-groove"),
    (Field::Lyric, "editor-phrase-editor-lyric"),
];

/// Spawns the (empty, hidden) root under `grid_area`. Its contents need a
/// `Commands` and a real parent entity (`spawn_text_input` does), so they
/// are filled in by [`populate_phrase_editor`] on a later frame — the same
/// spawn-once gate `meta_form::spawn_scale_combobox` uses.
pub(super) fn spawn_phrase_editor(
    grid_area: &mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>,
    colors: SongEditorColors,
) {
    grid_area.spawn((
        PhraseEditor,
        // A modal tab group: Tab cycles the four boxes and the close
        // button, and can't wander into the grid behind the popover —
        // tab navigation reaches invisible nodes otherwise.
        TabGroup::modal(),
        GlobalZIndex(200),
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            width: Val::Px(WIDTH),
            padding: UiRect::all(Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(colors.panel_bg),
        BorderColor::all(colors.accent),
    ));
}

/// Fills in [`PhraseEditor`] the first time it's seen empty.
pub(super) fn populate_phrase_editor(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    root: Query<Entity, (With<PhraseEditor>, Without<Children>)>,
) {
    let Ok(root) = root.single() else {
        return;
    };
    let colors = theme.song_editor_colors();

    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();
    commands.entity(header).with_children(|h| {
        h.spawn((
            PhraseEditorTitle,
            Text::new(String::new()),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(colors.accent),
            Pickable::IGNORE,
        ));
        let mut close = h.spawn((
            WidgetButton,
            TabIndex(0),
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Tooltip(String::from(loc.msg("editor-phrase-editor-close"))),
        ));
        make_interactive(&mut close, colors.btn_bg);
        close
            .observe(|_: On<Activate>, mut state: ResMut<EditorState>| {
                state.phrase_editor = None;
            })
            .with_children(|b| {
                b.spawn_empty().apply_scene(bsn! {
                    Text("\u{2717}")
                    TextFont { font_size: {FontSize::Px(13.0)} }
                    TextColor({colors.label})
                    ~{Pickable::IGNORE}
                });
            });
    });
    commands.entity(root).add_child(header);

    for (field, label_key) in FIELDS {
        let row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .id();
        commands.entity(row).with_children(|r| {
            r.spawn_empty().apply_scene(bsn! {
                Node {
                    width: {Val::Px(LABEL_W)},
                }
                Text({format!("{}:", loc.msg(label_key))})
                TextFont { font_size: {FontSize::Px(13.0)} }
                TextColor({colors.label})
                ~{Pickable::IGNORE}
            });
        });
        let input = spawn_text_input(
            &mut commands,
            row,
            "",
            INPUT_W,
            colors.field_bg,
            colors.accent.with_alpha(0.5),
            move |ev: On<TextInputCommitted>, mut state: ResMut<EditorState>| {
                if let Some(tick) = state.phrase_editor {
                    if field == Field::Lyric {
                        state.set_lyrics_from(tick, &ev.value);
                    } else {
                        state.set_annotation(tick, field, ev.value.clone());
                    }
                }
            },
        );
        commands.entity(input).insert(PhraseEditorBox(field));
        commands.entity(root).add_child(row);
    }
}

/// Where the popover's left edge goes for an onset at `tick`, in
/// `GridArea`'s own coordinates: directly under the marker, pulled back so
/// the panel never runs past the area's right edge, and never left of 0.
pub(super) fn popover_left(tick: usize, scroll_px: f32, area_width: f32) -> f32 {
    let x = tick as f32 * TICK_W - scroll_px;
    x.min((area_width - WIDTH).max(0.0)).max(0.0)
}

/// Shows, positions and fills the popover every frame it's open, and
/// closes it if its onset no longer has a note.
pub(super) fn update_phrase_editor(
    mut state: ResMut<EditorState>,
    scroll: Res<Scroll>,
    loc: Res<Localization>,
    focus: Res<InputFocus>,
    area: Query<&ComputedNode, With<GridArea>>,
    mut root: Query<&mut Node, With<PhraseEditor>>,
    mut title: Query<&mut Text, With<PhraseEditorTitle>>,
    mut boxes: Query<(Entity, &PhraseEditorBox, &mut EditableText)>,
    mut meter_cache: Local<Option<MeterClockCache>>,
) {
    let Ok(mut node) = root.single_mut() else {
        return;
    };
    // Closed, or open on an onset that has since lost its notes (undo,
    // delete, a Remove-range) — see the module docs.
    let tick = match state.phrase_editor {
        Some(tick) if state.notes.iter().any(|n| n.tick == tick) => tick,
        _ => {
            state.bypass_change_detection().phrase_editor = None;
            if node.display != Display::None {
                node.display = Display::None;
            }
            return;
        }
    };
    let area_width = area
        .single()
        .map(|c| c.size().x * c.inverse_scale_factor())
        .unwrap_or(WIDTH);
    if node.display != Display::Flex {
        node.display = Display::Flex;
    }
    let left = Val::Px(popover_left(tick, scroll.px, area_width));
    if node.left != left {
        node.left = left;
    }
    let top = Val::Px(ANNOTATION_TOP + ANNOTATION_H + GAP_BELOW_LANE);
    if node.top != top {
        node.top = top;
    }

    if let Ok(mut text) = title.single_mut() {
        let position = describe_tick(tick, MeterClockCache::map_for(&mut meter_cache, &state));
        let want = loc.msg_args("editor-phrase-editor-title", &[("position", position)]);
        if **text != *want {
            text.0.clear();
            text.0.push_str(&want);
        }
    }
    // Same rule as `panel::sync_meta_field_text`: never overwrite the box
    // the player is typing into.
    let focused = focus.get();
    for (entity, tag, mut text) in &mut boxes {
        if Some(entity) == focused {
            continue;
        }
        let want = state.annotation_text(tick, tag.0);
        if text.value() != want {
            text.editor_mut().set_text(want);
            text.queue_edit(TextEdit::TextEnd(false));
        }
    }
}
