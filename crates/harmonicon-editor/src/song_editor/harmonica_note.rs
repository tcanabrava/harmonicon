// SPDX-License-Identifier: MIT

//! Adapter for the existing harmonica grid and its editing tools.
//! The generic document stores `NoteTarget`; these helpers are used only
//! behind the editor's harmonica-only load gate. A new view supplies its
//! own projection rather than changing what `hole` means here.
use super::note_model::{Dir, GridNote, HarpTechnique};
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::pitch_map::HoleAssignment;

impl GridNote {
    pub(super) fn harmonica_target(
        hole: u8,
        dir: Dir,
        technique: HarpTechnique,
    ) -> harmonicon_core::note_target::NoteTarget {
        harmonicon_core::note_target::NoteTarget::Positioned(PlayPosition::Hole(HoleAssignment {
            hole,
            action: dir.action(),
            technique: technique.technique(),
        }))
    }

    /// Harmonica-view adapter. Other views read `target` directly and never
    /// reinterpret a string or a fingering as a hole.
    fn harp_tab(&self) -> HoleAssignment {
        match self.target.position() {
            Some(PlayPosition::Hole(tab)) => tab,
            _ => panic!("the harmonica view requires a resolved harmonica placement"),
        }
    }
    pub(super) fn hole(&self) -> u8 {
        self.harp_tab().hole
    }
    pub(super) fn dir(&self) -> Dir {
        Dir::from_action(self.harp_tab().action)
    }
    pub(super) fn technique(&self) -> HarpTechnique {
        HarpTechnique::from_technique(self.harp_tab().technique)
    }
    pub(super) fn position(&self) -> PlayPosition {
        PlayPosition::Hole(self.harp_tab())
    }
    pub(super) fn set_tab_position(&mut self, position: &PlayPosition) {
        self.target = harmonicon_core::note_target::NoteTarget::Positioned(*position);
    }
    /// Only the dev-only expected-notes layer moves a note by hole; the
    /// grid moves rows (`GridNote::set_row`).
    #[cfg(feature = "dev")]
    pub(super) fn set_hole(&mut self, hole: u8) {
        let mut tab = self.harp_tab();
        tab.hole = hole;
        self.set_tab_position(&PlayPosition::Hole(tab));
    }
    pub(super) fn set_dir(&mut self, dir: Dir) {
        let mut tab = self.harp_tab();
        tab.action = dir.action();
        self.set_tab_position(&PlayPosition::Hole(tab));
    }
    pub(super) fn set_technique(&mut self, technique: HarpTechnique) {
        let mut tab = self.harp_tab();
        tab.technique = technique.technique();
        self.set_tab_position(&PlayPosition::Hole(tab));
    }
    pub(super) fn bend(&self) -> f32 {
        match self.technique() {
            HarpTechnique::Bend(a) => a,
            _ => 0.0,
        }
    }
}
