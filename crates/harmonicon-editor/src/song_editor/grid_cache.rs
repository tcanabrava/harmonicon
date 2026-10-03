// SPDX-License-Identifier: MIT

//! Grid invalidation tracks rendered content, independently of editor metadata
//! and selection. Snapshots are copied only when a rebuild is required.
use super::snap::SnapMode;
use super::state::{EditorState, GridNote, HarmonicaKind, LoadedHarmonica, Mode, PhraseAnnotation};
use bevy::prelude::*;
use harmonicon_core::chart::Scale;

#[derive(Resource, Default)]
pub(super) struct GridCache {
    snapshot: Option<Snapshot>,
}

struct Snapshot {
    notes: Vec<GridNote>,
    tempo_changes: Vec<(usize, f32)>,
    meter_changes: Vec<(usize, String)>,
    phrase_annotations: std::collections::BTreeMap<usize, PhraseAnnotation>,
    tempo: String,
    key: String,
    time_signature: String,
    pickup_beats: String,
    repeats: Vec<harmonicon_core::chart::Repeat>,
    harmonica_kind: HarmonicaKind,
    loaded_harmonica: Option<LoadedHarmonica>,
    mode: Mode,
    user_locked: bool,
    scale: Scale,
    snap_mode: SnapMode,
    twelve_bar_tint: bool,
    scroll_beat: usize,
    cols: usize,
}

impl GridCache {
    pub(super) fn invalidate(&mut self) {
        self.snapshot = None;
    }

    pub(super) fn update(
        &mut self,
        state: &EditorState,
        cols: usize,
        external_change: bool,
    ) -> bool {
        if !external_change
            && self.snapshot.as_ref().is_some_and(|old| {
                old.notes == state.notes
                    && old.tempo_changes == state.tempo_changes
                    && old.meter_changes == state.meter_changes
                    && old.phrase_annotations == state.phrase_annotations
                    && old.tempo == state.tempo
                    && old.key == state.key
                    && old.time_signature == state.time_signature
                    && old.pickup_beats == state.pickup_beats
                    && old.repeats == state.repeats
                    && old.harmonica_kind == state.harmonica_kind
                    && old.loaded_harmonica == state.loaded_harmonica
                    && old.mode == state.mode
                    && old.user_locked == state.user_locked
                    && old.scale == state.scale
                    && old.snap_mode == state.snap_mode
                    && old.twelve_bar_tint == state.twelve_bar_tint
                    && old.scroll_beat == state.scroll_beat
                    && old.cols == cols
            })
        {
            return false;
        }
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.notes.clone_from(&state.notes);
            snapshot.tempo_changes.clone_from(&state.tempo_changes);
            snapshot.meter_changes.clone_from(&state.meter_changes);
            snapshot.phrase_annotations.clone_from(&state.phrase_annotations);
            snapshot.tempo.clone_from(&state.tempo);
            snapshot.key.clone_from(&state.key);
            snapshot.time_signature.clone_from(&state.time_signature);
            snapshot.pickup_beats.clone_from(&state.pickup_beats);
            snapshot.repeats.clone_from(&state.repeats);
            snapshot.harmonica_kind = state.harmonica_kind;
            snapshot.loaded_harmonica.clone_from(&state.loaded_harmonica);
            snapshot.mode = state.mode;
            snapshot.user_locked = state.user_locked;
            snapshot.scale = state.scale;
            snapshot.snap_mode = state.snap_mode;
            snapshot.twelve_bar_tint = state.twelve_bar_tint;
            snapshot.scroll_beat = state.scroll_beat;
            snapshot.cols = cols;
        } else {
            self.snapshot = Some(Snapshot {
                notes: state.notes.clone(),
                tempo_changes: state.tempo_changes.clone(),
                meter_changes: state.meter_changes.clone(),
                phrase_annotations: state.phrase_annotations.clone(),
                tempo: state.tempo.clone(),
                key: state.key.clone(),
                time_signature: state.time_signature.clone(),
                pickup_beats: state.pickup_beats.clone(),
                repeats: state.repeats.clone(),
                harmonica_kind: state.harmonica_kind,
                loaded_harmonica: state.loaded_harmonica.clone(),
                mode: state.mode,
                user_locked: state.user_locked,
                scale: state.scale,
                snap_mode: state.snap_mode,
                twelve_bar_tint: state.twelve_bar_tint,
                scroll_beat: state.scroll_beat,
                cols,
            });
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_and_selection_do_not_rebuild_but_content_and_viewport_do() {
        let mut state = EditorState::default();
        let mut cache = GridCache::default();
        assert!(cache.update(&state, 10, false));
        state.name = "renamed".into();
        state.author = "author".into();
        state.selected.push(42);
        assert!(!cache.update(&state, 10, false));
        state.tempo_changes.push((12, 90.0));
        assert!(cache.update(&state, 10, false));
        state.scroll_beat = 4;
        assert!(cache.update(&state, 10, false));
        // Both grid *view* options change what's drawn (which gridlines and
        // counting syllables; whether lanes carry the 12-bar tint), so they
        // have to invalidate even though neither is chart content.
        state.snap_mode = state.snap_mode.next();
        assert!(cache.update(&state, 10, false));
        state.twelve_bar_tint = true;
        assert!(cache.update(&state, 10, false));
        assert!(!cache.update(&state, 10, false));
        assert!(cache.update(&state, 11, false));
        assert!(cache.update(&state, 11, true)); // theme/waveform
        assert!(!cache.update(&state, 11, false));
        cache.invalidate(); // reenter editor with persistent document
        assert!(cache.update(&state, 11, false));
    }
    #[test]
    fn selection_updates_existing_note_entities_in_place() {
        use super::super::{grid::update_selection, ui::NoteView};
        use harmonicon_platform::theme::LoadedTheme;
        let mut app = App::new();
        app.init_resource::<EditorState>()
            .init_resource::<LoadedTheme>()
            .add_systems(Update, update_selection);
        let entity =
            app.world_mut().spawn((NoteView(42), Node::default(), BorderColor::default())).id();
        app.world_mut().resource_mut::<EditorState>().selected.push(42);
        app.update();
        assert_eq!(app.world().get::<Node>(entity).unwrap().border, UiRect::all(Val::Px(2.0)));
        app.world_mut().resource_mut::<EditorState>().selected.clear();
        app.update();
        assert_eq!(app.world().get::<Node>(entity).unwrap().border, UiRect::all(Val::Px(0.0)));
    }
}
