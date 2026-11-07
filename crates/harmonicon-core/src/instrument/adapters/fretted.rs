// SPDX-License-Identifier: MIT
use super::*;

impl InstrumentAdapter for FrettedInstrument {
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill> {
        crate::note_trainer::candidate_drills(
            self,
            kind,
            crate::note_trainer::guitar_candidates(self, kind),
        )
    }
    fn max_voices(&self) -> usize {
        self.strings.len()
    }
    fn kind(&self) -> InstrumentKind {
        self.kind.instrument_kind()
    }
    fn synth_voice(&self) -> Voice {
        Voice::PluckedString
    }
    fn notation(&self) -> NotationContext {
        NotationContext {
            transposition: match self.kind {
                fretted_strings::FrettedKind::Guitar | fretted_strings::FrettedKind::Bass => {
                    Transposition::OCTAVE_DOWN
                }
                fretted_strings::FrettedKind::Cavaquinho => Transposition::CONCERT,
            },
        }
    }
}
