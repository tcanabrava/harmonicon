// SPDX-License-Identifier: MIT

//! Harmonicon library crate.
//!
//! Houses every subsystem so they can be shared between the game binary
//! (`src/main.rs`) and the helper tools in `src/bin/` (e.g. `midi-to-chart`),
//! which are separate crates and can only reach this code through the library.

pub mod assets_management;
pub mod audio_system;
pub mod dialogs;
pub mod gameplay;
pub mod menu;
pub mod settings;
pub mod song;
pub mod spectrogram;
pub mod theme;
