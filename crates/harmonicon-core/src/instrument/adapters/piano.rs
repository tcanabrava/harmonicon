// SPDX-License-Identifier: MIT
use super::*;

impl InstrumentAdapter for Piano {
    fn setup_override(&self) -> bool {
        false
    }
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill> {
        crate::note_trainer::candidate_drills(
            self,
            kind,
            crate::note_trainer::piano_candidates(self),
        )
    }
    fn max_voices(&self) -> usize {
        10
    }
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::Piano
    }
    fn synth_voice(&self) -> Voice {
        Voice::Piano
    }
}
