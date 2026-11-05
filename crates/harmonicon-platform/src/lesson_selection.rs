// SPDX-License-Identifier: MIT
//! Explicit lesson selection, shared by discovery and translation loading.
use bevy::prelude::*;
use harmonicon_core::instrument::InstrumentKind;

/// No curriculum (or its text) is read until an instrument has been chosen.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct SelectedLessonInstrument(pub Option<InstrumentKind>);

/// Namespaced curricula are preferred. Existing unscoped packs remain usable
/// as harmonica packs, preserving their lesson ids and progress records.
#[cfg(not(target_arch = "wasm32"))]
pub fn lesson_roots(root: &std::path::Path, kind: InstrumentKind) -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    let scoped = root.join(kind.slug());
    if scoped.is_dir() {
        roots.push(scoped);
    }
    if kind == InstrumentKind::Harmonica {
        roots.push(root.to_path_buf());
    }
    roots
}
