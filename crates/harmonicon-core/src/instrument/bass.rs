// SPDX-License-Identifier: MIT
//! Fretted electric bass presets, highest string first. Pitch values are
//! sounding pitches; notation convention is handled by the score adapter.
//! Tuning reference: Yamaha, "Alternate Bass Tunings".
use super::fretted_strings::{FrettedInstrument, FrettedKind};
use super::guitar::{GuitarBody, Tuning};

pub const STANDARD_TUNING: [u8; 4] = [43, 38, 33, 28]; // G2 D2 A1 E1
pub const TUNINGS: &[Tuning] = &[
    Tuning { id: "bass-standard", strings: &STANDARD_TUNING },
    Tuning { id: "bass-five", strings: &[43, 38, 33, 28, 23] },
    Tuning { id: "bass-six", strings: &[48, 43, 38, 33, 28, 23] },
    Tuning { id: "bass-drop-d", strings: &[43, 38, 33, 26] },
];

impl FrettedInstrument {
    pub fn bass() -> Self {
        Self {
            kind: FrettedKind::Bass,
            strings: STANDARD_TUNING.to_vec(),
            frets: 24,
            capo: 0,
            body: GuitarBody::Electric,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::{ChartInstrument, HarpChart};
    use crate::instrument::guitar::{FretAssignment, StringTechnique};
    use crate::instrument::{
        Instrument, InstrumentKind, InstrumentModel, PlayPosition, TechniqueKind,
    };

    #[test]
    fn bass_identity_and_presets_keep_sounding_pitch_and_string_order() {
        let mut bass = FrettedInstrument::bass();
        assert_eq!(Instrument::from(bass.clone()).kind(), InstrumentKind::Bass);
        assert_eq!(bass.range(), Some((28, 67)));
        for preset in TUNINGS {
            bass.strings = preset.strings.to_vec();
            assert_eq!(bass.named_tuning().unwrap().id, preset.id);
            for (index, &midi) in preset.strings.iter().enumerate() {
                let at = FretAssignment {
                    string: (index + 1) as u8,
                    fret: 0,
                    technique: StringTechnique::Fretted,
                };
                assert_eq!(bass.pitch_of(&at), Some(midi));
                assert!(bass.positions_for(midi).contains(&PlayPosition::Fret(at)));
            }
            let spec = ChartInstrument::from_guitar(&bass);
            let encoded = serde_json::to_value(&spec).unwrap();
            assert_eq!(encoded["type"], "bass");
            assert!(encoded.get("body").is_none());
            assert_eq!(spec.to_instrument(), Some(bass.clone().into()));
        }
        bass.strings = TUNINGS[1].strings.to_vec();
        assert_eq!(bass.range().unwrap().0, 23); // B0, not B1
    }

    #[test]
    fn bass_drills_do_not_inherit_the_guitar_bend_curriculum() {
        let bass: Instrument = FrettedInstrument::bass().into();
        assert!(!bass.techniques().contains(&TechniqueKind::Glide));
        assert!(crate::note_trainer::drills(&bass, TechniqueKind::Glide).is_empty());
        let drills = crate::note_trainer::drills(&bass, TechniqueKind::Target);
        assert_eq!(drills.len(), 4 * 25);
        assert!(drills.iter().any(|drill| drill.target == 28.0));
        assert_eq!(FrettedInstrument::bass().hand_span_frets(), 3);
    }

    fn chart() -> HarpChart {
        serde_json::from_value(serde_json::json!({
            "metadata":{"format_version":"1.11.0"},
            "song":{"title":"Bass check","artist":"Harmonicon","genre":"Exercise","tempo_bpm":60,"key":"C","difficulty":"easy"},
            "timing":{"resolution":12,"tempo_map":[{"tick":0,"bpm":60}]},
            "instrument":{"type":"bass","strings":["G2","D2","A1","E1"],"frets":24},
            "track":[{"time":0,"duration":1,"events":[{"note":"E1","position":{"string":4,"fret":0}}]}],
            "scoring":{"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}
        })).unwrap()
    }

    #[test]
    fn chart_validation_and_codec_use_bass_positions_without_octave_transposition() {
        let mut chart = chart();
        assert_eq!(chart.instrument().unwrap().kind(), InstrumentKind::Bass);
        assert!(crate::chart_check::chart_problems(&chart).is_empty());
        let target = crate::note_event_codec::decode(&chart, &chart.track[0].events[0]).unwrap();
        assert_eq!(target.sounding_pitch(&chart.instrument().unwrap()), Some(28.0));
        chart.track[0].events[0].note = Some("E2".into());
        assert!(!crate::chart_check::chart_problems(&chart).is_empty());
    }

    #[test]
    fn retuning_preserves_pitch_but_guitar_is_not_a_bass_substitute() {
        let chart = chart();
        let mut dropped = FrettedInstrument::bass();
        dropped.strings = TUNINGS[3].strings.to_vec();
        let played: Instrument = dropped.into();
        let remapped = super::super::substitute::substitute_chart(&chart, &played);
        assert_eq!(played.pitch_of(&remapped[0][0].unwrap()), Some(28));
        assert_eq!(
            remapped[0][0],
            Some(PlayPosition::Fret(FretAssignment {
                string: 4,
                fret: 2,
                technique: StringTechnique::Fretted
            }))
        );
        let guitar = super::super::guitar::Guitar::standard(GuitarBody::Electric).into();
        assert_eq!(super::super::substitute::substitute_chart(&chart, &guitar), vec![vec![None]]);
    }
}
