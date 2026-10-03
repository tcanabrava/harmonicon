// SPDX-License-Identifier: MIT

//! Chart metadata fields, MIDI import controls, and hole labels.

use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_widgets::Button as WidgetButton;
use bevy::ui_widgets::{Activate, ValueChange};

use super::state::{
    ContentKind, DIFFICULTIES, EditorState, FIELDS, Field, HARP_KEYS, HarmonicaKind, LESSON_PATHS,
    LESSON_SCALES, LOOP_TYPES, PASS_CRITERIA_KINDS, POSITIONS, PROGRESSIONS, SONG_FEELS,
    TECHNIQUE_NAMES, cycle_next,
};
use super::ui::{
    ContentKindText, EditorRoot, HarmonicaKindText, HoleColumnContent, LegendColumn, MetaFieldBox,
    MetaFieldText, MidiTrackComboboxSlot, ScaleComboboxSlot, SnapModeText,
    TimeSignatureComboboxSlot,
};
use super::{HEADER_H, MIDI_PURPOSE, MUSIC_PURPOSE, ROW_H, SILENCE_ROW_H, grid_height};
use harmonicon_core::chart::Scale;
use harmonicon_platform::localization::{Localization, LocalizationExt, enum_label_key};
use harmonicon_platform::theme::SongEditorColors;
use harmonicon_ui::dialogs::button::make_interactive;
use harmonicon_ui::dialogs::checkbox::spawn_checkbox;
use harmonicon_ui::dialogs::combobox::{ComboboxSelect, ComboboxValue, spawn_combobox};
use harmonicon_ui::dialogs::file_dialog::{DialogId, DialogMode, OpenFileDialog};
use harmonicon_ui::dialogs::text_input::{
    TextInputCommitted, spawn_multiline_text_input, spawn_text_input,
};
use harmonicon_ui::dialogs::tooltip::Tooltip;
use harmonicon_ui::music_score::TIME_SIGNATURES;

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
    col.spawn(Node { width: Val::Percent(100.0), height: Val::Px(HEADER_H), ..default() });
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
            r.spawn_empty().apply_scene(bsn! {
                Text({format!("{hole:02}")})
                TextFont { font_size: {FontSize::Px(13.0)} }
                TextColor({colors.label})
            });
            r.spawn_empty().apply_scene(bsn! {
                Node {
                    width: {Val::Px(20.0)},
                    height: {Val::Px(20.0)},
                    border: {UiRect::all(Val::Px(1.5))},
                }
                BackgroundColor({colors.hole_box})
                ~{BorderColor::all(Color::srgb(0.45, 0.45, 0.55))}
            });
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
        r.spawn_empty().apply_scene(bsn! {
            Text({loc.msg("editor-silence-track-label").to_string()})
            TextFont { font_size: {FontSize::Px(11.0)} }
            TextColor({colors.label})
            ~{Pickable::IGNORE}
        });
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
) -> Entity {
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        flex_grow: 1.0,
        ..default()
    })
    .with_children(build)
    .id()
}

/// A labelled click-to-cycle button row: `<label>:  [ current value ]` —
/// the shared shape [`spawn_content_kind_row`]/[`spawn_harmonica_kind_row`]
/// both build. `marker` tags the value text so its own `update_*_text`
/// system can find it.
fn spawn_cycle_row<T: Component, B: Bundle, M: 'static>(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    label_key: &str,
    tooltip_key: Option<&str>,
    marker: T,
    button_marker: B,
    on_click: impl IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) -> Entity {
    spawn_form_line(col, loc, colors, label_key, |line| {
        let mut btn = line.spawn((
            WidgetButton,
            TabIndex(0),
            Node {
                width: Val::Px(240.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
            button_marker,
        ));
        if let Some(key) = tooltip_key {
            btn.insert(Tooltip(String::from(loc.msg(key))));
        }
        make_interactive(&mut btn, colors.field_bg);
        btn.observe(on_click).with_children(|b| {
            b.spawn_empty()
                .apply_scene(bsn! {
                    Text("")
                    TextFont { font_size: {FontSize::Px(14.0)} }
                    TextColor({Color::WHITE})
                    ~{Pickable::IGNORE}
                })
                .insert(marker);
        });
    })
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
        Some("editor-content-kind-toggle-tooltip"),
        ContentKindText,
        (),
        |_: On<Activate>, mut state: ResMut<EditorState>| {
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
        Some("editor-harmonica-toggle-tooltip"),
        HarmonicaKindText,
        (),
        |_: On<Activate>, mut state: ResMut<EditorState>| {
            let next = match state.harmonica_kind {
                HarmonicaKind::Diatonic => HarmonicaKind::PaddyRichter,
                HarmonicaKind::PaddyRichter => HarmonicaKind::CountryTuned,
                HarmonicaKind::CountryTuned => HarmonicaKind::NaturalMinor,
                HarmonicaKind::NaturalMinor => HarmonicaKind::Chromatic,
                HarmonicaKind::Chromatic => HarmonicaKind::Chromatic16,
                HarmonicaKind::Chromatic16 => HarmonicaKind::Diatonic,
            };
            state.set_harmonica_kind(next);
        },
    );
}

/// The Straight/Shuffle/Triplet grid-snap toggle — see [`SnapMode`]. Only
/// changes where the *next* click lands; existing notes are untouched.
fn spawn_snap_mode_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    spawn_cycle_row(
        col,
        loc,
        colors,
        "editor-field-snap-mode",
        Some("editor-snap-mode-toggle-tooltip"),
        SnapModeText,
        (),
        |_: On<Activate>, mut state: ResMut<EditorState>| {
            state.snap_mode = state.snap_mode.next();
        },
    );
}

/// The opt-in 12-bar-blues background tint — see
/// [`EditorState::twelve_bar_tint`]. A [`spawn_checkbox`] rather than
/// another [`spawn_cycle_row`]: it's a plain boolean, and the checkbox
/// widget carries its own keyboard handling. Spawned against `column` by
/// entity because `spawn_checkbox` takes a `Commands`, so it appends to the
/// column after everything the surrounding `with_children` closure built.
fn spawn_twelve_bar_tint_row(
    commands: &mut Commands,
    column: Entity,
    loc: &Localization,
    checked: bool,
) {
    spawn_checkbox(
        commands,
        column,
        &String::from(loc.msg("editor-field-twelve-bar-tint")),
        checked,
        |change: On<ValueChange<bool>>, mut state: ResMut<EditorState>| {
            state.twelve_bar_tint = change.value;
        },
    );
}

/// A form line: the `label_key` text in the label column, then whatever
/// `fill` spawns beside it. Returns the line.
fn spawn_form_line(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    label_key: &str,
    fill: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    col.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|line| {
        line.spawn_empty().apply_scene(bsn! {
            Node { width: {Val::Px(FORM_LABEL_W)} }
            Text({format!("{}:", loc.msg(label_key))})
            TextFont { font_size: {FontSize::Px(14.0)} }
            TextColor({colors.label})
        });
        fill(line);
    })
    .id()
}

/// Spawns one labelled field row and returns its own entity — so a caller
/// with a row whose relevance depends on another field's value (e.g.
/// `lesson_form`'s `LessonThreshold`/`LessonTechnique`) can tag it with a
/// marker component afterward for a visibility system to key on.
///
/// Two widget shapes, branching on [`cycle_field`]:
/// click-to-cycle fields (`Key`/`Position`, song enums, and lesson enums)
/// stay a plain `WidgetButton` that steps the value on `Activate`; every
/// other field gets a real text box (`dialogs::text_input::
/// spawn_text_input`, built on `bevy_text::EditableText`). A text box
/// doesn't re-render from `EditorState`; `panel::sync_meta_field_text` is
/// how a programmatic write — Load, MIDI import, Browse picking a file —
/// reaches a field the player isn't typing into.
pub(super) fn spawn_field_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    state: &EditorState,
    field: Field,
    label: &str,
) -> Entity {
    if let Some((choices, tooltip)) = cycle_field(field) {
        return spawn_cycle_row(
            col,
            loc,
            colors,
            label,
            tooltip,
            MetaFieldText(field),
            MetaFieldBox(field),
            move |_: On<Activate>, mut state: ResMut<EditorState>| {
                let value = cycle_next(choices, state.field_text(field));
                if field == Field::Key {
                    state.set_key(value);
                } else {
                    *state.field_text_mut(field) = value;
                }
            },
        );
    }
    spawn_form_line(col, loc, colors, label, |line| {
        let row_id = line.target_entity();
        let on_commit = move |ev: On<TextInputCommitted>, mut state: ResMut<EditorState>| {
            state.field_text_mut(field).clone_from(&ev.value);
        };
        let spawn_input =
            if field == Field::Description { spawn_multiline_text_input } else { spawn_text_input };
        let input_id = spawn_input(
            line.commands_mut(),
            row_id,
            state.field_text(field),
            240.0,
            colors.field_bg,
            Color::srgb(0.30, 0.30, 0.40),
            on_commit,
        );
        line.commands_mut().entity(input_id).insert((
            MetaFieldBox(field),
            Tooltip(String::from(loc.msg("editor-field-text-tooltip"))),
        ));

        if field == Field::Music {
            spawn_file_button(
                line,
                loc,
                "editor-browse",
                "editor-browse-tooltip",
                Color::srgb(0.18, 0.24, 0.36),
                MUSIC_PURPOSE,
                "dialog-select-music",
                &["ogg"],
            );
        }
    })
}

fn spawn_midi_track_row(
    col: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
) {
    spawn_form_line(col, loc, colors, "editor-field-midi-track", |line| {
        spawn_file_button(
            line,
            loc,
            "editor-import-midi",
            "editor-import-midi-tooltip",
            Color::srgb(0.24, 0.30, 0.20),
            MIDI_PURPOSE,
            "dialog-select-midi",
            &["mid", "midi"],
        );
        line.spawn((
            MidiTrackComboboxSlot,
            Node { flex_direction: FlexDirection::Column, ..default() },
        ));
    });
}

fn spawn_file_button(
    line: &mut ChildSpawnerCommands,
    loc: &Localization,
    label_key: &str,
    tooltip_key: &str,
    color: Color,
    purpose: DialogId,
    title_key: &'static str,
    extensions: &'static [&'static str],
) {
    let mut button = line.spawn((
        WidgetButton,
        TabIndex(0),
        Node {
            height: Val::Px(26.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
        Tooltip(String::from(loc.msg(tooltip_key))),
    ));
    make_interactive(&mut button, color);
    button
        .observe(
            move |_: On<Activate>,
                  loc: Res<Localization>,
                  mut open: MessageWriter<OpenFileDialog>| {
                open.write(OpenFileDialog {
                    purpose,
                    title: String::from(loc.msg(title_key)),
                    extensions: extensions.iter().map(|s| (*s).to_string()).collect(),
                    start_dir: dirs::home_dir(),
                    mode: DialogMode::Open,
                });
            },
        )
        .with_children(|b| {
            b.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg(label_key))})
                TextFont { font_size: {FontSize::Px(13.0)} }
                TextColor({Color::WHITE})
                ~{Pickable::IGNORE}
            });
        });
}

fn cycle_field(field: Field) -> Option<(&'static [&'static str], Option<&'static str>)> {
    let (choices, tooltip) = match field {
        Field::Key => (&HARP_KEYS[..], Some("editor-field-key-tooltip")),
        Field::Position => (&POSITIONS[..], Some("editor-field-position-tooltip")),
        Field::Difficulty => (&DIFFICULTIES[..], Some("editor-field-difficulty-tooltip")),
        Field::SongFeel => (&SONG_FEELS[..], Some("editor-field-feel-tooltip")),
        Field::ComboEnabled => (&["enabled", "disabled"][..], None),
        Field::LoopType => (&LOOP_TYPES[..], None),
        Field::LoopRepeat => (&["no", "yes"][..], None),
        Field::LessonPassCriteria => {
            (&PASS_CRITERIA_KINDS[..], Some("editor-field-lesson-pass-criteria-tooltip"))
        }
        Field::LessonTechnique => {
            (&TECHNIQUE_NAMES[..], Some("editor-field-lesson-technique-tooltip"))
        }
        Field::LessonProgression => {
            (&PROGRESSIONS[..], Some("editor-field-lesson-progression-tooltip"))
        }
        Field::LessonScale => (&LESSON_SCALES[..], Some("editor-field-lesson-scale-tooltip")),
        Field::LessonPath => (&LESSON_PATHS[..], Some("editor-field-lesson-path-tooltip")),
        _ => return None,
    };
    Some((choices, tooltip))
}

/// Fills in [`ScaleComboboxSlot`] the first time it's seen empty — a
/// spawn-once gate (`Without<Children>`) rather than a rebuild-on-message
/// system like [`rebuild_midi_track_combobox`](super::midi_import::
/// rebuild_midi_track_combobox), since [`Scale::all`]'s option list never
/// changes at runtime. Needs `EditorRoot` as the dropdown's backdrop parent,
/// which doesn't exist yet the frame `ui::spawn_fixed_chrome` spawns the
/// slot — hence deferring the spawn to this system instead of doing it
/// inline there.
///
/// The slot deliberately lives in the *fixed* chrome
/// (`ui::spawn_fixed_chrome`), not the scrollable meta form: `bevy_ui_
/// widgets::Popover` requires the dropdown list to be a literal ECS child
/// of its toggle, and Bevy's UI overflow clipping follows that same
/// ancestry, not the popover's computed screen position — a combobox
/// nested inside the form's `Overflow::scroll_y()` `ScrollArea` would get
/// its open dropdown clipped to that scrollable viewport no matter how
/// high its `GlobalZIndex` is, rendering behind the unclipped mod panel
/// and eating its clicks.
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
    // Shown in the player's language; the selection maps back by index
    // into the same `Scale::all()` order, so `label()` stays the chart's
    // stable id (see `localization::enum_label_key`).
    let options: Vec<String> = Scale::all().iter().map(|s| scale_label(*s, &loc)).collect();
    let combo = spawn_combobox(
        &mut commands,
        slot_entity,
        backdrop,
        &loc.msg("editor-field-scale"),
        &options,
        &scale_label(state.scale, &loc),
        on_scale_selected,
    );
    commands.entity(combo).insert(Tooltip(String::from(loc.msg("editor-field-scale-tooltip"))));
}

/// A scale's label in the player's language.
fn scale_label(scale: Scale, loc: &Localization) -> String {
    loc.msg(&enum_label_key("scale", scale.label())).into()
}

fn on_scale_selected(ev: On<ComboboxSelect>, mut state: ResMut<EditorState>) {
    if let Some(scale) = Scale::all().get(ev.index) {
        state.scale = *scale;
    }
}

/// Keeps the scale combobox's displayed value in step with
/// `EditorState::scale` after it changes from outside the widget itself —
/// namely, Load populating a different scale. Writing to [`ComboboxValue`]
/// directly is the widget's documented escape hatch for this; `dialogs::
/// combobox`'s `sync_combobox_visuals` then updates the visible toggle
/// label from it, same as a user pick would.
pub(super) fn sync_scale_combobox_value(
    state: Res<EditorState>,
    loc: Res<Localization>,
    slot: Query<&Children, With<ScaleComboboxSlot>>,
    mut values: Query<&mut ComboboxValue>,
) {
    let Ok(children) = slot.single() else {
        return;
    };
    for &child in children {
        if let Ok(mut value) = values.get_mut(child) {
            let want = scale_label(state.scale, &loc);
            if value.0 != want {
                value.0 = want;
            }
        }
    }
}

/// Fills in [`TimeSignatureComboboxSlot`] the first time it's seen empty —
/// the same spawn-once gate, fixed option list and fixed-chrome placement
/// [`spawn_scale_combobox`] documents in full.
///
/// A picker, not a text row in [`FIELDS`]: a time signature's lower
/// number names a note value, so only a power of two belongs there, and
/// typed text like `4/3` parses fine into a bar length that matches no
/// meter. Picking from `music_score::TIME_SIGNATURES` makes that
/// unrepresentable rather than merely detectable.
pub(super) fn spawn_time_signature_combobox(
    mut commands: Commands,
    state: Res<EditorState>,
    loc: Res<Localization>,
    slot: Query<Entity, (With<TimeSignatureComboboxSlot>, Without<Children>)>,
    editor_root: Query<Entity, With<EditorRoot>>,
) {
    let Ok(slot_entity) = slot.single() else {
        return;
    };
    let Ok(backdrop) = editor_root.single() else {
        return;
    };
    let options: Vec<String> = TIME_SIGNATURES.iter().map(|s| s.to_string()).collect();
    let combo = spawn_combobox(
        &mut commands,
        slot_entity,
        backdrop,
        &loc.msg("editor-field-time-signature"),
        &options,
        &state.time_signature,
        on_time_signature_selected,
    );
    commands
        .entity(combo)
        .insert(Tooltip(String::from(loc.msg("editor-field-time-signature-tooltip"))));
}

fn on_time_signature_selected(ev: On<ComboboxSelect>, mut state: ResMut<EditorState>) {
    state.time_signature.clone_from(&ev.value);
}

/// Keeps the meter combobox's displayed value in step with
/// `EditorState::time_signature` when something other than the widget
/// writes it — Load, or a MIDI import carrying the file's own meter.
///
/// A chart may legitimately hold a meter that isn't in `TIME_SIGNATURES`
/// (the list is the common ones, not every valid meter). Writing
/// [`ComboboxValue`] directly displays it faithfully rather than snapping
/// it to a nearby option; only *re-picking* is limited to the list.
pub(super) fn sync_time_signature_combobox_value(
    state: Res<EditorState>,
    slot: Query<&Children, With<TimeSignatureComboboxSlot>>,
    mut values: Query<&mut ComboboxValue>,
) {
    let Ok(children) = slot.single() else {
        return;
    };
    for &child in children {
        if let Ok(mut value) = values.get_mut(child)
            && value.0 != state.time_signature
        {
            value.0.clone_from(&state.time_signature);
        }
    }
}

/// The chart metadata form: two side-by-side field columns plus a third,
/// [`spawn_color_legend`], explaining what every color the grid/mod-panel/
/// scrollbar means. Split evenly (`FIELDS.len() / 2`): harmonica kind +
/// the first half of `FIELDS` in the left column, the second half + the
/// MIDI-track row in the middle — halves the form's height versus one
/// stacked column, which routinely ran taller than a default-sized window.
pub(super) fn spawn_meta_form(
    root: &mut ChildSpawnerCommands,
    loc: &Localization,
    colors: SongEditorColors,
    state: &EditorState,
    compact: bool,
    legend_visible: bool,
) {
    const MID: usize = FIELDS.len() / 2;
    root.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: if compact { FlexDirection::Column } else { FlexDirection::Row },
        column_gap: Val::Px(24.0),
        row_gap: Val::Px(24.0),
        padding: UiRect::all(Val::Px(12.0)),
        ..default()
    })
    .with_children(|form| {
        let first_col = spawn_form_column(form, |col| {
            spawn_content_kind_row(col, loc, colors);
            spawn_harmonica_kind_row(col, loc, colors);
            spawn_snap_mode_row(col, loc, colors);
            for &(field, label) in &FIELDS[..MID] {
                spawn_field_row(col, loc, colors, state, field, label);
            }
        });
        spawn_twelve_bar_tint_row(&mut form.commands(), first_col, loc, state.twelve_bar_tint);
        spawn_form_column(form, |col| {
            for &(field, label) in &FIELDS[MID..] {
                spawn_field_row(col, loc, colors, state, field, label);
            }
            spawn_midi_track_row(col, loc, colors);
        });
        let legend_col = spawn_form_column(form, |col| {
            super::legend::spawn_color_legend(col, loc, colors);
        });
        form.commands().entity(legend_col).insert((
            LegendColumn,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                flex_grow: 1.0,
                display: if legend_visible { Display::Flex } else { Display::None },
                ..default()
            },
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn form_buttons_preserve_field_updates_and_dialog_requests() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        let world = app.world_mut();
        world.insert_resource(Localization::default());
        world.init_resource::<Messages<OpenFileDialog>>();
        world.insert_resource(EditorState {
            loaded_harmonica: Some(super::super::state::LoadedHarmonica {
                key: "C".into(),
                kind: HarmonicaKind::Diatonic,
                harp: harmonicon_core::harmonica::richter_harp("C"),
            }),
            ..default()
        });
        world
            .run_system_once(
                |mut commands: Commands, state: Res<EditorState>, loc: Res<Localization>| {
                    commands.spawn_empty().with_children(|col| {
                        let colors = SongEditorColors::default();
                        for field in
                            [Field::Key, Field::ComboEnabled, Field::LessonPath, Field::Music]
                        {
                            spawn_field_row(col, &loc, colors, &state, field, "label");
                        }
                        spawn_midi_track_row(col, &loc, colors);
                    });
                },
            )
            .unwrap();
        let buttons: Vec<_> = world
            .query_filtered::<(Entity, Option<&MetaFieldBox>), With<WidgetButton>>()
            .iter(world)
            .map(|(entity, field)| (entity, field.map(|f| f.0)))
            .collect();
        assert_eq!(buttons.len(), 5);
        for (entity, field) in buttons {
            let before =
                field.map(|field| world.resource::<EditorState>().field_text(field).to_string());
            world.trigger(Activate { entity });
            if let Some(field) = field {
                assert_ne!(world.resource::<EditorState>().field_text(field), before.unwrap());
                if field == Field::Key {
                    assert!(world.resource::<EditorState>().loaded_harmonica.is_none());
                }
            }
        }
        let requests: Vec<_> = world.resource_mut::<Messages<OpenFileDialog>>().drain().collect();
        assert_eq!(requests.len(), 2);
        for (purpose, title, extensions) in [
            (MUSIC_PURPOSE, "dialog-select-music", vec!["ogg"]),
            (MIDI_PURPOSE, "dialog-select-midi", vec!["mid", "midi"]),
        ] {
            let request = requests.iter().find(|r| r.purpose == purpose).unwrap();
            assert_eq!(request.title, title);
            assert_eq!(request.extensions, extensions);
            assert!(matches!(request.mode, DialogMode::Open));
        }
    }
}
