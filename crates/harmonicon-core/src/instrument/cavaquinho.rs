// SPDX-License-Identifier: MIT
//! Brazilian cavaquinho presets, with sounding pitches and string 1 first.
//! References: UFERSA (2020), Cláudia Soares Feitosa Barbosa, table 6
//! https://mnpef.ufersa.edu.br/wp-content/uploads/sites/94/2021/03/Disserta%C3%A7%C3%A3o-Final.pdf
//! and Musixe's Cavaquinho presentation (traditional/natural/violin tunings).
use super::fretted_strings::{FrettedInstrument, FrettedKind};
use super::guitar::{GuitarBody, Tuning};

pub const BRAZILIAN_TUNING: [u8; 4] = [74, 71, 67, 62]; // D5 B4 G4 D4
pub const TUNINGS: &[Tuning] = &[
    Tuning { id: "cavaquinho-brazilian", strings: &BRAZILIAN_TUNING },
    Tuning { id: "cavaquinho-natural", strings: &[76, 71, 67, 62] },
    Tuning { id: "cavaquinho-mandolin", strings: &[76, 69, 62, 55] },
];
impl FrettedInstrument {
    pub fn cavaquinho() -> Self {
        Self {
            kind: FrettedKind::Cavaquinho,
            strings: BRAZILIAN_TUNING.to_vec(),
            frets: 19,
            capo: 0,
            body: GuitarBody::Acoustic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::ChartInstrument;
    use crate::instrument::{Instrument, InstrumentKind, InstrumentModel, TechniqueKind};
    #[test]
    fn traditional_tuning_has_the_verified_sounding_octaves() {
        let cavaquinho = FrettedInstrument::cavaquinho();
        assert_eq!(cavaquinho.range(), Some((62, 93)));
        let instrument: Instrument = cavaquinho.clone().into();
        assert_eq!(instrument.kind(), InstrumentKind::Cavaquinho);
        assert_eq!(cavaquinho.open_pitch(4), Some(62));
        assert_eq!(cavaquinho.open_pitch(1), Some(74));
        assert_eq!(instrument.techniques(), &[TechniqueKind::Target, TechniqueKind::Position]);
        for preset in TUNINGS {
            let mut setup = cavaquinho.clone();
            setup.strings = preset.strings.to_vec();
            let spec = ChartInstrument::from_fretted(&setup);
            assert_eq!(serde_json::to_value(&spec).unwrap()["type"], "cavaquinho");
            assert_eq!(spec.to_instrument(), Some(setup.into()));
        }
    }
}
