// SPDX-License-Identifier: MIT
use super::*;

impl InstrumentAdapter for Flute {
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill> {
        crate::note_trainer::wind_drills(self, kind)
    }
    fn max_voices(&self) -> usize {
        1
    }
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::Flute
    }
    fn synth_voice(&self) -> Voice {
        Voice::Flute
    }
    fn fingering_chart(&self) -> Option<&dyn wind::FingeringChart> {
        Some(self)
    }
}
impl InstrumentAdapter for Clarinet {
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill> {
        crate::note_trainer::wind_drills(self, kind)
    }
    fn max_voices(&self) -> usize {
        1
    }
    fn kind(&self) -> InstrumentKind {
        InstrumentKind::Clarinet
    }
    fn synth_voice(&self) -> Voice {
        Voice::Clarinet
    }
    fn notation(&self) -> NotationContext {
        Clarinet::notation(*self)
    }
    fn fingering_chart(&self) -> Option<&dyn wind::FingeringChart> {
        Some(self)
    }
}
