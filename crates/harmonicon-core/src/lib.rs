// SPDX-License-Identifier: MIT

//! Harmonicon's pure-logic core: no Bevy, no ECS, no I/O beyond a couple of
//! plain file writes.
//!
//! Everything here is the bottom of `docs/physical_design_plan.md`'s level
//! order — the music theory (`harmonica`, `chart`, `note_parser`), the
//! scoring maths (`scoring`), pitch/MIDI conversion (`midi`), and the
//! crash-safe config write (`config_file`). The game crate re-exports these
//! under their historical paths (`crate::chart`, `crate::scoring`,
//! `crate::midi`, ...), so call sites read exactly as before.
//!
//! Being a separate crate is what *enforces* the layering: Cargo cannot
//! express a circular dependency, so nothing here can ever reach back up
//! into gameplay, UI or asset loading.

pub mod chart;
pub mod config_file;
pub mod harmonica;
pub mod harmonica_constraints;
pub mod harp_remap;
pub mod midi;
/// Standard MIDI File parsing — named apart from [`midi`], which is pitch
/// and note-number conversion. Re-exported by the game as `song::midi`.
pub mod midi_file;
pub mod note_parser;
pub mod pitch_map;
pub mod scoring;
pub mod snap;
pub mod synth;
pub mod training;
pub mod wav;
