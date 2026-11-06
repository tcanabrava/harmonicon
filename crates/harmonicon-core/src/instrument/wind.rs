// SPDX-License-Identifier: MIT
//! Shared wind fingering vocabulary. Control IDs are local to an instrument's
//! chart; flute key bits are not a universal woodwind key map. Diagrams may
//! arrange these controls independently of pitch and assessment.
use super::flute::{Flute, FootJoint, Keys};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FingeringAssignment {
    pub note: u8,
    pub alternate: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ControlId(pub u8);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlMask(pub u64);
impl ControlMask {
    pub const fn contains(self, id: ControlId) -> bool {
        id.0 < 64 && self.0 & (1u64 << id.0) != 0
    }
    pub const fn without(self, ignored: Self) -> Self {
        Self(self.0 & !ignored.0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlKind {
    MainHole,
    AuxiliaryKey,
    RegisterKey,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finger {
    LeftThumb,
    LeftIndex,
    LeftMiddle,
    LeftRing,
    LeftLittle,
    RightIndex,
    RightMiddle,
    RightRing,
    RightLittle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindControl {
    pub id: ControlId,
    pub label: &'static str,
    pub kind: ControlKind,
    /// `None` when the chart does not prescribe which finger operates it.
    pub finger: Option<Finger>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FingeringPattern {
    pub position: FingeringAssignment,
    pub pressed: ControlMask,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegisterRule {
    pub semitones: u8,
    pub ignored_controls: ControlMask,
}

pub trait FingeringChart {
    fn controls(&self) -> Vec<WindControl>;
    /// Stable ordering and alternate indices; pitches are sounding pitches.
    fn patterns(&self) -> Vec<FingeringPattern>;
    fn register_rule(&self) -> Option<RegisterRule>;
    /// Teaching pairs can include verified instrument-specific alternatives.
    fn register_exercises(&self) -> Vec<(FingeringAssignment, FingeringAssignment)> {
        register_pairs(self)
    }
    fn pattern(&self, at: FingeringAssignment) -> Option<ControlMask> {
        self.patterns()
            .into_iter()
            .find(|pattern| pattern.position == at)
            .map(|pattern| pattern.pressed)
    }
}
pub fn positions_for(
    chart: &(impl FingeringChart + ?Sized),
    pitch: u8,
) -> Vec<FingeringAssignment> {
    chart
        .patterns()
        .into_iter()
        .filter(|pattern| pattern.position.note == pitch)
        .map(|pattern| pattern.position)
        .collect()
}
/// A register exercise needs both the declared interval and the required
/// control relationship. Identical keys alone never imply an octave.
pub fn register_pairs(
    chart: &(impl FingeringChart + ?Sized),
) -> Vec<(FingeringAssignment, FingeringAssignment)> {
    let Some(rule) = chart.register_rule() else { return Vec::new() };
    let patterns: Vec<_> =
        chart.patterns().into_iter().filter(|pattern| pattern.position.alternate == 0).collect();
    let mut pairs = Vec::new();
    for start in &patterns {
        if let Some(target) = patterns.iter().find(|target| {
            start.position.note.checked_add(rule.semitones) == Some(target.position.note)
                && start.pressed.without(rule.ignored_controls)
                    == target.pressed.without(rule.ignored_controls)
        }) {
            pairs.push((start.position, target.position));
        }
    }
    pairs
}

impl FingeringChart for Flute {
    fn register_exercises(&self) -> Vec<(FingeringAssignment, FingeringAssignment)> {
        // Keep the flute's verified third-register and alternate exercises.
        self.overblown_pairs()
    }
    fn controls(&self) -> Vec<WindControl> {
        Keys::ALL
            .into_iter()
            .filter(|&key| self.foot == FootJoint::B || key != Keys::B)
            .map(|key| {
                let finger = match key {
                    Keys::TH => Some(Finger::LeftThumb),
                    Keys::L1 => Some(Finger::LeftIndex),
                    Keys::L2 => Some(Finger::LeftMiddle),
                    Keys::L3 => Some(Finger::LeftRing),
                    Keys::GS => Some(Finger::LeftLittle),
                    Keys::R1 => Some(Finger::RightIndex),
                    Keys::R2 => Some(Finger::RightMiddle),
                    Keys::R3 => Some(Finger::RightRing),
                    Keys::EB | Keys::CS | Keys::C | Keys::B => Some(Finger::RightLittle),
                    _ => None,
                };
                WindControl {
                    id: ControlId(key.trailing_zeros() as u8),
                    label: super::flute::key_label(key),
                    kind: if matches!(
                        key,
                        Keys::L1 | Keys::L2 | Keys::L3 | Keys::R1 | Keys::R2 | Keys::R3
                    ) {
                        ControlKind::MainHole
                    } else {
                        ControlKind::AuxiliaryKey
                    },
                    finger,
                }
            })
            .collect()
    }
    fn patterns(&self) -> Vec<FingeringPattern> {
        let mut alternates = std::collections::BTreeMap::<u8, u8>::new();
        Flute::fingerings(self)
            .into_iter()
            .map(|fingering| {
                let alternate = alternates.entry(fingering.note).or_default();
                let position = FingeringAssignment { note: fingering.note, alternate: *alternate };
                *alternate += 1;
                FingeringPattern { position, pressed: ControlMask(u64::from(fingering.keys.0)) }
            })
            .collect()
    }
    fn register_rule(&self) -> Option<RegisterRule> {
        Some(RegisterRule { semitones: 12, ignored_controls: ControlMask::default() })
    }
    fn pattern(&self, at: FingeringAssignment) -> Option<ControlMask> {
        self.keys(at).map(|keys| ControlMask(u64::from(keys.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flute_adapter_preserves_foot_range_alternates_and_every_pressed_key() {
        for foot in [FootJoint::C, FootJoint::B] {
            let flute = Flute::new(foot);
            let controls = flute.controls();
            assert_eq!(controls.len(), if foot == FootJoint::B { 14 } else { 13 });
            let allowed = controls.iter().fold(0u64, |mask, control| mask | (1 << control.id.0));
            for pattern in flute.patterns() {
                assert_eq!(pattern.pressed.0 & !allowed, 0);
                assert_eq!(pattern.pressed.0, u64::from(flute.keys(pattern.position).unwrap().0));
                assert!(positions_for(&flute, pattern.position.note).contains(&pattern.position));
            }
            assert_eq!(flute.patterns()[0].position.note, flute.lowest());
            assert!(flute.pattern(FingeringAssignment { note: 1, alternate: 0 }).is_none());
        }
    }
    #[test]
    fn register_pairs_use_the_declared_interval_and_control_relationship() {
        let pairs = register_pairs(&Flute::default());
        assert!(!pairs.is_empty());
        assert!(pairs.iter().all(|(start, target)| target.note - start.note == 12));
        assert!(pairs.iter().any(|(start, target)| start.note == 64 && target.note == 76));
        assert!(!ControlMask(u64::MAX).contains(ControlId(64)));
    }
}
