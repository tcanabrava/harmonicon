// SPDX-License-Identifier: MIT

//! Units as a chain of gates: the curriculum's second, coarser level.
//!
//! [`graph`](super::graph) treats the prerequisite edges as a graph, which
//! is the fine structure — "may I start *this* lesson". This module is the
//! coarse one: the five shipped units in order, each opening only once
//! enough of the one before it is done. Drawn, they are the spine a skill
//! tree hangs off; read, they are what turns forty-one loose lessons into
//! "Unit 1, then Unit 2".
//!
//! **Nothing new is authored for this.** A unit's identity is
//! `LessonManifest::unit`, its order is the order lessons were discovered
//! in (`catalog`'s scan sorts by the `01_`/`02_` directory prefixes), and
//! its display name is the `lesson-unit-<id>` Fluent key the lesson list
//! has always used. There is no `unit.json`, and adding one would mean a
//! second build-script manifest for wasm/Android to carry — worth it only
//! once a unit needs something that genuinely can't be derived.
//!
//! **The unit order is a valid layering of the prerequisite graph**, which
//! is what makes gating on it safe: measured over the shipped curriculum,
//! all eighteen cross-unit prerequisites point forward (`blowing` →
//! `rhythm` → `blues` → …), so a unit gate can never contradict a lesson's
//! own prerequisites or deadlock the player.
//! [`crossing_prerequisites`] reports any that don't, and
//! `tests/asset_layout.rs` fails the build over one.
//!
//! Pure and Bevy-free, like the rest of `lessons`' data layer.

use std::collections::HashSet;

use super::manifest::LessonManifest;

/// The fraction of a unit's lessons that must be passed before the next
/// unit opens.
///
/// **Deliberately not all of them.** Requiring every lesson would make
/// `deep-bends` — the hardest thing on a diatonic harmonica, and a lesson
/// a beginner can genuinely stall on for weeks — a hard wall in front of
/// the entire rest of the course. A threshold keeps one bad day from
/// stopping progress while still leaving the completionist something to
/// come back for. The gate is on *count*, not on which ones: skill trees
/// generally open a tier on points spent in it, not on a specific node.
pub const UNIT_UNLOCK_THRESHOLD: f32 = 0.7;

/// One unit, and the lessons inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitNode {
    /// `LessonManifest::unit` — `"blowing"`, `"rhythm"`, …
    pub id: String,
    /// Fluent key for the display name. Derived, never stored: the
    /// curriculum has used `lesson-unit-<id>` since the lesson list's unit
    /// tabs, and all three locales already define one per shipped unit.
    pub title_key: String,
    /// Member lesson ids, in discovery order.
    pub lessons: Vec<String>,
}

/// Every unit, in curriculum order, each gating the next.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitChain {
    units: Vec<UnitNode>,
}

impl UnitChain {
    /// Groups `manifests` into units, preserving the order each unit first
    /// appears in — which for the bundled curriculum is the `01_`/`02_`
    /// directory order `catalog`'s scan sorted by.
    ///
    /// Unlike [`LessonGraph::build`](super::graph::LessonGraph::build) this
    /// cannot fail: a lesson always has a unit (the manifest field is
    /// required), and a unit is whatever its lessons say it is, so there is
    /// no such thing as an unknown or cyclic one.
    pub fn build(manifests: &[LessonManifest]) -> Self {
        let mut units: Vec<UnitNode> = Vec::new();
        for m in manifests {
            match units.iter_mut().find(|u| u.id == m.unit) {
                Some(unit) => unit.lessons.push(m.id.clone()),
                None => units.push(UnitNode {
                    id: m.unit.clone(),
                    title_key: format!("lesson-unit-{}", m.unit),
                    lessons: vec![m.id.clone()],
                }),
            }
        }
        Self { units }
    }

    pub fn units(&self) -> &[UnitNode] {
        &self.units
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Where a unit sits in the chain, by id.
    pub fn index_of(&self, unit: &str) -> Option<usize> {
        self.units.iter().position(|u| u.id == unit)
    }

    /// Which unit a lesson belongs to.
    pub fn unit_of(&self, lesson: &str) -> Option<usize> {
        self.units
            .iter()
            .position(|u| u.lessons.iter().any(|l| l == lesson))
    }

    /// How many of this unit's lessons must be passed for the next one to
    /// open — [`UNIT_UNLOCK_THRESHOLD`] of its size, rounded up, and never
    /// zero (an empty unit would otherwise open the next one for free).
    pub fn required(&self, unit: usize) -> usize {
        let Some(node) = self.units.get(unit) else {
            return 0;
        };
        let total = node.lessons.len();
        ((total as f32 * UNIT_UNLOCK_THRESHOLD).ceil() as usize).clamp(1, total.max(1))
    }

    /// How many of this unit's lessons are passed.
    pub fn completed(&self, unit: usize, passed: &HashSet<&str>) -> usize {
        self.units
            .get(unit)
            .map(|u| {
                u.lessons
                    .iter()
                    .filter(|l| passed.contains(l.as_str()))
                    .count()
            })
            .unwrap_or(0)
    }

    /// Whether this unit has met its own threshold — i.e. whether it has
    /// done enough to open the one after it.
    pub fn is_satisfied(&self, unit: usize, passed: &HashSet<&str>) -> bool {
        self.completed(unit, passed) >= self.required(unit)
    }

    /// Whether this unit is open to the player: the first one always is,
    /// and any later one needs *every* unit before it satisfied.
    ///
    /// Cumulative rather than just checking the immediate predecessor —
    /// with lesson prerequisites also in play the two nearly always agree,
    /// but "everything before this is done" is the claim the drawing makes,
    /// so it is the one worth checking.
    pub fn is_unlocked(&self, unit: usize, passed: &HashSet<&str>) -> bool {
        (0..unit.min(self.units.len())).all(|earlier| self.is_satisfied(earlier, passed))
    }
}

/// Prerequisites that cross a unit boundary *backwards* — a lesson in an
/// earlier unit depending on a later one.
///
/// Each is a `(lesson, prerequisite)` pair. Any result at all means the
/// unit order is no longer a valid layering of the prerequisite graph, and
/// gating on it would lock a player out of a lesson they can never reach:
/// the prerequisite's unit only opens once the dependent's unit is done,
/// and the dependent can never be done. Forward and same-unit edges are
/// fine and are not reported.
pub fn crossing_prerequisites(manifests: &[LessonManifest]) -> Vec<(String, String)> {
    let chain = UnitChain::build(manifests);
    let mut backwards = Vec::new();
    for m in manifests {
        let Some(here) = chain.index_of(&m.unit) else {
            continue;
        };
        for p in &m.prerequisites {
            let Some(there) = chain.unit_of(p) else {
                continue; // an unknown prerequisite is `graph`'s error to raise
            };
            if there > here {
                backwards.push((m.id.clone(), p.clone()));
            }
        }
    }
    backwards
}

#[cfg(test)]
mod tests;
