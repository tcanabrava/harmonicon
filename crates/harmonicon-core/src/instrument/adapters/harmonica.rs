// SPDX-License-Identifier: MIT
use super::*;

impl InstrumentAdapter for Harmonica {
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill> {
        crate::note_trainer::candidate_drills(
            self,
            kind,
            crate::note_trainer::harmonica_candidates(self, kind),
        )
    }
    fn max_voices(&self) -> usize {
        usize::from(self.hole_count())
    }
    fn breath_model(&self) -> Option<&Harmonica> {
        Some(self)
    }
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::Harmonica
    }
    fn synth_voice(&self) -> Voice {
        Voice::Harmonica
    }
}
