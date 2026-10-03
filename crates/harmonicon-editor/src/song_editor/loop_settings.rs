// SPDX-License-Identifier: MIT

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LoopSettings {
    pub(super) kind: String,
    pub(super) repeat: String,
    pub(super) start: String,
    pub(super) end: String,
}

impl Default for LoopSettings {
    fn default() -> Self {
        Self { kind: "full".into(), repeat: "no".into(), start: "0".into(), end: "last".into() }
    }
}

pub(super) const LOOP_TYPES: [&str; 6] = ["full", "intro", "verse", "chorus", "bridge", "outro"];
