// SPDX-License-Identifier: MIT

//! Where every node of the skill tree goes, and what state it is in.
//!
//! Pure — no Bevy, no assets, no pixels. It answers "which column, which
//! row, locked or not, how much of the ladder is done", and the spawn code
//! beside it turns that into nodes. Same split the notation staff uses:
//! decisions here, translation there, so the decisions are testable.
//!
//! **A layered drawing, not a grid of tracks.** A node's column is its
//! depth in the prerequisite graph, so it always sits right of everything
//! it needs; its row is chosen to keep edges from crossing. Tracks stop
//! being rows and become colour — the shape of the graph is what carries
//! the meaning, the way a tech tree reads.
//!
//! **Grid coordinates, not pixels.** Node size and spacing are the
//! renderer's business and change with the theme; which node sits left of
//! which does not.

use std::collections::{HashMap, HashSet};

use harmonicon_app::profile::PlayerProfile;
use harmonicon_core::training::Tier;
use harmonicon_song::lessons::LessonEntry;
use harmonicon_song::lessons::graph::LessonGraph;

/// Crossing-reduction sweeps. Four down-and-up passes is well past the
/// point this curriculum stops improving; it is cheap and runs once.
const ORDERING_PASSES: usize = 4;

/// How a node reads at a glance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeState {
    /// A prerequisite is still unmet.
    Locked,
    /// Playable now, not yet passed.
    Available,
    /// Passed. Its ladder may still have tiers left.
    Passed,
    /// Passed, and every training tier with it.
    Mastered,
}

#[derive(Clone, PartialEq, Debug)]
pub struct PlacedNode {
    pub id: String,
    /// Fluent key — the tree never holds display text, same rule as the
    /// rest of the lesson UI.
    pub title_key: String,
    /// The track this lesson belongs to. Drawn as colour rather than as a
    /// row, so grouping survives without constraining position.
    pub track: String,
    /// The lesson's depth in the prerequisite graph. A node always sits to
    /// the right of everything it depends on.
    pub column: usize,
    /// Vertical position within the column, in node-heights. Fractional so
    /// a short layer can be centred against a tall one.
    pub row: f32,
    pub state: NodeState,
    /// 0..1, how much of the training ladder is passed.
    pub mastery: f32,
    /// Whether this lesson has trainings at all. A lesson with none draws
    /// no ring — there is nothing to fill, and an empty ring would read as
    /// "you have done none of it" rather than "there is none".
    pub has_trainings: bool,
}

/// A prerequisite edge, in grid coordinates.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Edge {
    pub from: (usize, f32),
    pub to: (usize, f32),
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct TreeLayout {
    pub nodes: Vec<PlacedNode>,
    pub edges: Vec<Edge>,
}

impl TreeLayout {
    /// Columns the canvas needs — one past the deepest node.
    pub fn columns(&self) -> usize {
        self.nodes.iter().map(|n| n.column + 1).max().unwrap_or(0)
    }

    /// Rows the canvas needs, in node-heights.
    pub fn rows(&self) -> f32 {
        self.nodes
            .iter()
            .map(|n| n.row + 1.0)
            .fold(0.0_f32, f32::max)
    }

    #[cfg(test)]
    pub fn node(&self, id: &str) -> Option<&PlacedNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Places every lesson, given what the player has done.
pub fn layout(entries: &[LessonEntry], graph: &LessonGraph, profile: &PlayerProfile) -> TreeLayout {
    let passed: HashSet<&str> = profile.passed_lesson_ids().into_iter().collect();
    let tiers = Tier::ALL.len();

    // Only lessons the catalogue can label. One in the graph but not here
    // is skipped rather than drawn as a blank node.
    let mut nodes: Vec<PlacedNode> = Vec::new();
    for entry in entries {
        let id = &entry.manifest.id;
        let Some(graph_node) = graph.get(id) else {
            continue;
        };
        let unlocked = graph_node
            .prerequisites
            .iter()
            .all(|p| passed.contains(p.as_str()));
        let is_passed = passed.contains(id.as_str());
        let has_trainings = entry.manifest.training.is_some();
        let mastery = if has_trainings {
            profile.mastery(id, tiers)
        } else {
            0.0
        };
        nodes.push(PlacedNode {
            id: id.clone(),
            title_key: entry.manifest.title_key.clone(),
            track: graph_node.track.clone(),
            column: graph_node.depth,
            row: 0.0,
            state: match (unlocked, is_passed) {
                (false, false) => NodeState::Locked,
                (_, true) if has_trainings && mastery >= 1.0 => NodeState::Mastered,
                (_, true) => NodeState::Passed,
                (true, false) => NodeState::Available,
            },
            mastery,
            has_trainings,
        });
    }
    if nodes.is_empty() {
        return TreeLayout::default();
    }

    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Predecessors, by node index — only the ones actually placed.
    let predecessors: Vec<Vec<usize>> = nodes
        .iter()
        .map(|n| {
            graph
                .get(&n.id)
                .map(|g| {
                    g.prerequisites
                        .iter()
                        .filter_map(|p| index.get(p.as_str()).copied())
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (to, preds) in predecessors.iter().enumerate() {
        for &from in preds {
            successors[from].push(to);
        }
    }

    let column_count = nodes.iter().map(|n| n.column + 1).max().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); column_count];
    for (i, n) in nodes.iter().enumerate() {
        layers[n.column].push(i);
    }
    // Start from the catalogue's own order, so the result is deterministic
    // and — before any crossing reduction — already roughly the order a
    // reader met these lessons in.
    for layer in &mut layers {
        layer.sort_unstable();
    }

    order_layers(&mut layers, &predecessors, &successors);

    // Centre every layer against the tallest, so the tree hangs balanced
    // rather than pinned to the top with the root alone in the corner.
    let tallest = layers.iter().map(Vec::len).max().unwrap_or(0) as f32;
    for layer in &layers {
        let offset = (tallest - layer.len() as f32) / 2.0;
        for (row, &node) in layer.iter().enumerate() {
            nodes[node].row = offset + row as f32;
        }
    }

    let mut edges: Vec<Edge> = Vec::new();
    for (to, preds) in predecessors.iter().enumerate() {
        for &from in preds {
            edges.push(Edge {
                from: (nodes[from].column, nodes[from].row),
                to: (nodes[to].column, nodes[to].row),
            });
        }
    }
    edges.sort_by(|a, b| {
        (a.from.0, a.from.1, a.to.0, a.to.1)
            .partial_cmp(&(b.from.0, b.from.1, b.to.0, b.to.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    TreeLayout { nodes, edges }
}

/// Reorders each layer so edges cross as little as possible.
///
/// The standard barycentre heuristic: sweep down putting each node beside
/// the average position of what it depends on, then sweep back up putting
/// it beside the average of what depends on it, and repeat. It is not
/// optimal — minimising crossings exactly is NP-hard — but it is a few
/// lines and turns a tangle into something a reader can follow.
///
/// A node with nothing to average against (a root on the down sweep, a leaf
/// on the up) keeps where it is, which is what stops the sweeps fighting
/// each other.
fn order_layers(layers: &mut [Vec<usize>], predecessors: &[Vec<usize>], successors: &[Vec<usize>]) {
    let mut row_of: HashMap<usize, f32> = HashMap::new();
    for layer in layers.iter() {
        for (row, &node) in layer.iter().enumerate() {
            row_of.insert(node, row as f32);
        }
    }

    let barycentre = |node: usize, neighbours: &[usize], row_of: &HashMap<usize, f32>| {
        if neighbours.is_empty() {
            return row_of.get(&node).copied().unwrap_or(0.0);
        }
        neighbours
            .iter()
            .filter_map(|n| row_of.get(n).copied())
            .sum::<f32>()
            / neighbours.len() as f32
    };

    let resort =
        |layer: &mut Vec<usize>, neighbours: &[Vec<usize>], row_of: &mut HashMap<usize, f32>| {
            let mut keyed: Vec<(f32, usize)> = layer
                .iter()
                .map(|&n| (barycentre(n, &neighbours[n], row_of), n))
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            *layer = keyed.into_iter().map(|(_, n)| n).collect();
            for (row, &n) in layer.iter().enumerate() {
                row_of.insert(n, row as f32);
            }
        };

    for _ in 0..ORDERING_PASSES {
        // Down: every layer but the first, against what it depends on.
        for layer in layers.iter_mut().skip(1) {
            resort(layer, predecessors, &mut row_of);
        }
        // Back up: every layer but the last, against what depends on it.
        for layer in layers.iter_mut().rev().skip(1) {
            resort(layer, successors, &mut row_of);
        }
    }
}

#[cfg(test)]
mod tests;
