// SPDX-License-Identifier: MIT
use super::*;

impl InstrumentAdapter for bowed_strings::BowedStrings {
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill> {
        let mut beginner = self.clone();
        beginner.stopped_semitones = 7;
        let mut candidates = Vec::new();
        for note in beginner.range().map(|(low, high)| low..=high).into_iter().flatten() {
            let choices = beginner.positions_for(note);
            if kind == TechniqueKind::Target {
                candidates
                    .extend(choices.into_iter().map(|at| (PlayPosition::Fingerboard(at), None)));
            } else if kind == TechniqueKind::Position && choices.len() > 1 {
                candidates
                    .extend(choices.into_iter().map(|at| (PlayPosition::Fingerboard(at), None)));
            }
        }
        crate::note_trainer::candidate_drills(self, kind, candidates)
    }
    fn max_voices(&self) -> usize {
        2
    }
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::Violin
    }
    fn synth_voice(&self) -> Voice {
        Voice::BowedString
    }
}
