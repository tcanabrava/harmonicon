// SPDX-License-Identifier: MIT

//! A chart played on the instrument the player holds rather than the one it
//! was written for: a guitar in another tuning, a flute with the other foot
//! joint. The harmonica's equivalent is `harp_remap`, which also has to
//! choose what to preserve; here the pitch is always kept, because a
//! position is guidance and the judge listens for pitch.
//!
//! - **Flute**: a note keeps its charted fingering where the player's flute
//!   has it, its standard one otherwise; a note below the player's foot
//!   joint (a B3 on a C foot) has no place at all.
//! - **Guitar**: each track item's pitches are fingered afresh for the
//!   player's tuning by the part's own shortest path
//!   (`guitar::fingering`), so the hand moves as little as it can across
//!   the whole part. A bend keeps its depth, sounding the same pitch from
//!   the fret below; a harmonic depends on its open string's pitch, so on
//!   a retuned string it becomes the plain fretted note it sounded.

use super::flute::{FingeringAssignment, Flute};
use super::guitar::fingering::{Onset, fingering};
use super::guitar::{FretAssignment, Guitar, StringTechnique};
use super::{Instrument, PlayPosition};
use crate::chart::HarpChart;

/// Every event of `chart` placed on `played`, by track item and then event,
/// in the chart's own order. `None` for an event the played instrument
/// can't sound, or that doesn't decode (which validation already refuses).
/// Events are left where they were when `played` is the chart's own
/// instrument.
pub fn substitute_chart(chart: &HarpChart, played: &Instrument) -> Vec<Vec<Option<PlayPosition>>> {
    let decoded: Vec<Vec<Option<PlayPosition>>> = chart
        .track
        .iter()
        .map(|item| {
            item.events
                .iter()
                .map(|event| crate::note_event_codec::decode(chart, event).ok()?.position())
                .collect()
        })
        .collect();
    let Some(written) = chart.instrument() else { return decoded };
    if &written == played {
        return decoded;
    }
    match (&written, played) {
        (Instrument::Flute(_), Instrument::Flute(flute)) => decoded
            .into_iter()
            .map(|item| item.into_iter().map(|p| p.and_then(|p| onto_flute(flute, p))).collect())
            .collect(),
        (Instrument::Guitar(from), Instrument::Guitar(to)) => onto_guitar(from, to, &decoded),
        // Nothing substitutes across instruments.
        _ => decoded.iter().map(|item| vec![None; item.len()]).collect(),
    }
}

fn onto_flute(flute: &Flute, position: PlayPosition) -> Option<PlayPosition> {
    let PlayPosition::Fingering(at) = position else { return None };
    if flute.keys(at).is_some() {
        return Some(position);
    }
    let standard = FingeringAssignment { alternate: 0, ..at };
    flute.keys(standard).map(|_| PlayPosition::Fingering(standard))
}

fn onto_guitar(
    from: &Guitar,
    to: &Guitar,
    decoded: &[Vec<Option<PlayPosition>>],
) -> Vec<Vec<Option<PlayPosition>>> {
    // Per event: the fretted pitch a bend starts from (or the pitch itself),
    // the technique to put back, and the pitch it must still sound.
    struct Wanted {
        base: u8,
        technique: StringTechnique,
        sounds: u8,
    }
    let wanted: Vec<Vec<Option<Wanted>>> = decoded
        .iter()
        .map(|item| {
            item.iter()
                .map(|p| {
                    let PlayPosition::Fret(at) = (*p)? else { return None };
                    let sounds = from.pitch_of(&at)?;
                    let (base, technique) = match at.technique {
                        StringTechnique::Bend(_) => (
                            from.pitch_of(&FretAssignment {
                                technique: StringTechnique::Fretted,
                                ..at
                            })?,
                            at.technique,
                        ),
                        // A harmonic's pitch is its open string's, so on
                        // another tuning it is played as the note it sounds.
                        StringTechnique::Harmonic | StringTechnique::Fretted => {
                            (sounds, StringTechnique::Fretted)
                        }
                    };
                    Some(Wanted { base, technique, sounds })
                })
                .collect()
        })
        .collect();
    let onsets: Vec<Onset> = wanted
        .iter()
        .map(|item| Onset { notes: item.iter().flatten().map(|w| (w.base, None)).collect() })
        .collect();
    let voicings = fingering(to, &onsets);
    wanted
        .iter()
        .zip(voicings)
        .map(|(item, voicing)| {
            let mut placed = voicing.unwrap_or_default().into_iter();
            item.iter()
                .map(|w| {
                    let w = w.as_ref()?;
                    let at = placed.next()?;
                    let with_technique = FretAssignment { technique: w.technique, ..at };
                    (to.pitch_of(&with_technique) == Some(w.sounds))
                        .then_some(PlayPosition::Fret(with_technique))
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instrument::InstrumentModel;
    use crate::instrument::flute::FootJoint;
    use crate::instrument::guitar::{GuitarBody, TUNINGS};

    fn chart(json: &str) -> HarpChart {
        serde_json::from_str(json).unwrap()
    }

    fn flute_chart() -> HarpChart {
        chart(include_str!("../../../harmonicon-song/tests/fixtures/flute-scale.harpchart"))
    }

    fn guitar_chart() -> HarpChart {
        chart(include_str!("../../../harmonicon-song/tests/fixtures/guitar-highway.harpchart"))
    }

    #[test]
    fn the_charts_own_instrument_changes_nothing() {
        let chart = guitar_chart();
        let own = chart.instrument().unwrap();
        let placed = substitute_chart(&chart, &own);
        assert!(placed.iter().flatten().all(Option::is_some));
    }

    /// The fixture is a B-foot chart: on a C foot its B3 has nowhere to go,
    /// and every other note keeps its fingering, the alternate included.
    #[test]
    fn a_c_foot_drops_the_low_b_and_keeps_every_other_fingering() {
        let chart = flute_chart();
        let c_foot: Instrument = Flute::new(FootJoint::C).into();
        let placed = substitute_chart(&chart, &c_foot);
        assert_eq!(placed[0], [None], "B3");
        let own = substitute_chart(&chart, &chart.instrument().unwrap());
        assert_eq!(placed[1..], own[1..]);
    }

    /// Drop D lowers only the sixth string: its open E2 must now be fretted
    /// at the second fret, and every pitch still sounds as charted.
    #[test]
    fn drop_d_refrets_the_low_string_and_keeps_every_pitch() {
        let chart = guitar_chart();
        let drop_d = TUNINGS.iter().find(|t| t.id == "drop-d").unwrap();
        let played: Instrument =
            Guitar { strings: drop_d.strings.to_vec(), ..Guitar::standard(GuitarBody::Electric) }
                .into();
        let placed = substitute_chart(&chart, &played);
        assert_eq!(
            placed[0][0],
            Some(PlayPosition::Fret(FretAssignment {
                string: 6,
                fret: 2,
                technique: StringTechnique::Fretted
            }))
        );
        let own = substitute_chart(&chart, &chart.instrument().unwrap());
        for (item, own_item) in placed.iter().zip(&own) {
            for (p, o) in item.iter().zip(own_item) {
                let (Some(p), Some(o)) = (p, o) else { panic!("every note still has a place") };
                assert_eq!(played.pitch_of(p), chart.instrument().unwrap().pitch_of(o));
            }
        }
        // The fixture's whole-step bend on string 2 keeps its bend.
        assert!(matches!(
            placed[5][0],
            Some(PlayPosition::Fret(FretAssignment { technique: StringTechnique::Bend(_), .. }))
        ));
    }

    /// Down a half step, the 12th-fret harmonic on G (G4) can only be the
    /// plain fretted G4.
    #[test]
    fn a_harmonic_on_a_retuned_string_becomes_the_note_it_sounded() {
        let chart = guitar_chart();
        let down = TUNINGS.iter().find(|t| t.id == "half-step-down").unwrap();
        let played: Instrument =
            Guitar { strings: down.strings.to_vec(), ..Guitar::standard(GuitarBody::Electric) }
                .into();
        let Some(PlayPosition::Fret(at)) = substitute_chart(&chart, &played)[6][0] else {
            panic!("G4 is on the neck")
        };
        assert_eq!(at.technique, StringTechnique::Fretted);
        assert_eq!(played.pitch_of(&PlayPosition::Fret(at)), Some(67));
    }
}
