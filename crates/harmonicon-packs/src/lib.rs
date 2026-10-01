// SPDX-License-Identifier: MIT

//! Lessons and songs as git repositories: the `pack.json` format and its
//! compatibility rules, which repositories are configured and installed, and
//! the shallow-clone transport that fetches them.
//!
//! No Bevy here, so all of it tests in seconds; see
//! `docs/content_packs_plan.md` for how the engine uses it.

#[cfg(not(target_arch = "wasm32"))]
pub mod git;
pub mod pack;
pub mod repo;
