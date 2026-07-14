// SPDX-License-Identifier: MIT

//! Harmonicon library crate.
//!
//! Houses every subsystem so they can be shared between the game binary
//! (`src/main.rs`) and the helper tools in `src/bin/` (e.g. `midi-to-chart`),
//! which are separate crates and can only reach this code through the library.

// Bevy systems routinely take one parameter per resource/component/message
// type they touch — that's the ECS, not a design smell — and `Query` filter
// tuples like `(With<A>, Without<B>, Without<C>)` are inherently "complex" by
// this lint's simple heuristic despite being completely idiomatic Bevy.
// Allowed crate-wide rather than annotating every system individually.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod assets_management;
pub mod audio_system;
pub mod dialogs;
pub mod gameplay;
pub mod jam_backing;
pub mod lessons;
pub mod localization;
pub mod menu;
pub mod profile;
pub mod scoring;
pub mod settings;
pub mod song;
pub mod spectrogram;
pub mod theme;
pub mod song_editor;
