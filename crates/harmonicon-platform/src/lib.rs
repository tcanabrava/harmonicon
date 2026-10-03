// SPDX-License-Identifier: MIT

//! Everything the game needs from the machine it runs on, below any
//! gameplay concept: asset discovery ([`assets_management`]), translated
//! strings ([`localization`]), persisted preferences ([`settings`]), the
//! visual theme ([`theme`]), the narrow-window breakpoint ([`responsive`])
//! and which lesson/song packs are installed ([`content_packs`]).
//!
//! Depends only on `harmonicon-core`, `harmonicon-audio` and
//! `harmonicon-packs`. Nothing here knows what a song, a note or a screen
//! is.

pub mod assets_management;
pub mod calendar;
pub mod content_packs;
#[cfg(not(target_arch = "wasm32"))]
pub mod content_sync;
pub mod localization;
pub mod paths;
pub mod responsive;
pub mod settings;
pub mod theme;

pub mod song_library;
