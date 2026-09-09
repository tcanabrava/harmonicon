// SPDX-License-Identifier: MIT

//! Where every node of the skill tree goes, and what state it is in.
//!
//! Pure — no Bevy, no assets, no pixels. It answers "which row, which
//! column, locked or not, how much of the ladder is done", and the spawn
//! code beside it turns that into nodes. Same split the notation staff
//! uses: decisions here, translation there, so the decisions are testable.
//!
//! **Grid coordinates, not pixels.** Node size and spacing are the
//! renderer's business and change with the theme; which node sits left of
//! which does not.

use std::collections::HashSet;

use harmonicon_app::profile::PlayerProfile;
use harmonicon_core::training::Tier;
use harmonicon_song::lessons::LessonEntry;
use harmonicon_song::lessons::graph::LessonGraph;

/// How a node reads at a glance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeState {
    /// A prerequisite is still unmet. Desaturated, padlocked.
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
    /// Absolute row, counting sub-rows across every track above this one.
    pub row: usize,
    /// **The lesson's graph depth**, not its position in the track.
    ///
    /// This is what makes the drawing a tree rather than a grid: a node
    /// always sits to the right of everything it depends on, tracks start
    /// at different columns, and a track with nothing at some depth leaves
    /// a visible gap. Placing by position-within-track instead put every
    /// row in lockstep from column 0 and read as a lattice.
    pub column: usize,
    pub state: NodeState,
    /// 0..1, how much of the training ladder is passed. Drives the ring
    /// drawn around the node's art.
    pub mastery: f32,
    /// Whether this lesson has trainings at all. A lesson with none draws
    /// no ring — there is nothing to fill, and an empty ring would read as
    /// "you have done none of it" rather than "there is none".
    pub has_trainings: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct PlacedRow {
    /// Fluent key is `lesson-track-<track>`; this is the bare track id.
    pub track: String,
    /// Absolute row this track starts at, and how many it occupies — a
    /// track needs more than one whenever it has several lessons at the
    /// same depth (`tone` has three at depth 1). The label is centred
    /// across them.
    pub first_row: usize,
    pub height: usize,
    pub nodes: Vec<PlacedNode>,
}

/// A prerequisite edge, in grid coordinates.
///
/// Only edges the drawing actually needs: both ends are placed nodes, and
/// an edge inside one row is kept as much as one crossing rows — a row is
/// a family, not a chain, so neighbours are *not* implicitly connected and
/// the real edge has to be drawn even when it is horizontal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Edge {
    pub from: (usize, usize),
    pub to: (usize, usize),
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct TreeLayout {
    pub rows: Vec<PlacedRow>,
    pub edges: Vec<Edge>,
}

impl TreeLayout {
    /// Columns the canvas needs — one past the deepest node, since a
    /// column *is* a depth and tracks leave gaps rather than packing left.
    pub fn columns(&self) -> usize {
        self.rows
            .iter()
            .flat_map(|r| &r.nodes)
            .map(|n| n.column + 1)
            .max()
            .unwrap_or(0)
    }

    /// Rows the canvas needs, counting the sub-rows a track occupies when
    /// it has several lessons at one depth.
    pub fn height(&self) -> usize {
        self.rows
            .iter()
            .map(|r| r.first_row + r.height)
            .max()
            .unwrap_or(0)
    }

    /// Lookup by id. Only the tests need this — the renderer walks
    /// [`rows`](Self::rows) in order — so it isn't compiled into the game.
    #[cfg(test)]
    pub fn node(&self, id: &str) -> Option<&PlacedNode> {
        self.rows.iter().flat_map(|r| &r.nodes).find(|n| n.id == id)
    }
}

/// Places every lesson, given what the player has done.
///
/// Rows come from [`LessonGraph::rows`] — tracks ordered by how early they
/// can be started, members by depth — so the tree reads top-down through
/// the curriculum with foundations first.
pub fn layout(entries: &[LessonEntry], graph: &LessonGraph, profile: &PlayerProfile) -> TreeLayout {
    let passed: HashSet<&str> = profile.passed_lesson_ids().into_iter().collect();
    let tiers = Tier::ALL.len();

    // Catalogue position of each track's earliest lesson. Tracks are drawn
    // in that order rather than by size: `assets/lessons` is named
    // `01_blowing/01_single_note`, so catalogue order *is* teaching order,
    // and ordering by track length put `form` above `tone` when
    // `single-note` is where the curriculum actually begins.
    let mut track_rank: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        track_rank
            .entry(e.manifest.track())
            .and_modify(|r| *r = (*r).min(i))
            .or_insert(i);
    }

    let mut graph_rows = graph.rows();
    graph_rows.sort_by_key(|r| {
        track_rank
            .get(r.track.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });

    let mut rows = Vec::new();
    let mut at: std::collections::HashMap<&str, (usize, usize)> = std::collections::HashMap::new();
    let mut next_row = 0usize;

    for row in &graph_rows {
        // Same-depth siblings within a track can't share a cell, so they
        // stack into sub-rows: `tone` has three lessons at depth 1.
        let mut used: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let mut nodes = Vec::new();

        for id in &row.lessons {
            let Some(entry) = entries.iter().find(|e| &e.manifest.id == id) else {
                // In the graph but not the catalogue: skip rather than
                // place a node with no title to draw.
                continue;
            };
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

            let state = match (unlocked, is_passed) {
                (false, false) => NodeState::Locked,
                (_, true) if has_trainings && mastery >= 1.0 => NodeState::Mastered,
                (_, true) => NodeState::Passed,
                (true, false) => NodeState::Available,
            };

            let column = graph_node.depth;
            let sub = used.entry(column).or_insert(0);
            let row_ix = next_row + *sub;
            *sub += 1;

            at.insert(id.as_str(), (row_ix, column));
            nodes.push(PlacedNode {
                id: id.clone(),
                title_key: entry.manifest.title_key.clone(),
                row: row_ix,
                column,
                state,
                mastery,
                has_trainings,
            });
        }

        let height = used.values().copied().max().unwrap_or(0).max(1);
        rows.push(PlacedRow {
            track: row.track.clone(),
            first_row: next_row,
            height,
            nodes,
        });
        next_row += height;
    }

    let mut edges = Vec::new();
    for row in &rows {
        for node in &row.nodes {
            let Some(graph_node) = graph.get(&node.id) else {
                continue;
            };
            for prereq in &graph_node.prerequisites {
                if let Some(&from) = at.get(prereq.as_str()) {
                    edges.push(Edge {
                        from,
                        to: (node.row, node.column),
                    });
                }
            }
        }
    }
    // `at` holds (row, column); both ends are absolute, so an edge is
    // drawable without knowing which track either end belongs to.
    // Stable order so the drawing does not reshuffle between frames.
    edges.sort_by_key(|e| (e.from, e.to));

    TreeLayout { rows, edges }
}

#[cfg(test)]
mod tests;
