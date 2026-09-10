// SPDX-License-Identifier: MIT

//! Where every node of the skill tree goes, and what state it is in.
//!
//! Pure — no Bevy, no assets, no pixels. It answers "which column, which
//! row, locked or not, how much of the ladder is done", and the spawn code
//! beside it turns that into nodes. Same split the notation staff uses:
//! decisions here, translation there, so the decisions are testable.
//!
//! **Two levels, not one.** Across the top runs a spine of *unit* nodes —
//! Unit 1, Unit 2, … — each opening once enough of the one before it is
//! passed ([`harmonicon_song::lessons::units`]). Under each hangs that
//! unit's own lessons, laid out as their own small layered graph: column is
//! depth *within the unit*, row is chosen to keep edges untangled, and
//! track is colour rather than row.
//!
//! **That two-level shape is what makes the drawing readable**, and it is
//! the whole reason for it. Laid out as one flat graph, the curriculum's
//! eighteen cross-unit prerequisites became edges spanning four and five
//! columns, drawn straight through whatever nodes and labels lay between —
//! and no amount of crossing reduction helps, because the endpoints are
//! genuinely that far apart. Grouping by unit turns those eighteen into
//! four spine edges, and every remaining edge is local to one cluster.
//!
//! **A cross-unit prerequisite is therefore not drawn.** It is not lost:
//! [`PlacedNode::unmet`] carries whatever a locked lesson is still waiting
//! on, so the renderer can name it on the node itself rather than make the
//! player trace a line across the screen.
//!
//! **Grid coordinates, not pixels.** Node size and spacing are the
//! renderer's business and change with the theme; which node sits left of
//! which does not.

use std::collections::{HashMap, HashSet};

use harmonicon_app::profile::PlayerProfile;
use harmonicon_core::training::Tier;
use harmonicon_song::lessons::LessonEntry;
use harmonicon_song::lessons::graph::LessonGraph;
use harmonicon_song::lessons::units::UnitChain;

/// Crossing-reduction sweeps. Four down-and-up passes is well past the
/// point this curriculum stops improving; it is cheap and runs once.
const ORDERING_PASSES: usize = 4;

/// Blank columns between one unit's cluster and the next, so the units read
/// as separate groups rather than one continuous field of nodes.
const UNIT_GAP_COLUMNS: usize = 1;

/// The row the spine runs along. Everything else hangs below it, which is
/// what keeps the spine's own edges horizontal and crossing nothing.
const SPINE_ROW: f32 = 0.0;
/// The first row a lesson may occupy.
const CLUSTER_TOP_ROW: f32 = 1.0;

/// How a node reads at a glance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeState {
    /// A prerequisite — or the whole unit — is still shut.
    Locked,
    /// Playable now, not yet passed.
    Available,
    /// Passed. Its ladder may still have tiers left.
    Passed,
    /// Passed, and every training tier with it.
    Mastered,
}

/// One unit of the curriculum, drawn as a major node on the spine.
#[derive(Clone, PartialEq, Debug)]
pub struct PlacedUnit {
    pub id: String,
    /// Fluent key — `lesson-unit-<id>`, the key the curriculum already uses.
    pub title_key: String,
    pub column: usize,
    pub row: f32,
    /// Whether the player has reached this unit yet.
    pub locked: bool,
    /// Lessons passed inside it, and how many open the next unit. Shown as
    /// a count on the node: a gate the player can't see the terms of is
    /// just an obstacle.
    pub completed: usize,
    pub required: usize,
}

#[derive(Clone, PartialEq, Debug)]
pub struct PlacedNode {
    pub id: String,
    /// Unit whose expand/collapse control owns this node.
    pub unit_id: String,
    /// Fluent key — the tree never holds display text, same rule as the
    /// rest of the lesson UI.
    pub title_key: String,
    /// The track this lesson belongs to. Drawn as colour rather than as a
    /// row, so grouping survives without constraining position.
    pub track: String,
    /// Depth *within this lesson's unit*, offset by where that unit's
    /// cluster starts. A node always sits right of everything it depends on
    /// inside its own unit.
    pub column: usize,
    /// Vertical position, in node-heights. Fractional so a short layer can
    /// be centred against a tall one.
    pub row: f32,
    pub state: NodeState,
    /// 0..1, how much of the training ladder is passed.
    pub mastery: f32,
    /// Whether this lesson has trainings at all. A lesson with none draws
    /// no ring — there is nothing to fill, and an empty ring would read as
    /// "you have done none of it" rather than "there is none".
    pub has_trainings: bool,
    /// Title keys of the prerequisites still unpassed. Carries the
    /// cross-unit ones no longer drawn as edges, so a locked node can say
    /// what it wants instead of leaving the player to guess.
    pub unmet: Vec<String>,
}

/// What an edge means, which is also how it should be drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeKind {
    /// Unit to unit, along the spine.
    Spine,
    /// A unit down into its own lessons, or one lesson to another inside a
    /// unit.
    Branch,
}

/// An edge, in grid coordinates.
#[derive(Clone, PartialEq, Debug)]
pub struct Edge {
    pub from: (usize, f32),
    pub to: (usize, f32),
    pub kind: EdgeKind,
    /// Unit whose cluster owns this edge. Spine edges stay visible and have
    /// no owner.
    pub unit_id: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct TreeLayout {
    pub units: Vec<PlacedUnit>,
    pub nodes: Vec<PlacedNode>,
    pub edges: Vec<Edge>,
}

impl TreeLayout {
    /// Columns the canvas needs — one past the rightmost node.
    pub fn columns(&self) -> usize {
        let lessons = self.nodes.iter().map(|n| n.column + 1).max().unwrap_or(0);
        let units = self.units.iter().map(|u| u.column + 1).max().unwrap_or(0);
        lessons.max(units)
    }

    /// Rows the canvas needs, in node-heights.
    pub fn rows(&self) -> f32 {
        self.nodes
            .iter()
            .map(|n| n.row + 1.0)
            .chain(self.units.iter().map(|u| u.row + 1.0))
            .fold(0.0_f32, f32::max)
    }

    #[cfg(test)]
    pub fn node(&self, id: &str) -> Option<&PlacedNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    #[cfg(test)]
    pub fn unit(&self, id: &str) -> Option<&PlacedUnit> {
        self.units.iter().find(|u| u.id == id)
    }
}

/// Places every unit and every lesson, given what the player has done.
pub fn layout(
    entries: &[LessonEntry],
    graph: &LessonGraph,
    chain: &UnitChain,
    profile: &PlayerProfile,
) -> TreeLayout {
    let passed: HashSet<&str> = profile.passed_lesson_ids().into_iter().collect();
    let tiers = Tier::ALL.len();

    // Only lessons the catalogue can label. One in the graph but not here
    // is skipped rather than drawn as a blank node.
    let mut nodes: Vec<PlacedNode> = Vec::new();
    let mut unit_of_node: Vec<usize> = Vec::new();
    for entry in entries {
        let id = &entry.manifest.id;
        let (Some(graph_node), Some(unit)) = (graph.get(id), chain.unit_of(id)) else {
            continue;
        };
        let unit_open = chain.is_unlocked(unit, &passed);
        let unmet: Vec<&str> = graph_node
            .prerequisites
            .iter()
            .map(String::as_str)
            .filter(|p| !passed.contains(p))
            .collect();
        let is_passed = passed.contains(id.as_str());
        let has_trainings = entry.manifest.training.is_some();
        let mastery = if has_trainings {
            profile.mastery(id, tiers)
        } else {
            0.0
        };
        nodes.push(PlacedNode {
            id: id.clone(),
            unit_id: entry.manifest.unit.clone(),
            title_key: entry.manifest.title_key.clone(),
            track: graph_node.track.clone(),
            column: 0,
            row: 0.0,
            // A lesson needs both gates open: its own prerequisites, and
            // the unit holding it.
            state: match (unit_open && unmet.is_empty(), is_passed) {
                (false, false) => NodeState::Locked,
                (_, true) if has_trainings && mastery >= 1.0 => NodeState::Mastered,
                (_, true) => NodeState::Passed,
                (true, false) => NodeState::Available,
            },
            mastery,
            has_trainings,
            unmet: unmet
                .iter()
                .map(|p| title_key_of(entries, p).unwrap_or_else(|| (*p).to_string()))
                .collect(),
        });
        unit_of_node.push(unit);
    }
    if nodes.is_empty() {
        return TreeLayout::default();
    }

    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Adjacency, by node index, and **only within a unit** — a cross-unit
    // prerequisite is represented by the spine, not by an edge.
    let predecessors: Vec<Vec<usize>> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            graph
                .get(&n.id)
                .map(|g| {
                    g.prerequisites
                        .iter()
                        .filter_map(|p| index.get(p.as_str()).copied())
                        .filter(|&p| unit_of_node[p] == unit_of_node[i])
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

    let mut units: Vec<PlacedUnit> = Vec::new();
    let mut cursor = 0usize;
    for (ix, unit) in chain.units().iter().enumerate() {
        let members: Vec<usize> = (0..nodes.len())
            .filter(|&n| unit_of_node[n] == ix)
            .collect();
        if members.is_empty() {
            continue;
        }

        // Depth within the cluster. `graph.get(..).depth` is a valid
        // topological order over the whole curriculum, so walking members
        // in that order means every predecessor is resolved before its
        // dependent — no second sort needed.
        let mut by_depth = members.clone();
        by_depth.sort_by_key(|&n| (graph.get(&nodes[n].id).map(|g| g.depth).unwrap_or(0), n));
        let mut local: HashMap<usize, usize> = HashMap::new();
        for &n in &by_depth {
            let depth = predecessors[n]
                .iter()
                .filter_map(|p| local.get(p))
                .map(|d| d + 1)
                .max()
                .unwrap_or(0);
            local.insert(n, depth);
        }

        let width = local.values().map(|d| d + 1).max().unwrap_or(1);
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); width];
        for &n in &members {
            layers[local[&n]].push(n);
        }
        // Start from the catalogue's own order, so the result is
        // deterministic and — before any crossing reduction — already
        // roughly the order a reader met these lessons in.
        for layer in &mut layers {
            layer.sort_unstable();
        }
        order_layers(&mut layers, &predecessors, &successors);

        // Centre every layer against the tallest, so a cluster hangs
        // balanced rather than pinned to the top.
        let tallest = layers.iter().map(Vec::len).max().unwrap_or(0) as f32;
        for (column, layer) in layers.iter().enumerate() {
            let offset = (tallest - layer.len() as f32) / 2.0;
            for (row, &n) in layer.iter().enumerate() {
                nodes[n].column = cursor + column;
                nodes[n].row = CLUSTER_TOP_ROW + offset + row as f32;
            }
        }

        units.push(PlacedUnit {
            id: unit.id.clone(),
            title_key: unit.title_key.clone(),
            column: cursor,
            row: SPINE_ROW,
            locked: !chain.is_unlocked(ix, &passed),
            completed: chain.completed(ix, &passed),
            required: chain.required(ix),
        });
        cursor += width + UNIT_GAP_COLUMNS;
    }

    let mut edges: Vec<Edge> = Vec::new();
    for pair in units.windows(2) {
        edges.push(Edge {
            from: (pair[0].column, pair[0].row),
            to: (pair[1].column, pair[1].row),
            kind: EdgeKind::Spine,
            unit_id: None,
        });
    }
    for (to, preds) in predecessors.iter().enumerate() {
        if preds.is_empty() {
            // A cluster root hangs off its unit node — otherwise the first
            // column of every unit floats unattached to anything.
            if let Some(unit) = units
                .iter()
                .find(|u| u.id == chain.units()[unit_of_node[to]].id)
            {
                edges.push(Edge {
                    from: (unit.column, unit.row),
                    to: (nodes[to].column, nodes[to].row),
                    kind: EdgeKind::Branch,
                    unit_id: Some(unit.id.clone()),
                });
            }
            continue;
        }
        for &from in preds {
            edges.push(Edge {
                from: (nodes[from].column, nodes[from].row),
                to: (nodes[to].column, nodes[to].row),
                kind: EdgeKind::Branch,
                unit_id: Some(nodes[to].unit_id.clone()),
            });
        }
    }
    edges.sort_by(|a, b| {
        (a.from.0, a.from.1, a.to.0, a.to.1)
            .partial_cmp(&(b.from.0, b.from.1, b.to.0, b.to.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    TreeLayout {
        units,
        nodes,
        edges,
    }
}

/// A lesson's Fluent title key, for naming it somewhere other than its own
/// node.
fn title_key_of(entries: &[LessonEntry], id: &str) -> Option<String> {
    entries
        .iter()
        .find(|e| e.manifest.id == id)
        .map(|e| e.manifest.title_key.clone())
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
        let known: Vec<f32> = neighbours
            .iter()
            .filter_map(|n| row_of.get(n).copied())
            .collect();
        if known.is_empty() {
            return row_of.get(&node).copied().unwrap_or(0.0);
        }
        known.iter().sum::<f32>() / known.len() as f32
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
