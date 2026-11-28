// SPDX-License-Identifier: MIT
use super::*;
use voice::SingingVoice;

impl InstrumentAdapter for SingingVoice {
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill> {
        let candidates = match kind {
            TechniqueKind::Target => {
                (self.lowest()..=self.highest()).map(|m| (PlayPosition::Sung(m), None)).collect()
            }
            TechniqueKind::Glide => self
                .slides()
                .into_iter()
                .map(|(start, target)| {
                    (PlayPosition::Sung(target), Some(PlayPosition::Sung(start)))
                })
                .collect(),
            TechniqueKind::Register | TechniqueKind::Position => Vec::new(),
        };
        crate::note_trainer::candidate_drills(self, kind, candidates)
    }
    /// A sung note is taught on the staff it is read from.
    fn score_picker(&self) -> bool {
        true
    }
    fn max_voices(&self) -> usize {
        1
    }
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::Voice
    }
    fn synth_voice(&self) -> Voice {
        Voice::Sung
    }
}
