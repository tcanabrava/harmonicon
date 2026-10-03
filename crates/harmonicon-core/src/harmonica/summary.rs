// SPDX-License-Identifier: MIT

//! A harmonica's one-line description, as parts a UI can word in the
//! player's language and as the English join used by logs and tests.

use super::{BendingProfile, Harmonica};

/// What [`Harmonica::summary`] returns: the parts of a one-line description,
/// left for the caller to word and join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarpSummary<'a> {
    pub chromatic: bool,
    pub holes: u8,
    /// As authored in the chart (`"2nd"`), when it declares one.
    pub position: Option<&'a str>,
    /// The diatonic tuning's name; `None` on a chromatic harp.
    pub profile: Option<&'static str>,
}

impl HarpSummary<'_> {
    /// The middle dot the parts are joined with, in every language.
    pub const SEPARATOR: &'static str = " \u{00B7} ";
}

impl Harmonica {
    /// The facts a one-line description of this harmonica is made of, for a
    /// UI to localize and join (`gameplay::song_info` does). `profile` is a
    /// tuning name — a proper noun, shown as-is in every language — and is
    /// `None` for a chromatic harp, which has no bending profile.
    pub fn summary(&self) -> HarpSummary<'_> {
        match self {
            Harmonica::Diatonic { holes, bending_profile, position, .. } => HarpSummary {
                chromatic: false,
                holes: *holes,
                position: position.as_deref(),
                profile: Some(match bending_profile {
                    BendingProfile::RichterStandard => "Richter",
                    BendingProfile::CountryTuned => "Country",
                    BendingProfile::PaddyRichter => "Paddy Richter",
                    BendingProfile::NaturalMinor => "Natural Minor",
                }),
            },
            Harmonica::Chromatic { holes, position, .. } => HarpSummary {
                chromatic: true,
                holes: *holes,
                position: position.as_deref(),
                profile: None,
            },
        }
    }

    /// [`summary`](Self::summary) joined in English —
    /// `"Diatonic · 10 holes · 2nd position · Richter"` — for logs and
    /// tests. The position segment appears only when the chart declares
    /// one: a chart without it used to print `? position`, which reads as a
    /// defect rather than an absence, and on a chromatic harp (fully
    /// chromatic, so "position" is mostly a diatonic idea) it's usually
    /// absent by design.
    pub fn display(&self) -> String {
        let s = self.summary();
        let mut parts = vec![
            if s.chromatic { "Chromatic" } else { "Diatonic" }.to_string(),
            format!("{} holes", s.holes),
        ];
        parts.extend(s.position.map(|p| format!("{p} position")));
        parts.extend(s.profile.map(str::to_string));
        parts.join(HarpSummary::SEPARATOR)
    }
}
