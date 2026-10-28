// SPDX-License-Identifier: MIT

//! Scored play: the audio-synced clock, note scheduling and judging, the
//! 2D highway, the shared HUD overlays, and the standalone Bending
//! Trainer.
//!
//! Owns the clock/bar vocabulary (`GameplayClock`, `CurrentBar`,
//! `AbsoluteBar`) and the overlay spawners that Jam Session and the Song
//! Editor build on, which is why it sits below both of them.

pub mod gameplay;
pub use gameplay::*;
