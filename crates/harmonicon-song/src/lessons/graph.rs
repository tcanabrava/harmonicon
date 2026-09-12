// SPDX-License-Identifier: MIT

//! The curriculum as a graph: depth, ordering, and what a player can do next.
//!
//! The prerequisite edges have always been there (`LessonManifest::
//! prerequisites`); what was missing is anything that treats them as a
//! *graph* rather than a per-lesson gate. `lessons::is_unlocked` answers
//! "may I start this one", which is all the flat list ever needed. A skill
//! tree needs rows, an order along each row, and a guarantee the thing can
//! be drawn at all — see `docs/training_tree_plan.md`.
//!
//! Pure and Bevy-free, like the rest of `lessons`' data layer.
//!
//! **Only one of the checks this module runs is real.** Three were planned:
//! - **Cycles** — genuinely possible, genuinely unchecked before this, and
//!   fatal twice over: a lesson in a cycle can never unlock, and a renderer
//!   walking the edges would not terminate. [`LessonGraph::build`] refuses
//!   to construct rather than returning something unusable.
//! - *Every edge points forward* — vacuous. [`depth`](LessonNode::depth) is
//!   the longest path from a root, so a prerequisite's depth is always less
//!   than its dependent's by construction. There is nothing to check.
//! - *Every lesson reachable from a root* — vacuous for the same reason.
//!   Any node of a finite DAG is reachable from some node with no incoming
//!   edges; the only way to fail is a prerequisite naming a lesson that does
//!   not exist, which is [`GraphError::UnknownPrerequisite`].
//!
//! A fourth was planned and turned out to be **false of the real
//! curriculum**: "at least two lessons are always available, so the player
//! always has a choice". Sampled over the shipped curriculum, the graph funnels to a
//! single option at several points — `deep-bends` most often, then
//! `swing-eighths` and `blues-scale`. So [`min_choices`] reports the number
//! instead of asserting it, and widening those chokepoints is curriculum
//! work rather than something a test can enforce.

use std::collections::{HashMap, HashSet};

use super::manifest::LessonManifest;

/// One lesson, placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonNode {
    pub id: String,
    /// The skill-tree row — `LessonManifest::track`, already defaulted.
    pub track: String,
    pub prerequisites: Vec<String>,
    /// Longest path from a lesson with no prerequisites. Longest, not
    /// shortest: a lesson is only truly reachable once its *deepest*
    /// prerequisite is, so the longest path is what says when it can
    /// actually be played.
    pub depth: usize,
}

/// One row of the skill tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub track: String,
    /// Members ordered left to right: by depth, then by id so the order is
    /// stable across runs.
    ///
    /// **A row is a family, not a chain.** Adjacent members need not depend
    /// on each other — `country-scale` does not lead to `blues-scale`, they
    /// merely belong together — so a renderer must draw the real edges from
    /// [`LessonNode::prerequisites`] and never a link between neighbours.
    pub lessons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// The ids still unplaced when a topological sort ran out of nodes with
    /// no unmet prerequisites — every one is in, or downstream of, a cycle.
    Cycle(Vec<String>),
    /// A prerequisite naming a lesson that does not exist. Separate from a
    /// cycle because the fix is different: a typo, or a lesson deleted out
    /// from under its dependents.
    UnknownPrerequisite { lesson: String, missing: String },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::Cycle(ids) => {
                write!(f, "prerequisite cycle involving: {}", ids.join(", "))
            }
            GraphError::UnknownPrerequisite { lesson, missing } => {
                write!(
                    f,
                    "lesson '{lesson}' requires '{missing}', which does not exist"
                )
            }
        }
    }
}

/// The curriculum, ordered and depth-assigned.
#[derive(Debug, Clone, Default)]
pub struct LessonGraph {
    /// Topologically ordered, so any prerequisite precedes its dependents.
    nodes: Vec<LessonNode>,
    index: HashMap<String, usize>,
}

impl LessonGraph {
    /// Places every lesson, or says why it can't.
    pub fn build(manifests: &[LessonManifest]) -> Result<Self, GraphError> {
        let known: HashSet<&str> = manifests.iter().map(|m| m.id.as_str()).collect();
        for m in manifests {
            for p in &m.prerequisites {
                if !known.contains(p.as_str()) {
                    return Err(GraphError::UnknownPrerequisite {
                        lesson: m.id.clone(),
                        missing: p.clone(),
                    });
                }
            }
        }

        // Kahn's algorithm, taking ids in sorted order at each step so the
        // result is deterministic rather than dependent on scan order.
        let mut remaining: Vec<&LessonManifest> = manifests.iter().collect();
        remaining.sort_by(|a, b| a.id.cmp(&b.id));

        let mut placed: HashMap<String, usize> = HashMap::new();
        let mut nodes: Vec<LessonNode> = Vec::with_capacity(manifests.len());

        while !remaining.is_empty() {
            let ready: Vec<&LessonManifest> = remaining
                .iter()
                .copied()
                .filter(|m| m.prerequisites.iter().all(|p| placed.contains_key(p)))
                .collect();
            if ready.is_empty() {
                // Nothing can be placed and nodes are left: every survivor
                // is in a cycle or waiting on one.
                let mut stuck: Vec<String> = remaining.iter().map(|m| m.id.clone()).collect();
                stuck.sort();
                return Err(GraphError::Cycle(stuck));
            }
            for m in &ready {
                let depth = m
                    .prerequisites
                    .iter()
                    .filter_map(|p| placed.get(p))
                    .map(|d| d + 1)
                    .max()
                    .unwrap_or(0);
                placed.insert(m.id.clone(), depth);
                nodes.push(LessonNode {
                    id: m.id.clone(),
                    track: m.track().to_string(),
                    prerequisites: m.prerequisites.clone(),
                    depth,
                });
            }
            let done: HashSet<&str> = ready.iter().map(|m| m.id.as_str()).collect();
            remaining.retain(|m| !done.contains(m.id.as_str()));
        }

        let index = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), i))
            .collect();
        Ok(Self { nodes, index })
    }

    /// Every lesson, in topological order.
    pub fn nodes(&self) -> &[LessonNode] {
        &self.nodes
    }

    pub fn get(&self, id: &str) -> Option<&LessonNode> {
        self.index.get(id).map(|&i| &self.nodes[i])
    }

    /// How deep the curriculum goes — the number of rows a layered drawing
    /// would need.
    pub fn max_depth(&self) -> usize {
        self.nodes.iter().map(|n| n.depth).max().unwrap_or(0)
    }

    /// Lessons whose prerequisites are all met and which are not passed
    /// yet: exactly what the player may start next.
    pub fn available(&self, passed: &HashSet<&str>) -> Vec<&str> {
        self.nodes
            .iter()
            .filter(|n| !passed.contains(n.id.as_str()))
            .filter(|n| n.prerequisites.iter().all(|p| passed.contains(p.as_str())))
            .map(|n| n.id.as_str())
            .collect()
    }

    /// The skill tree's rows, ordered for drawing.
    ///
    /// Tracks are ordered by how early they can be started (the shallowest
    /// member), then by size descending, then by name — so foundations sit
    /// at the top and the long tracks lead, without the order depending on
    /// which file happened to be read first.
    pub fn rows(&self) -> Vec<Row> {
        let mut by_track: HashMap<&str, Vec<&LessonNode>> = HashMap::new();
        for n in &self.nodes {
            by_track.entry(n.track.as_str()).or_default().push(n);
        }
        let mut rows: Vec<Row> = by_track
            .into_iter()
            .map(|(track, mut members)| {
                members.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.id.cmp(&b.id)));
                Row {
                    track: track.to_string(),
                    lessons: members.iter().map(|n| n.id.clone()).collect(),
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            let key = |r: &Row| {
                let shallowest = r
                    .lessons
                    .iter()
                    .filter_map(|id| self.get(id))
                    .map(|n| n.depth)
                    .min()
                    .unwrap_or(usize::MAX);
                (shallowest, std::cmp::Reverse(r.lessons.len()))
            };
            key(a).cmp(&key(b)).then_with(|| a.track.cmp(&b.track))
        });
        rows
    }
}

/// The fewest lessons a player is ever offered at once, over `trials`
/// randomly-ordered playthroughs, while at least `while_remaining` lessons
/// are still unpassed.
///
/// **A measurement, not a check.** The design goal was that a player should
/// always have a choice of at least two, but the shipped curriculum funnels
/// to one at several points, so asserting it would either fail or have to be
/// weakened until it only described today's data. Reporting the number lets
/// the curriculum be widened deliberately.
///
/// Deterministic: `seed` drives a small xorshift rather than a dependency,
/// so the same seed always walks the same orders.
pub fn min_choices(graph: &LessonGraph, trials: u32, while_remaining: usize, seed: u64) -> usize {
    let mut state = seed | 1;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let total = graph.nodes().len();
    let mut fewest = usize::MAX;
    for _ in 0..trials {
        let mut passed: HashSet<&str> = HashSet::new();
        while passed.len() < total {
            let options = graph.available(&passed);
            if options.is_empty() {
                break;
            }
            if total - passed.len() >= while_remaining {
                fewest = fewest.min(options.len());
            }
            let pick = (next() % options.len() as u64) as usize;
            passed.insert(options[pick]);
        }
    }
    if fewest == usize::MAX { 0 } else { fewest }
}

#[cfg(test)]
mod tests;
