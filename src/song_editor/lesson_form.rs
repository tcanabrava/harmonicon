// SPDX-License-Identifier: MIT

//! The lesson-only metadata panel (shown while [`ContentKind::Lesson`] is
//! active) and `lesson.json` save/load — the `ContentKind::Lesson` sibling
//! of `harpchart.rs`'s plain-song save/load. A lesson's chart, if it has
//! one, is an ordinary `.harpchart` written alongside the manifest at
//! `song/chart.harpchart` (relative to it) via the exact same
//! `harpchart::serialize_harpchart`/`load_harpchart` a plain song uses —
//! nothing about note editing, playback, or practice differs between the
//! two `ContentKind`s.
//!
//! **Scope boundary**: `lesson.json` only stores Fluent *keys*
//! (`title_key`/`body_key`), never display text — this codebase's
//! localization convention (`CLAUDE.md`). This module can't write real
//! translations (it doesn't know pt-BR/es-ES text for whatever the author
//! typed), so it derives the keys from `Field::LessonId` and prints the
//! key/text pairs the author still needs to add to the locale files by
//! hand — the same manual step authoring any bundled lesson already
//! requires. A MIDI-imported backing track isn't carried over to a lesson
//! save either (`harpchart::handle_save_chosen`'s `save_midi_backing` step
//! only runs for `ContentKind::Song`) — author the chart as a song first if
//! it needs one, then switch to Lesson mode to add the curriculum fields.

use bevy::prelude::*;

use super::state::{ContentKind, EditorState, LESSON_FIELDS, Scroll};
use super::ui::LessonFormGroup;
use super::{LOAD_PURPOSE, SAVE_PURPOSE};
use crate::dialogs::file_dialog::FileChosen;
use crate::dialogs::tooltip::Tooltip;
use crate::lessons::{LessonManifest, PassCriteria, parse_lesson};
use crate::localization::LocalizationExt;
use crate::theme::SongEditorColors;
use bevy_fluent::prelude::Localization;

/// The lesson-only fields panel — one row per [`LESSON_FIELDS`] entry,
/// reusing `meta_form::spawn_field_row` (the exact same click-to-cycle/
/// type-to-edit machinery the song fields use). Hidden by default;
/// [`update_lesson_form_visibility`] shows it once `ContentKind::Lesson` is
/// active.
pub(super) fn spawn_lesson_form(root: &mut ChildSpawnerCommands, loc: &Localization, colors: SongEditorColors) {
    root.spawn((
        LessonFormGroup,
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(12.0)),
            display: Display::None,
            ..default()
        },
        Tooltip(String::from(loc.msg("editor-lesson-form-tooltip"))),
    ))
    .with_children(|col| {
        for &(field, label) in &LESSON_FIELDS {
            super::meta_form::spawn_field_row(col, loc, colors, field, label);
        }
    });
}

/// Shows the lesson fields panel only while [`ContentKind::Lesson`] is
/// active — mirrors `panel::update_mode_visibility`'s `Node::display`
/// approach (not `Visibility`, which would still reserve its layout space).
pub(super) fn update_lesson_form_visibility(
    state: Res<EditorState>,
    mut groups: Query<&mut Node, With<LessonFormGroup>>,
) {
    let visible = state.content_kind == ContentKind::Lesson;
    for mut node in &mut groups {
        node.display = if visible { Display::Flex } else { Display::None };
    }
}

// ── Serialisation ────────────────────────────────────────────────────────────

/// Builds a `lesson.json` document from the lesson fields — schema-shaped
/// per `assets/lesson_schema.dtd.json`, and validated against it (via
/// [`parse_lesson`]) before being handed back, printing any schema error as
/// a warning rather than silently writing an invalid manifest. Also prints
/// the Fluent key/text pairs (`title_key`/`body_key`) the author needs to
/// add to the locale files — see this module's doc comment for why that
/// can't be automated.
pub(super) fn serialize_lesson(state: &EditorState) -> String {
    use serde_json::json;

    let id = state.lesson_id.trim();
    let unit = state.lesson_unit.trim();
    if id.is_empty() || unit.is_empty() {
        println!(
            "Warning: lesson id/unit is empty — this lesson.json won't load in-game until both are filled in."
        );
    }
    let title_key = format!("lesson-{id}-title");
    let body_key = format!("lesson-{id}-body");

    let mut manifest = json!({
        "id": id,
        "unit": unit,
        "title_key": title_key,
        "body_key": body_key,
    });

    if !state.notes.is_empty() {
        manifest["chart"] = json!("song/chart.harpchart");
    }

    let prerequisites: Vec<&str> = state
        .lesson_prerequisites
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if !prerequisites.is_empty() {
        manifest["prerequisites"] = json!(prerequisites);
    }

    if state.lesson_pass_criteria != "none" {
        let threshold: f32 = state.lesson_threshold.trim().parse().unwrap_or(0.7);
        manifest["pass_criteria"] = if state.lesson_pass_criteria == "technique" {
            json!({
                "type": "technique",
                "technique": state.lesson_technique,
                "threshold": threshold,
            })
        } else {
            json!({ "type": state.lesson_pass_criteria, "threshold": threshold })
        };
    }

    if state.lesson_progression != "none" {
        manifest["progression"] = json!(state.lesson_progression);
    }

    let json_text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    if let Err(err) = parse_lesson(json_text.as_bytes()) {
        println!("Warning: this lesson.json doesn't pass its own schema yet: {err}");
    }

    let title_text = if state.name.is_empty() {
        "(no title entered)"
    } else {
        &state.name
    };
    let body_text = if state.lesson_explanation.is_empty() {
        "(no explanation entered)"
    } else {
        &state.lesson_explanation
    };
    println!(
        "Add these to assets/locales/<lang>/main/ui.ftl (all three shipped locales) so the \
         lesson shows real text in-game:\n  {title_key} = {title_text}\n  {body_key} = {body_text}"
    );

    json_text
}

/// Writes `path` as the lesson's `lesson.json`, and — if the editor
/// currently has any notes — also writes `song/chart.harpchart` next to it
/// (relative to `path`'s own directory) via the ordinary
/// `harpchart::serialize_harpchart`, matching every shipped lesson's own
/// `"chart": "song/chart.harpchart"` convention.
pub(super) fn save_lesson(path: &std::path::Path, state: &EditorState) {
    let json = serialize_lesson(state);
    match std::fs::write(path, json.as_bytes()) {
        Ok(()) => println!("Saved lesson: {}", path.display()),
        Err(e) => {
            println!("Save failed (write lesson.json): {e}");
            return;
        }
    }

    if state.notes.is_empty() {
        return;
    }
    let Some(parent) = path.parent() else {
        return;
    };
    let song_dir = parent.join("song");
    if let Err(e) = std::fs::create_dir_all(&song_dir) {
        println!("Save failed (mkdir {}): {e}", song_dir.display());
        return;
    }
    let chart_path = song_dir.join("chart.harpchart");
    let chart_json = super::harpchart::serialize_harpchart(state);
    match std::fs::write(&chart_path, chart_json.as_bytes()) {
        Ok(()) => println!("Saved lesson chart: {}", chart_path.display()),
        Err(e) => println!("Save failed (write {}): {e}", chart_path.display()),
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Populates the lesson fields from a parsed manifest — the `ContentKind::
/// Lesson` sibling of `harpchart::load_harpchart`. `title_key`/`body_key`
/// aren't round-tripped as raw text (they're keys, not values — see this
/// module's doc comment); `Field::Name`/`Field::LessonExplanation` are left
/// as whatever's already in the editor; the author re-enters them to
/// regenerate matching Fluent entries on the next save.
pub(super) fn populate_from_lesson_manifest(manifest: &LessonManifest, state: &mut EditorState) {
    state.lesson_id = manifest.id.clone();
    state.lesson_unit = manifest.unit.clone();
    state.lesson_prerequisites = manifest.prerequisites.join(", ");
    match &manifest.pass_criteria {
        None => state.lesson_pass_criteria = "none".into(),
        Some(PassCriteria::Accuracy { threshold }) => {
            state.lesson_pass_criteria = "accuracy".into();
            state.lesson_threshold = threshold.to_string();
        }
        Some(PassCriteria::Technique { technique, threshold }) => {
            state.lesson_pass_criteria = "technique".into();
            state.lesson_technique = technique.clone();
            state.lesson_threshold = threshold.to_string();
        }
        Some(PassCriteria::ScaleAdherence { threshold }) => {
            state.lesson_pass_criteria = "scale-adherence".into();
            state.lesson_threshold = threshold.to_string();
        }
        Some(PassCriteria::ChordToneAdherence { threshold }) => {
            state.lesson_pass_criteria = "chord-tone-adherence".into();
            state.lesson_threshold = threshold.to_string();
        }
        Some(PassCriteria::PhraseDiscipline { threshold }) => {
            state.lesson_pass_criteria = "phrase-discipline".into();
            state.lesson_threshold = threshold.to_string();
        }
    }
    state.lesson_progression = manifest.progression.clone().unwrap_or_else(|| "none".into());
}

/// Reads and schema-validates `path` as a `lesson.json`, populates the
/// lesson fields, and — if it declares a `chart` — loads that too (relative
/// to `path`'s own directory) through the ordinary `harpchart::
/// load_harpchart`. An instructional-only lesson (no `chart`) clears any
/// notes already in the editor instead of leaving stale ones from whatever
/// was open before.
fn load_lesson(path: &std::path::Path, state: &mut EditorState, scroll: &mut Scroll) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            println!("Load failed (read): {e}");
            return;
        }
    };
    let manifest = match parse_lesson(&bytes) {
        Ok(m) => m,
        Err(e) => {
            println!("Load failed (invalid lesson.json): {e}");
            return;
        }
    };

    match &manifest.chart {
        Some(chart_rel) => {
            let Some(parent) = path.parent() else {
                return;
            };
            let chart_path = parent.join(chart_rel);
            match std::fs::read_to_string(&chart_path) {
                Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(v) => super::harpchart::load_harpchart(&v, state, scroll),
                    Err(e) => println!("Load failed (parse {}): {e}", chart_path.display()),
                },
                Err(e) => println!("Load failed (read {}): {e}", chart_path.display()),
            }
        }
        None => {
            state.notes.clear();
            state.next_id = 0;
            state.selected = None;
        }
    }

    populate_from_lesson_manifest(&manifest, state);
    state.content_kind = ContentKind::Lesson;
    println!("Loaded lesson: {}", path.display());
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// The `ContentKind::Lesson` sibling of `harpchart::handle_save_chosen` —
/// each skips the other's `ContentKind`, so exactly one acts on a given
/// `FileChosen { purpose: SAVE_PURPOSE }` message.
pub(super) fn handle_save_lesson_chosen(
    mut chosen: MessageReader<FileChosen>,
    state: Res<EditorState>,
) {
    for ev in chosen.read() {
        if ev.purpose != SAVE_PURPOSE || state.content_kind != ContentKind::Lesson {
            continue;
        }
        if let Some(parent) = ev.path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            println!("Save failed (mkdir): {e}");
            continue;
        }
        save_lesson(&ev.path, &state);
    }
}

/// The `ContentKind::Lesson` sibling of `harpchart::handle_load_chosen`.
pub(super) fn handle_load_lesson_chosen(
    mut chosen: MessageReader<FileChosen>,
    mut state: ResMut<EditorState>,
    mut scroll: ResMut<Scroll>,
) {
    for ev in chosen.read() {
        if ev.purpose != LOAD_PURPOSE || state.content_kind != ContentKind::Lesson {
            continue;
        }
        load_lesson(&ev.path, &mut state, &mut scroll);
    }
}
