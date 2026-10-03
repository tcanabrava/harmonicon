// SPDX-License-Identifier: MIT

//! Selected-note phrase labels and expression depth editing.

use super::state::{DEFAULT_INTENSITY, EditorState, Expr, Field};

impl EditorState {
    /// The phrase at `tick`'s section/chord/groove/lyric text, or `""`.
    pub(super) fn annotation_text(&self, tick: usize, field: Field) -> &str {
        let Some(annotation) = self.phrase_annotations.get(&tick) else {
            return "";
        };
        match field {
            Field::Section => annotation.section.as_deref().unwrap_or(""),
            Field::Chord => annotation.chord.as_deref().unwrap_or(""),
            Field::Groove => annotation.groove.as_deref().unwrap_or(""),
            Field::Lyric => annotation.lyric.as_deref().unwrap_or(""),
            _ => unreachable!(),
        }
    }

    /// Sets one text field of the phrase at `tick` — an empty value clears
    /// it, and an annotation left with nothing set is dropped. Refuses a
    /// tick no note starts on: such an annotation is exactly the orphan
    /// `metadata_sync::drop_orphaned_metadata` exists to remove, so
    /// accepting it would only lose the text at the next prune.
    pub(super) fn set_annotation(&mut self, tick: usize, field: Field, value: String) {
        if !matches!(field, Field::Section | Field::Chord | Field::Groove | Field::Lyric) {
            return;
        }
        if !self.notes.iter().any(|n| n.tick == tick) {
            return;
        }
        let value = (!value.trim().is_empty()).then_some(value);
        let annotation = self.phrase_annotations.entry(tick).or_default();
        match field {
            Field::Section => annotation.section = value,
            Field::Chord => annotation.chord = value,
            Field::Groove => annotation.groove = value,
            Field::Lyric => annotation.lyric = value,
            _ => unreachable!(),
        }
        self.remove_empty_annotation(tick);
    }

    /// The phrase editor's Lyric box. One syllable sets this onset's lyric,
    /// like any other field; several, separated by spaces, are a line typed
    /// in one go and land one per onset from `tick` on, with `_` leaving an
    /// onset without one (a note held across the syllable before). Onsets
    /// beyond the last syllable are left alone.
    pub(super) fn set_lyrics_from(&mut self, tick: usize, text: &str) {
        let syllables: Vec<&str> = text.split_whitespace().collect();
        if syllables.len() <= 1 {
            self.set_annotation(tick, Field::Lyric, text.trim().to_string());
            return;
        }
        let onsets: std::collections::BTreeSet<usize> =
            self.notes.iter().map(|n| n.tick).filter(|&t| t >= tick).collect();
        for (onset, syllable) in onsets.into_iter().zip(syllables) {
            let value = if syllable == "_" { String::new() } else { syllable.to_string() };
            self.set_annotation(onset, Field::Lyric, value);
        }
    }

    pub(super) fn selected_annotation_text(&self, field: Field) -> &str {
        match self.selected_note().map(|note| note.tick) {
            Some(tick) => self.annotation_text(tick, field),
            None => "",
        }
    }

    pub(super) fn selected_expression_intensity(&self) -> &str {
        let Some(note) = self.selected_note() else {
            return "";
        };
        if note.expr == Expr::None {
            return "";
        }
        self.expression_intensities.get(&note.id).map(String::as_str).unwrap_or("0.5")
    }

    /// The depth the Depth button shows and steps from: the selected
    /// note's, or with nothing selected the sticky one — and `""` for a
    /// selected note that has no vibrato/wah to have a depth of.
    pub(super) fn depth_for_button(&self) -> &str {
        match self.selected_note() {
            Some(note) if note.expr == Expr::None => "",
            Some(_) => self.selected_expression_intensity(),
            None => &self.sticky_intensity,
        }
    }

    /// The Depth button's click: steps the selected notes' depth, or with
    /// nothing selected the sticky depth a new note gets. A primary note
    /// with no vibrato/wah makes it do nothing — the same "silently do
    /// nothing on an incompatible note" rule Overblow follows on a hole that
    /// can't.
    pub(super) fn cycle_depth(&mut self) {
        match self.selected_note() {
            Some(note) if note.expr == Expr::None => {}
            Some(_) => {
                // The primary note's depth decides the step, as with the
                // other technique buttons; every selected note with an
                // expression takes it, and one without is skipped.
                let next = next_depth_step(self.selected_expression_intensity());
                let mut skipped = 0;
                for id in self.selected.clone() {
                    if !self.set_expression_intensity(id, &next) {
                        skipped += 1;
                    }
                }
                if skipped > 0 && self.selected.len() > 1 {
                    self.technique_notice = Some(skipped);
                }
            }
            None => self.sticky_intensity = next_depth_step(&self.sticky_intensity),
        }
    }

    #[cfg(test)]
    pub(super) fn set_selected_expression_intensity(&mut self, value: String) {
        if let Some(id) = self.selected.last().copied() {
            self.set_expression_intensity(id, &value);
        }
    }

    /// Sets note `id`'s vibrato/wah depth; `false` for a note with neither,
    /// which has no depth to set. `0.5` is the default and is never stored.
    fn set_expression_intensity(&mut self, id: u32, value: &str) -> bool {
        if self.note_by_id(id).is_none_or(|note| note.expr == Expr::None) {
            return false;
        }
        let value = value.trim();
        if value.is_empty() || value == "0.5" {
            self.expression_intensities.remove(&id);
        } else if value.parse::<f32>().is_ok_and(|v| (0.0..=1.0).contains(&v)) {
            self.expression_intensities.insert(id, value.to_owned());
        }
        true
    }

    pub(super) fn selected_call(&self) -> bool {
        self.selected_note()
            .and_then(|n| self.phrase_annotations.get(&n.tick))
            .is_some_and(|annotation| annotation.call)
    }

    pub(super) fn set_selected_call(&mut self, call: bool) {
        let Some(tick) = self.selected_note().map(|note| note.tick) else {
            return;
        };
        if call {
            self.phrase_annotations.entry(tick).or_default().call = true;
        } else if let Some(annotation) = self.phrase_annotations.get_mut(&tick) {
            annotation.call = false;
            self.remove_empty_annotation(tick);
        }
    }

    pub(super) fn selected_split(&self) -> bool {
        self.selected_note()
            .and_then(|n| self.phrase_annotations.get(&n.tick))
            .is_some_and(|annotation| annotation.split)
    }

    pub(super) fn set_selected_split(&mut self, split: bool) {
        let Some(tick) = self.selected_note().map(|note| note.tick) else {
            return;
        };
        if split {
            self.phrase_annotations.entry(tick).or_default().split = true;
        } else if let Some(annotation) = self.phrase_annotations.get_mut(&tick) {
            annotation.split = false;
            self.remove_empty_annotation(tick);
        }
    }

    fn remove_empty_annotation(&mut self, tick: usize) {
        if self.phrase_annotations.get(&tick).is_some_and(|annotation| {
            annotation.section.is_none()
                && annotation.chord.is_none()
                && annotation.groove.is_none()
                && annotation.lyric.is_none()
                && !annotation.call
                && !annotation.split
        }) {
            self.phrase_annotations.remove(&tick);
        }
    }
}

/// The depth after one Depth-button click from `current`: the next of the
/// four quarter steps strictly above it, wrapping from 1 back to ¼. A
/// value between steps (a chart can carry any 0–1 depth) rounds *up* to the
/// next step rather than snapping to the nearest, so a click always
/// visibly changes something. Unparseable input counts as the default.
pub(super) fn next_depth_step(current: &str) -> String {
    const STEPS: [f32; 4] = [0.25, 0.5, 0.75, 1.0];
    let current: f32 =
        current.trim().parse().unwrap_or_else(|_| DEFAULT_INTENSITY.parse().unwrap());
    let next = STEPS.iter().copied().find(|&step| step > current + 1e-6).unwrap_or(STEPS[0]);
    if (next - 0.5).abs() < 1e-6 { DEFAULT_INTENSITY.to_string() } else { format!("{next}") }
}

/// `"0.75"` as `"75%"` — the Depth button's label. A value the map stores
/// as `""` (a note with no vibrato/wah) shows nothing.
pub(super) fn depth_label(value: &str) -> String {
    match value.trim().parse::<f32>() {
        Ok(v) => format!("{}%", (v * 100.0).round() as i32),
        Err(_) => String::new(),
    }
}
