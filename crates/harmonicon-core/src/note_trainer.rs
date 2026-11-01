// SPDX-License-Identifier: MIT

//! What the Note Trainer drills, for any instrument, and how a heard pitch
//! reads against a drill.
//!
//! A drill is one [`TechniqueKind`] at one [`PlayPosition`]: hit and hold a
//! note (`Target`), slide from a plain note to a bent one (`Glide`), jump
//! from a plain note to an overblow or a harmonic (`Register`), or find a
//! pitch in one particular place among several (`Position`). Which drills
//! exist is per instrument, so [`drills`] matches on the closed
//! [`Instrument`] enum: a new instrument fails to compile until it says
//! what it drills. An instrument only gets the kinds its
//! [`InstrumentModel::techniques`] lists.
//!
//! Pitches are fractional MIDI numbers, as [`NoteTarget::sounding_pitch`]
//! gives them, so a quarter-tone bend is aimed at where it really is.

use crate::chart::Action;
use crate::harmonica::Harmonica;
use crate::instrument::guitar::{FretAssignment, Guitar, StringTechnique};
use crate::instrument::{Instrument, InstrumentModel, PlayPosition, TechniqueKind};
use crate::note_target::NoteTarget;
use crate::pitch_map::{HoleAssignment, Technique, max_bend};

/// One thing to practise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drill {
    pub kind: TechniqueKind,
    pub position: PlayPosition,
    /// The plain note a glide or register jump starts from: the same hole
    /// and breath unbent, or the same string and fret fretted. `None` for a
    /// drill that is just its target.
    pub start: Option<f32>,
    /// The pitch to land on.
    pub target: f32,
}

/// Every `kind` drill `instrument` offers, in a stable order (row, then
/// how far the technique goes). Empty for a kind the instrument doesn't
/// list in `techniques()`.
pub fn drills(instrument: &Instrument, kind: TechniqueKind) -> Vec<Drill> {
    if !instrument.techniques().contains(&kind) {
        return Vec::new();
    }
    let candidates = match instrument {
        Instrument::Harmonica(harp) => harmonica_candidates(harp, kind),
        Instrument::Guitar(guitar) => guitar_candidates(guitar, kind),
    };
    let pitch = |p: PlayPosition| NoteTarget::Positioned(p).sounding_pitch(instrument);
    candidates
        .into_iter()
        .filter_map(|(position, start)| {
            let target = pitch(position)?;
            let start = match start {
                Some(plain) => Some(pitch(plain)?),
                None => None,
            };
            Some(Drill { kind, position, start, target })
        })
        .collect()
}

/// `(position, the plain position it starts from)` for each harmonica
/// drill of `kind`. Candidates the harp can't play are dropped by
/// [`drills`], which asks the harp for each pitch.
fn harmonica_candidates(
    harp: &Harmonica,
    kind: TechniqueKind,
) -> Vec<(PlayPosition, Option<PlayPosition>)> {
    let hole =
        |hole, action, technique| PlayPosition::Hole(HoleAssignment { hole, action, technique });
    let mut out = Vec::new();
    for h in 1..=harp.hole_count() {
        for action in [Action::Blow, Action::Draw] {
            let plain = hole(h, action, Technique::Natural);
            match kind {
                TechniqueKind::Target => out.push((plain, None)),
                TechniqueKind::Glide => {
                    for depth in 1..=max_bend(harp, h) as u8 {
                        out.push((hole(h, action, Technique::Bend(f32::from(depth))), Some(plain)));
                    }
                }
                TechniqueKind::Register => {
                    for over in [Technique::Overblow, Technique::Overdraw] {
                        out.push((hole(h, action, over), Some(plain)));
                    }
                }
                TechniqueKind::Position => {}
            }
        }
    }
    out
}

/// `(position, the plain position it starts from)` for each guitar drill
/// of `kind`. Bends are a half step and a whole step, the two a player
/// learns first; an open string has no fret to push against, so it isn't
/// bent. A harmonic starts from its open string, the fundamental it sits
/// above. A `Position` drill is every fretted place a pitch can be found
/// in more than one.
fn guitar_candidates(
    guitar: &Guitar,
    kind: TechniqueKind,
) -> Vec<(PlayPosition, Option<PlayPosition>)> {
    let at =
        |string, fret, technique| PlayPosition::Fret(FretAssignment { string, fret, technique });
    let mut out = Vec::new();
    for string in 1..=guitar.string_count() {
        for fret in 0..=guitar.top_fret() {
            let plain = at(string, fret, StringTechnique::Fretted);
            match kind {
                TechniqueKind::Target => out.push((plain, None)),
                TechniqueKind::Glide if fret > 0 => {
                    for depth in [1.0, 2.0] {
                        out.push((at(string, fret, StringTechnique::Bend(depth)), Some(plain)));
                    }
                }
                TechniqueKind::Register => {
                    let open = at(string, 0, StringTechnique::Fretted);
                    out.push((at(string, fret, StringTechnique::Harmonic), Some(open)));
                }
                TechniqueKind::Position => {
                    let shared = guitar
                        .pitch_of(&FretAssignment {
                            string,
                            fret,
                            technique: StringTechnique::Fretted,
                        })
                        .is_some_and(|midi| guitar.fretted_positions(midi).len() > 1);
                    if shared {
                        out.push((plain, None));
                    }
                }
                TechniqueKind::Glide => {}
            }
        }
    }
    out
}

/// How far from a drill's target a pitch may sit and still count as
/// landing on it: a quarter tone, half a semitone. Narrower judging (the
/// trainer's own tolerance setting) is the caller's to apply to the cents
/// [`Heard::Target`] reports.
pub const LANDING_CENTS: f32 = 50.0;

/// What a heard pitch is, against one drill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heard {
    /// On the target, this many cents sharp (positive) or flat.
    Target { cents: f32 },
    /// Partway along a glide, from its start (0) to its target (1).
    Gliding { progress: f32 },
    /// The plain note the drill starts from.
    Start,
    /// Something else: another note, an octave error, an unrelated
    /// harmonic. Never turned into a large cents value.
    Elsewhere,
}

/// Reads `midi` (fractional) against `drill`. Only a glide has a way in
/// between its start and target; a register jump is discrete, so a pitch
/// between a plain note and its harmonic is a crack, not progress.
pub fn hear(drill: &Drill, midi: f32) -> Heard {
    let cents = (midi - drill.target) * 100.0;
    if cents.abs() <= LANDING_CENTS {
        return Heard::Target { cents };
    }
    let Some(start) = drill.start else { return Heard::Elsewhere };
    if ((midi - start) * 100.0).abs() <= LANDING_CENTS {
        return Heard::Start;
    }
    let (low, high) = (start.min(drill.target), start.max(drill.target));
    if drill.kind == TechniqueKind::Glide && (low..=high).contains(&midi) {
        return Heard::Gliding { progress: (midi - start) / (drill.target - start) };
    }
    Heard::Elsewhere
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonica::{chromatic_harp, richter_harp};
    use crate::instrument::guitar::GuitarBody;

    fn harp() -> Instrument {
        richter_harp("C").into()
    }

    fn guitar() -> Instrument {
        Guitar::standard(GuitarBody::Electric).into()
    }

    fn find(drills: &[Drill], position: PlayPosition) -> Option<Drill> {
        drills.iter().copied().find(|d| d.position == position)
    }

    #[test]
    fn a_harmonica_targets_every_reed_and_glides_down_its_bends() {
        let targets = drills(&harp(), TechniqueKind::Target);
        assert_eq!(targets.len(), 20, "ten holes, blow and draw");
        let glides = drills(&harp(), TechniqueKind::Glide);
        // Draw 3 on a C harp is B4 and bends down to A4 at most.
        let deepest = PlayPosition::Hole(HoleAssignment {
            hole: 3,
            action: Action::Draw,
            technique: Technique::Bend(3.0),
        });
        let d = find(&glides, deepest).expect("draw 3 bends three semitones");
        assert_eq!((d.start, d.target), (Some(71.0), 68.0));
        // Hole 5 doesn't bend; hole 10 bends on the blow side.
        assert!(glides.iter().all(|d| !matches!(d.position, PlayPosition::Hole(a) if a.hole == 5)));
        assert!(glides.iter().any(|d| matches!(
            d.position,
            PlayPosition::Hole(a) if a.hole == 10 && a.action == Action::Blow
        )));
    }

    #[test]
    fn a_harmonica_register_jump_is_its_overblows_and_overdraws() {
        let jumps = drills(&harp(), TechniqueKind::Register);
        // Overblow 6 sounds draw 6 (A5) plus a semitone.
        let ob6 = PlayPosition::Hole(HoleAssignment {
            hole: 6,
            action: Action::Blow,
            technique: Technique::Overblow,
        });
        let d = find(&jumps, ob6).expect("hole 6 overblows");
        assert_eq!((d.start, d.target), (Some(79.0), 82.0));
        assert!(drills(&harp(), TechniqueKind::Position).is_empty(), "not a harmonica drill");
    }

    #[test]
    fn a_chromatic_offers_only_its_targets() {
        let chromatic: Instrument = chromatic_harp("C").into();
        assert!(!drills(&chromatic, TechniqueKind::Target).is_empty());
        assert!(drills(&chromatic, TechniqueKind::Glide).is_empty());
    }

    #[test]
    fn a_guitar_bends_fretted_notes_and_finds_harmonics() {
        let g = guitar();
        let bend = PlayPosition::Fret(FretAssignment {
            string: 3,
            fret: 7,
            technique: StringTechnique::Bend(2.0),
        });
        let d = find(&drills(&g, TechniqueKind::Glide), bend).expect("a whole-step bend");
        assert_eq!((d.start, d.target), (Some(62.0), 64.0));
        assert!(
            drills(&g, TechniqueKind::Glide)
                .iter()
                .all(|d| !matches!(d.position, PlayPosition::Fret(a) if a.fret == 0)),
            "an open string isn't bent"
        );
        let harmonic = PlayPosition::Fret(FretAssignment {
            string: 6,
            fret: 12,
            technique: StringTechnique::Harmonic,
        });
        let d = find(&drills(&g, TechniqueKind::Register), harmonic).expect("the 12th-fret node");
        assert_eq!((d.start, d.target), (Some(40.0), 52.0), "open E2, then its octave");
        let none_at_2 = PlayPosition::Fret(FretAssignment {
            string: 6,
            fret: 2,
            technique: StringTechnique::Harmonic,
        });
        assert!(find(&drills(&g, TechniqueKind::Register), none_at_2).is_none());
    }

    #[test]
    fn a_guitar_position_drill_is_a_pitch_with_more_than_one_place() {
        let positions = drills(&guitar(), TechniqueKind::Position);
        let open_e = PlayPosition::Fret(FretAssignment {
            string: 1,
            fret: 0,
            technique: StringTechnique::Fretted,
        });
        assert!(find(&positions, open_e).is_some(), "E4 is also B string fret 5");
        let low_e = PlayPosition::Fret(FretAssignment {
            string: 6,
            fret: 0,
            technique: StringTechnique::Fretted,
        });
        assert!(find(&positions, low_e).is_none(), "E2 exists only there");
    }

    fn glide() -> Drill {
        Drill {
            kind: TechniqueKind::Glide,
            position: PlayPosition::natural_hole(3, Action::Draw),
            start: Some(71.0),
            target: 69.0,
        }
    }

    #[test]
    fn a_heard_pitch_reads_as_start_on_the_way_or_landed() {
        assert_eq!(hear(&glide(), 71.1), Heard::Start);
        assert_eq!(hear(&glide(), 70.0), Heard::Gliding { progress: 0.5 });
        let Heard::Target { cents } = hear(&glide(), 68.8) else { panic!("landed") };
        assert!((cents + 20.0).abs() < 0.01, "20 cents flat, got {cents}");
    }

    #[test]
    fn octaves_and_overshoots_are_elsewhere_not_large_cents() {
        assert_eq!(hear(&glide(), 57.0), Heard::Elsewhere, "an octave below the target");
        assert_eq!(hear(&glide(), 67.0), Heard::Elsewhere, "bent past it");
        let harmonic =
            Drill { kind: TechniqueKind::Register, start: Some(52.0), target: 64.0, ..glide() };
        assert_eq!(hear(&harmonic, 58.0), Heard::Elsewhere, "a register jump has no way between");
    }
}
