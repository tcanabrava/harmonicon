// SPDX-License-Identifier: MIT

//! The Jam Session feature: the free-play screen and live hole-map feedback
//! ([`session`]), its improv-lesson scale-adherence accumulator
//! ([`improv`]), and the procedurally-generated 12-bar backing track
//! ([`backing`]) for jamming without picking an existing song.

pub mod backing;
pub mod improv;
pub mod session;
