// SPDX-License-Identifier: MIT

//! Lesson curriculum: manifest loading ([`manifest`]), discovery
//! ([`catalog`]), and unlock/pass judgment ([`progress`]). See
//! `docs/lessons_plan.md` for the full design.
//!
//! A lesson is a directory under `assets/lessons/<unit_dir>/<lesson_dir>/`
//! holding a `lesson.json` manifest (schema-validated against
//! `assets/lesson_schema.dtd.json`) and, for chart-backed lessons, a normal
//! song folder (`song/chart.harpchart` + `song/music.ogg` + artwork) that
//! plays through the ordinary gameplay pipeline — lessons deliberately add
//! no scoring machinery of their own, so they stay as honest as regular
//! play. Directory names give the menu order (`01_...`, `02_...`); the
//! manifest's `id` is the stable identity used for profile records and
//! prerequisites.
//!
//! All user-visible lesson text is localized: the manifest carries Fluent
//! *keys* (`title_key`/`body_key`, plus `lesson-unit-<unit>` for the unit
//! heading), never display strings.

mod catalog;
mod manifest;
mod progress;

pub use catalog::*;
pub use manifest::*;
pub use progress::*;
