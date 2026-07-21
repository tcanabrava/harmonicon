// SPDX-License-Identifier: MIT

//! The chart metadata form (harmonica kind/key/position/tempo/... fields,
//! the MIDI-track import row) and the hole-column spawning it's visually
//! paired with in the setup layout (both are per-hole/per-chart "reference"
//! panels, unlike the interactive note grid or mod panel).

use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::grid::{OUT_OF_SCALE_MIX, OUT_OF_SCALE_TINT, TEMPO_MARKER_COLOR, mix_srgba};
use super::interaction::{SCROLLBAR_BLOW_COLOR, SCROLLBAR_DRAW_COLOR};
use super::state::{
    ContentKind, Dir, EditorState, FIELDS, Field, HARP_KEYS, HarmonicaKind, PASS_CRITERIA_KINDS,
    POSITIONS, PROGRESSIONS, Pitch, TECHNIQUE_NAMES, cycle_next, pitch_color,
};
use super::timeline_overlay::{RANGE_HIGHLIGHT_COLOR, SPLIT_LINE_COLOR};
use super::ui::{
    ContentKindText, EditorRoot, HarmonicaKindText, HoleColumnContent, MetaFieldBox, MetaFieldText,
    MidiTrackComboboxSlot, ScaleComboboxSlot,
};
use super::{HEADER_H, MIDI_PURPOSE, MUSIC_PURPOSE, ROW_H, SILENCE_ROW_H, grid_height};
use crate::dialogs::combobox::{ComboboxSelect, ComboboxValue, spawn_combobox};
use crate::dialogs::file_dialog::{DialogMode, OpenFileDialog};
use crate::dialogs::tooltip::Tooltip;
use crate::localization::LocalizationExt;
use crate::song::chart::Scale;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

pub(super) fn spawn_hole_column(
    row: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
    hole_count: u8,
    loc: &Localization,
) {
    row.spawn((
        HoleColumnContent,
        Node {
            width: Val::Px(super::HOLE_COL_W),
            height: Val::Px(grid_height(hole_count)),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..default()
        },
    ))
    .with_children(|col| {
        spawn_hole_column_rows(col, colors, hole_count, loc);
    });
}

/// Respawns the hole column's contents (called from `ui::setup` initially,
/// and from `ui::sync_hole_column` whenever the harmonica's hole count
/// changes).
pub(super) fn spawn_hole_column_rows(
    col: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
    hole_count: u8,
    loc: &Localization,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Px(HEADER_H),
        ..default()
    });
    for hole in 1..=hole_count {
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
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
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
    // Label for the silence track's background strip (spawned in
    // `grid::rebuild_grid`) — keeps this column's total height matching
    // `grid_height` so the hole rows on the right stay aligned with it.
    col.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(SILENCE_ROW_H),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        Tooltip(String::from(loc.msg("editor-silence-track-tooltip"))),
    ))
    .with_children(|r| {
        r.spawn((
            Text::new(loc.msg("editor-silence-track-label").to_string()),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(colors.label),
            Pickable::IGNORE,
        ));
    });
}

/// Label width within a form column — fixed so a column's field boxes all
/// line up at the same x position, same reasoning the pre-two-column layout
/// used a fixed label width for.
const FORM_LABEL_W: f32 = 110.0;

/// A form column: one of the two side-by-side stacks [`spawn_meta_form`]
/// splits its 8 rows across — also reused by `lesson_form::spawn_lesson_form`
/// for the same reason (halving a long field list's height).
pub(super) fn spawn_form_column(
    root: &mut ChildSpawnerCommands,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        flex_grow: 1.0,
        ..default()
    })
    .with_children(build);
}

/// A labelled click-to-cycle button row: `<label>:  [ current value ]` — the
/// shared shape [`spawn_content_kind_row`]/[`spawn_harmonica_kind_row`] both
/// build (the Key/Position/lesson pass-criteria/technique/progression
/// cycle fields are a separate case — they already share their scaffold via
/// [`spawn_field_row`], just branching on which `Field` they are). `marker`
/// tags the value text so its own `update_*_text` system can find it.
fn spawn_cycle_row<T: Component, M: 'static>(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    label_key: &str,
    tooltip_key: &str,
    marker: T,
    on_click: impl IntoObserverSystem<Pointer<Click>, (), M> + Clone + Sync + 'static,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn((
            Node {
                width: Val::Px(FORM_LABEL_W),
                ..default()
            },
            Text::new(format!("{}:", loc.msg(label_key))),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(colors.label),
        ));
        line.spawn((
            Button,
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
            Tooltip(String::from(loc.msg(tooltip_key))),
        ))
        .observe(on_click)
        .with_children(|b| {
            b.spawn((
                marker,
                Text::new(String::new()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
    });
}

/// The "Record Song"/"Record Lesson" toggle — switching `content_kind` has
/// no side effects to reconcile (unlike harmonica kind, which must sanitize
/// existing notes), so the observer just flips it directly rather than
/// calling a dedicated `EditorState` method.
fn spawn_content_kind_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    spawn_cycle_row(
        col,
        loc,
        colors,
        "editor-field-content-kind",
        "editor-content-kind-toggle-tooltip",
        ContentKindText,
        |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            state.content_kind = match state.content_kind {
                ContentKind::Song => ContentKind::Lesson,
                ContentKind::Lesson => ContentKind::Song,
            };
        },
    );
}

fn spawn_harmonica_kind_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    spawn_cycle_row(
        col,
        loc,
        colors,
        "editor-field-harmonica",
        "editor-harmonica-toggle-tooltip",
        HarmonicaKindText,
        |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
            let next = match state.harmonica_kind {
                HarmonicaKind::Diatonic => HarmonicaKind::Chromatic,
                HarmonicaKind::Chromatic => HarmonicaKind::Diatonic,
            };
            state.set_harmonica_kind(next);
        },
    );
}

/// Spawns one labelled field row and returns its own entity — so a caller
/// with a row whose relevance depends on another field's value (e.g.
/// `lesson_form`'s `LessonThreshold`/`LessonTechnique`) can tag it with a
/// marker component afterward for a visibility system to key on.
pub(super) fn spawn_field_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    field: Field,
    label: &str,
) -> Entity {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn((
            Node {
                width: Val::Px(FORM_LABEL_W),
                ..default()
            },
            Text::new(format!("{}:", loc.msg(label))),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
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
            btn.insert(Tooltip(String::from(loc.msg("editor-field-key-tooltip"))))
                .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                    state.key = cycle_next(&HARP_KEYS, &state.key);
                });
        } else if field == Field::Position {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-position-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.position = cycle_next(&POSITIONS, &state.position);
            });
        } else if field == Field::LessonPassCriteria {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-lesson-pass-criteria-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.lesson_pass_criteria =
                    cycle_next(&PASS_CRITERIA_KINDS, &state.lesson_pass_criteria);
            });
        } else if field == Field::LessonTechnique {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-lesson-technique-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.lesson_technique = cycle_next(&TECHNIQUE_NAMES, &state.lesson_technique);
            });
        } else if field == Field::LessonProgression {
            btn.insert(Tooltip(String::from(
                loc.msg("editor-field-lesson-progression-tooltip"),
            )))
            .observe(|_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                state.lesson_progression = cycle_next(&PROGRESSIONS, &state.lesson_progression);
            });
        } else {
            btn.observe(
                move |_: On<Pointer<Click>>, mut state: ResMut<EditorState>| {
                    state.focus = Some(field);
                },
            );
        }

        btn.with_children(|b| {
            b.spawn((
                MetaFieldText(field),
                Text::new(String::new()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
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
                Tooltip(String::from(loc.msg("editor-browse-tooltip"))),
            ))
            .observe(
                |_: On<Pointer<Click>>,
                 loc: Res<Localization>,
                 mut open: MessageWriter<OpenFileDialog>| {
                    open.write(OpenFileDialog {
                        purpose: MUSIC_PURPOSE,
                        title: String::from(loc.msg("dialog-select-music")),
                        extensions: vec!["ogg".into()],
                        start_dir: dirs::home_dir(),
                        mode: DialogMode::Open,
                    });
                },
            )
            .with_children(|b| {
                b.spawn((
                    Text::new(String::from(loc.msg("editor-browse"))),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Pickable::IGNORE,
                ));
            });
        }
    })
    .id()
}

fn spawn_midi_track_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn((
            Node {
                width: Val::Px(FORM_LABEL_W),
                ..default()
            },
            Text::new(format!("{}:", loc.msg("editor-field-midi-track"))),
            TextFont {
                font_size: FontSize::Px(14.0),
                ..default()
            },
            TextColor(colors.label),
        ));
        line.spawn((
            Button,
            Node {
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.24, 0.30, 0.20)),
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
            Tooltip(String::from(loc.msg("editor-import-midi-tooltip"))),
        ))
        .observe(
            |_: On<Pointer<Click>>,
             loc: Res<Localization>,
             mut open: MessageWriter<OpenFileDialog>| {
                open.write(OpenFileDialog {
                    purpose: MIDI_PURPOSE,
                    title: String::from(loc.msg("dialog-select-midi")),
                    extensions: vec!["mid".into(), "midi".into()],
                    start_dir: dirs::home_dir(),
                    mode: DialogMode::Open,
                });
            },
        )
        .with_children(|b| {
            b.spawn((
                Text::new(String::from(loc.msg("editor-import-midi"))),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });
        line.spawn((
            MidiTrackComboboxSlot,
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ));
    });
}

/// Fills in [`ScaleComboboxSlot`] the first time it's seen empty — a
/// spawn-once gate (`Without<Children>`) rather than a rebuild-on-message
/// system like [`rebuild_midi_track_combobox`](super::midi_import::
/// rebuild_midi_track_combobox), since [`Scale::all`]'s option list never
/// changes at runtime; there's nothing to rebuild for. Needs `EditorRoot`
/// as the dropdown's backdrop parent (see `dialogs::combobox`'s own docs
/// for why a combobox needs one), which doesn't exist yet the frame
/// `ui::spawn_fixed_chrome` spawns the slot — hence deferring the actual
/// combobox spawn to this system instead of doing it inline there.
///
/// The slot itself deliberately lives in the *fixed* chrome
/// (`ui::spawn_fixed_chrome`), not the scrollable meta form alongside the
/// other fields: `bevy_ui_widgets::Popover` (what makes the dropdown list
/// float above everything, positioned relative to its toggle) requires the
/// list to be a literal ECS child of that toggle, and Bevy's UI overflow
/// clipping follows that same ancestry — not the popover's own computed
/// screen position. A combobox nested inside the form's `Overflow::
/// scroll_y()` `ScrollArea` gets its open dropdown clipped to that
/// scrollable viewport no matter how high its `GlobalZIndex` is, which is
/// exactly why it used to render behind the (unclipped) mod panel and eat
/// clicks meant for it. Living in the fixed chrome instead sidesteps the
/// clipping ancestor entirely, the only way to fix this given the widget's
/// design.
pub(super) fn spawn_scale_combobox(
    mut commands: Commands,
    state: Res<EditorState>,
    loc: Res<Localization>,
    slot: Query<Entity, (With<ScaleComboboxSlot>, Without<Children>)>,
    editor_root: Query<Entity, With<EditorRoot>>,
) {
    let Ok(slot_entity) = slot.single() else {
        return;
    };
    let Ok(backdrop) = editor_root.single() else {
        return;
    };
    let options: Vec<String> = Scale::all().iter().map(|s| s.label().to_string()).collect();
    spawn_combobox(
        &mut commands,
        slot_entity,
        backdrop,
        &loc.msg("editor-field-scale"),
        &options,
        state.scale.label(),
        on_scale_selected,
    );
}

fn on_scale_selected(ev: On<ComboboxSelect>, mut state: ResMut<EditorState>) {
    if let Some(scale) = Scale::from_label(&ev.value) {
        state.scale = scale;
    }
}

/// Keeps the scale combobox's displayed value in step with
/// `EditorState::scale` after it changes from outside the widget itself —
/// namely, Load populating a different scale than whatever was previously
/// selected. Writing to [`ComboboxValue`] directly is the widget's own
/// documented escape hatch for exactly this; `dialogs::combobox`'s own
/// `sync_combobox_visuals` (always running) then updates the visible
/// toggle label text from it, same as a user pick would.
pub(super) fn sync_scale_combobox_value(
    state: Res<EditorState>,
    slot: Query<&Children, With<ScaleComboboxSlot>>,
    mut values: Query<&mut ComboboxValue>,
) {
    let Ok(children) = slot.single() else {
        return;
    };
    for &child in children {
        if let Ok(mut value) = values.get_mut(child) {
            let want = state.scale.label();
            if value.0 != want {
                value.0 = want.to_string();
            }
        }
    }
}

/// The chart metadata form: two side-by-side field columns plus a third,
/// [`spawn_color_legend`], explaining what every color the grid/mod-panel/
/// scrollbar uses actually means — with 8 field rows (harmonica kind,
/// [`FIELDS`]'s 6, and the MIDI-track row), stacking them all in one column
/// routinely ran taller than a default-sized window. Split evenly
/// (`FIELDS.len() / 2`, so each field column gets at least 4 rows for
/// today's 8): harmonica kind + the first half of `FIELDS` in the left
/// column, the second half + the MIDI-track row in the middle — halving the
/// form's height for the same content.
pub(super) fn spawn_meta_form(
    root: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    const MID: usize = FIELDS.len() / 2;
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(24.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    })
    .with_children(|form| {
        spawn_form_column(form, |col| {
            spawn_content_kind_row(col, loc, colors);
            spawn_harmonica_kind_row(col, loc, colors);
            for &(field, label) in &FIELDS[..MID] {
                spawn_field_row(col, loc, colors, field, label);
            }
        });
        spawn_form_column(form, |col| {
            for &(field, label) in &FIELDS[MID..] {
                spawn_field_row(col, loc, colors, field, label);
            }
            spawn_midi_track_row(col, loc, colors);
        });
        spawn_form_column(form, |col| {
            spawn_color_legend(col, loc, colors);
        });
    });
}

// ── Color legend ──────────────────────────────────────────────────────────────

/// A legend swatch's fixed size — small enough to sit beside its label like
/// a bullet, big enough that the color itself (not just its position) reads
/// clearly.
const SWATCH_SIZE: f32 = 16.0;

/// One legend entry: a color swatch (a plain filled box, or — via
/// `border_only` — an unfilled box with just a colored border, for the
/// entries that are actually borders in the real UI, not fills) plus its
/// explanation.
fn spawn_legend_row(
    col: &mut ChildSpawnerCommands,
    colors: SongEditorColors,
    swatch: Color,
    border_only: bool,
    text: String,
) {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn((
            Node {
                width: Val::Px(SWATCH_SIZE),
                height: Val::Px(SWATCH_SIZE),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(if border_only { 2.0 } else { 1.0 })),
                ..default()
            },
            BackgroundColor(if border_only { Color::NONE } else { swatch }),
            BorderColor::all(if border_only {
                swatch
            } else {
                Color::srgb(0.30, 0.30, 0.40)
            }),
        ));
        line.spawn((
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(12.5),
                ..default()
            },
            TextColor(colors.label),
        ));
    });
}

/// A small section heading within the legend column — same accent color
/// the rest of the editor uses for anything meant to draw the eye.
fn spawn_legend_heading(col: &mut ChildSpawnerCommands, colors: SongEditorColors, text: String) {
    col.spawn((
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(colors.accent),
        Node {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
    ));
}

/// Explains every color the song editor uses, grouped by where it shows up
/// — written for exactly the confusion a first-time user hits: the grid
/// note's *fill* color is its playing technique (blow vs. draw is instead
/// the small ↑/↓ arrow drawn on the note, not a color at all), while the
/// horizontal scrollbar's minimap markers use a *different* blue/orange
/// pair that means blow/draw specifically — the same blue means two
/// different things in two different places, which is exactly the kind of
/// thing worth spelling out rather than leaving the player to reverse
/// -engineer from `theme.json`.
fn spawn_color_legend(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    spawn_legend_heading(col, colors, loc.msg("editor-legend-notes").to_string());
    spawn_legend_row(
        col,
        colors,
        pitch_color(Pitch::Normal),
        false,
        loc.msg("editor-legend-normal").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        pitch_color(Pitch::Bend(1.0)),
        false,
        loc.msg("editor-legend-bend").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        pitch_color(Pitch::Overblow),
        false,
        loc.msg("editor-legend-overblow").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        pitch_color(Pitch::Overdraw),
        false,
        loc.msg("editor-legend-overdraw").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        pitch_color(Pitch::Slide),
        false,
        loc.msg("editor-legend-slide").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        mix_srgba(
            pitch_color(Pitch::Normal),
            OUT_OF_SCALE_TINT,
            OUT_OF_SCALE_MIX,
        ),
        false,
        loc.msg("editor-legend-out-of-scale").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        colors.accent,
        true,
        loc.msg("editor-legend-selected").to_string(),
    );
    col.spawn((
        Text::new(format!(
            "{}  {} / {}  {}",
            Dir::Blow.arrow(),
            loc.msg("editor-legend-blow"),
            loc.msg("editor-legend-draw"),
            Dir::Draw.arrow(),
        )),
        TextFont {
            font_size: FontSize::Px(12.5),
            ..default()
        },
        TextColor(colors.label),
    ));

    spawn_legend_heading(col, colors, loc.msg("editor-legend-dragging").to_string());
    spawn_legend_row(
        col,
        colors,
        colors.ghost_ok.with_alpha(0.30),
        false,
        loc.msg("editor-legend-drag-ok").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        colors.ghost_bad.with_alpha(0.30),
        false,
        loc.msg("editor-legend-drag-bad").to_string(),
    );

    spawn_legend_heading(col, colors, loc.msg("editor-legend-elsewhere").to_string());
    spawn_legend_row(
        col,
        colors,
        TEMPO_MARKER_COLOR,
        false,
        loc.msg("editor-legend-tempo-marker").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        SPLIT_LINE_COLOR,
        false,
        loc.msg("editor-legend-split-point").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        RANGE_HIGHLIGHT_COLOR,
        false,
        loc.msg("editor-legend-range-preview").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        colors.btn_active,
        false,
        loc.msg("editor-legend-active-button").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        SCROLLBAR_BLOW_COLOR,
        false,
        loc.msg("editor-legend-scrollbar-blow").to_string(),
    );
    spawn_legend_row(
        col,
        colors,
        SCROLLBAR_DRAW_COLOR,
        false,
        loc.msg("editor-legend-scrollbar-draw").to_string(),
    );
    col.spawn((
        Text::new(loc.msg("editor-legend-scrollbar-note").to_string()),
        TextFont {
            font_size: FontSize::Px(11.5),
            ..default()
        },
        TextColor(colors.label.with_alpha(0.75)),
    ));
}
