// SPDX-License-Identifier: MIT

//! One file per top-level menu page. `mod.rs` here is mod declarations
//! only — each page's setup/update systems are wired into the schedule by
//! `menu::MenuPlugin`, and each page's own state (if any) stays private to
//! its file unless another page or `menu::routing` needs it.

pub(crate) mod artist_list;
pub(crate) mod calibration;
pub(crate) mod credits;
pub(crate) mod help_about;
pub(crate) mod jam_generate;
pub(crate) mod jam_session;
pub(crate) mod lessons;
pub(crate) mod main;
pub(crate) mod mode_select;
pub(crate) mod options;
pub(crate) mod play;
pub(crate) mod song_list;
pub(crate) mod theme_picker;
pub(crate) mod tutorial;
