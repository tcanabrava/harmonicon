// SPDX-License-Identifier: MIT

//! Gameplay: the scored song-playing experience (2D/3D/Jam Session), the
//! standalone Bending Trainer, and the results screen. This module is
//! wiring + re-exports only — see `plugin.rs` for the schedule,
//! `state.rs`/`notes.rs`/`bars.rs`/`clock.rs` for shared resources and pure
//! helpers, and `judge.rs`/`hud.rs`/`lifecycle.rs` for the scoring/HUD/
//! song-lifetime systems `plugin.rs` wires together.

mod adaptive_difficulty;
mod bars;
mod bending_trainer;
mod call_response;
mod clock;
pub mod countdown_overlay;
mod gameplay_2d;
mod gameplay_3d;
pub mod harmonica_overlay;
mod hud;
mod judge;
mod lifecycle;
pub mod metronome_overlay;
mod modifier_legend;
pub mod note_tail_2d;
mod note_tail_3d;
pub mod note_visual_2d;
mod notes;
mod pause_menu;
mod phrase_overlay;
mod plugin;
mod results;
pub mod song_progress_overlay;
mod state;
pub mod twelve_bar_blues_overlay;
mod wait_freeze_overlay;

pub use crate::scoring::{HitQuality, NoteOutcome, classify_note, compute_points, sustain_points};

pub use bars::*;
pub use clock::*;
pub use gameplay_2d::HIT_H_PCT;
pub use hud::*;
pub use judge::*;
pub use notes::*;
pub use plugin::*;
pub use state::*;

#[cfg(test)]
mod tests;
