// SPDX-License-Identifier: MIT
//! Family registration and presentation metadata. Physical models remain
//! independent of sibling families; callers depend on this capability boundary.
use super::*;
use crate::{
    notation::{NotationContext, Transposition},
    synth::Voice,
};

pub trait InstrumentAdapter: InstrumentModel {
    fn kind(&self) -> InstrumentKind;
    /// Detector voice limit, not a claim about ergonomic playability.
    fn max_voices(&self) -> usize;
    fn setup_override(&self) -> bool {
        true
    }
    /// Whether target selection should be taught directly on a staff.
    fn score_picker(&self) -> bool {
        false
    }
    fn practice_drills(&self, kind: TechniqueKind) -> Vec<crate::note_trainer::Drill>;
    fn breath_model(&self) -> Option<&Harmonica> {
        None
    }
    fn synth_voice(&self) -> Voice;
    fn notation(&self) -> NotationContext {
        NotationContext { transposition: Transposition::CONCERT }
    }
    fn fingering_chart(&self) -> Option<&dyn wind::FingeringChart> {
        None
    }
}

mod bowed;
mod fretted;
mod harmonica;
mod piano;
mod wind_family;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::ChartInstrument;
    use crate::note_trainer::drills;

    #[test]
    fn registered_families_keep_pitch_drills_and_wire_configuration_consistent() {
        let mut instruments = vec![
            Instrument::Harmonica(crate::harmonica::richter_harp("C")),
            Instrument::Piano(Piano::default()),
            Instrument::Flute(Flute::default()),
            Instrument::Clarinet(Clarinet::default()),
            Instrument::Bowed(bowed_strings::BowedKind::Violin.standard()),
        ];
        instruments.extend(
            fretted_strings::FrettedKind::ALL
                .into_iter()
                .map(|kind| Instrument::Fretted(kind.default_setup())),
        );
        for instrument in instruments {
            let adapter = instrument.adapter();
            let pitches = instrument.valid_pitches();
            assert_eq!(
                instrument.range(),
                Some((*pitches.iter().min().unwrap(), *pitches.iter().max().unwrap()))
            );
            for drill in drills(&instrument, TechniqueKind::Target) {
                assert!(pitches.contains(&(drill.target.round() as u8)), "{:?}", instrument.kind());
                assert_eq!(
                    crate::note_target::NoteTarget::Positioned(drill.position)
                        .sounding_pitch(&instrument),
                    Some(drill.target)
                );
            }
            assert!(!drills(&instrument, TechniqueKind::Target).is_empty());
            assert_eq!(adapter.kind(), instrument.kind());
            if let Some(spec) = ChartInstrument::from_instrument(&instrument) {
                assert_eq!(spec.to_instrument().unwrap(), instrument);
            } else {
                assert!(adapter.breath_model().is_some());
            }
        }
    }
}
