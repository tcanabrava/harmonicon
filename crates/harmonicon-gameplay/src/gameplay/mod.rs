// SPDX-License-Identifier: MIT

//! Gameplay: the scored song-playing experience (2D/Jam Session), the
//! standalone Bending Trainer, and the results screen. This module is
//! wiring + re-exports only — see `plugin.rs` for the schedule,
//! `state.rs`/`notes.rs`/`bars.rs`/`clock.rs` for shared resources and pure
//! helpers, and `judge.rs`/`hud.rs`/`lifecycle.rs` for the scoring/HUD/
//! song-lifetime systems `plugin.rs` wires together.

mod adaptive_difficulty;
#[cfg(feature = "dev")]
pub mod autoplay;
mod bars;
mod beat_guides;
mod bending_trainer;
mod call_response;
mod clock;
pub mod coaching;
pub mod countdown_overlay;
mod gameplay_2d;
pub mod harmonica_overlay;
mod highway_2d;
mod hud;
mod hud_panel;
mod judge;
mod karaoke;
pub mod lifecycle;
pub mod metronome_overlay;
mod modifier_legend;
mod music_score_bridge;
pub mod note_feedback;
mod note_ribbon;
pub mod note_ribbon_2d;
mod notes;
pub mod pause_menu;
mod phrase_overlay;
pub mod plugin;
mod practice_badges;
mod results;
mod song_info;
pub mod song_progress_overlay;
mod song_waveform_material;
mod state;
mod technique_coach;
mod technique_cue;
pub mod twelve_bar_blues_overlay;
mod wait_freeze_overlay;
pub mod warning_banner;

pub use harmonicon_core::scoring::{
    HitQuality, NoteOutcome, classify_note, compute_points, sustain_points,
};

pub use adaptive_difficulty::AdaptiveDifficulty;
pub use adaptive_difficulty::carry_over_note_state;
pub use bars::*;
pub use clock::*;
pub use gameplay_2d::HIT_H_PCT;
pub use hud::*;
pub use judge::*;
pub use notes::*;
pub use plugin::*;
pub use song_info::*;
pub use state::*;

#[cfg(test)]
mod tests;
