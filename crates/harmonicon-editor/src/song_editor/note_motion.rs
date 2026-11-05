// SPDX-License-Identifier: MIT
//! Transient feedback follows logical note IDs, even when the grid rebuilds.
use super::{state::EditorState, ui::NoteView};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub(super) struct Request {
    pub id: u32,
    pub rows: f32,
    pub rejected: bool,
}
pub(super) struct Motion {
    request: Request,
    elapsed: f32,
}

fn offset(request: Request, progress: f32, row_height: f32) -> f32 {
    if request.rejected {
        request.rows * 7.0 * (std::f32::consts::PI * progress).sin() * (1.0 - progress)
    } else {
        // Cubic ease-out settles softly into the destination row.
        request.rows * row_height * (1.0 - progress).powi(3)
    }
}

pub(super) fn animate(
    mut state: ResMut<EditorState>,
    time: Res<Time>,
    mut active: Local<HashMap<u32, Motion>>,
    mut views: Query<(&NoteView, &mut Node)>,
) {
    if !state.note_motion.is_empty() {
        for request in std::mem::take(&mut state.note_motion) {
            active.insert(request.id, Motion { request, elapsed: 0.0 });
        }
    }
    for (view, mut node) in &mut views {
        let Some(motion) = active.get_mut(&view.0) else { continue };
        let Some(note) = state.note_by_id(view.0) else { continue };
        let progress = (motion.elapsed / 0.24).min(1.0);
        let (_, top, _, _) =
            super::state::note_rect(note, state.display_row(note), state.display_rows());
        node.top =
            Val::Px(top + offset(motion.request, progress, super::row_h(state.display_rows())));
    }
    for motion in active.values_mut() {
        motion.elapsed += time.delta_secs();
    }
    active.retain(|id, motion| {
        motion.elapsed <= 0.24 + time.delta_secs() && state.note_by_id(*id).is_some()
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn movement_starts_at_old_row_and_settles_at_new_row() {
        let request = Request { id: 1, rows: -1.0, rejected: false };
        assert_eq!(offset(request, 0.0, 30.0), -30.0);
        assert_eq!(offset(request, 1.0, 30.0), 0.0);
        assert!(offset(request, 0.5, 30.0).abs() < 15.0);
    }
    #[test]
    fn rejected_move_returns_to_its_original_position() {
        let request = Request { id: 1, rows: 1.0, rejected: true };
        assert_eq!(offset(request, 0.0, 30.0), 0.0);
        assert!(offset(request, 0.4, 30.0) > 0.0);
        assert_eq!(offset(request, 1.0, 30.0), 0.0);
    }
}
