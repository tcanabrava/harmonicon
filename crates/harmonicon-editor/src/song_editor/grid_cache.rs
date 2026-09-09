// SPDX-License-Identifier: MIT

//! Grid invalidation tracks rendered content, independently of editor metadata
//! and selection. Snapshots are copied only when a rebuild is required.
use super::snap::SnapMode;
use super::state::{EditorState, GridNote, HarmonicaKind, Mode};
use bevy::prelude::*;
use harmonicon_core::chart::Scale;

#[derive(Resource, Default)]
pub(super) struct GridCache {
    snapshot: Option<Snapshot>,
}

struct Snapshot {
    notes: Vec<GridNote>,
    tempo_changes: Vec<(usize, f32)>,
    tempo: String,
    key: String,
    time_signature: String,
    harmonica_kind: HarmonicaKind,
    mode: Mode,
    user_locked: bool,
    scale: Scale,
    snap_mode: SnapMode,
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
                    && old.tempo == state.tempo
                    && old.key == state.key
                    && old.time_signature == state.time_signature
                    && old.harmonica_kind == state.harmonica_kind
                    && old.mode == state.mode
                    && old.user_locked == state.user_locked
                    && old.scale == state.scale
                    && old.snap_mode == state.snap_mode
                    && old.scroll_beat == state.scroll_beat
                    && old.cols == cols
            })
        {
            return false;
        }
        self.snapshot = Some(Snapshot {
            notes: state.notes.clone(),
            tempo_changes: state.tempo_changes.clone(),
            tempo: state.tempo.clone(),
            key: state.key.clone(),
            time_signature: state.time_signature.clone(),
            harmonica_kind: state.harmonica_kind,
            mode: state.mode,
            user_locked: state.user_locked,
            scale: state.scale,
            snap_mode: state.snap_mode,
            scroll_beat: state.scroll_beat,
            cols,
        });
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
        let entity = app
            .world_mut()
            .spawn((NoteView(42), Node::default(), BorderColor::default()))
            .id();
        app.world_mut()
            .resource_mut::<EditorState>()
            .selected
            .push(42);
        app.update();
        assert_eq!(
            app.world().get::<Node>(entity).unwrap().border,
            UiRect::all(Val::Px(2.0))
        );
        app.world_mut()
            .resource_mut::<EditorState>()
            .selected
            .clear();
        app.update();
        assert_eq!(
            app.world().get::<Node>(entity).unwrap().border,
            UiRect::all(Val::Px(0.0))
        );
    }
}
