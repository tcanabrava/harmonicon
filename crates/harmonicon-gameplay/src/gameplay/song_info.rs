// SPDX-License-Identifier: MIT

//! What the HUD says about the *song* rather than about the performance:
//! title, key/tempo/meter, which harp to hold, the chart's description and
//! author.
//!
//! Resolved once at song setup into [`SongInfo`] and read from there by every
//! screen that shows it, rather than each one re-deriving it from the chart.
//!
//! **It is read material, not a readout.** None of it changes during a
//! performance, so it belongs where the player has time to read it — the
//! countdown and the pause menu — with only the title kept on screen while
//! notes are falling. See [`spawn_song_details`] and [`spawn_song_header`].

use bevy::prelude::*;

use harmonicon_core::chart::HarpChart;
use harmonicon_core::harmonica::{Harmonica, HarpSummary, Position, detected_harp_key};
use harmonicon_platform::localization::{Localization, LocalizationExt, enum_label_key};

/// Every song-describing string the HUD needs, resolved and localized once.
#[derive(Resource, Default, Clone)]
pub struct SongInfo {
    /// `"Artist — Title"`.
    pub title: String,
    /// `"Key: C ♩ = 80 3/4"`.
    pub meter: String,
    /// `"Diatonic · 10 holes · 1st position · Richter"`.
    pub harp: String,
    pub description: Option<String>,
    /// Already localized as `"Chart: {author}"`, not the bare name.
    pub chart_author: Option<String>,
}

impl SongInfo {
    /// `harp` is the one being played — the chart's own unless the player
    /// substituted one — since the line describes what to pick up.
    /// `key` is the key the music sounds in — `EffectiveHarmonica::
    /// song_key_for`, which differs from the chart's under a same-holes
    /// substitution.
    pub fn from_chart(chart: &HarpChart, harp: &Harmonica, key: &str, loc: &Localization) -> Self {
        Self {
            title: format!("{} \u{2014} {}", chart.song.artist, chart.song.title),
            meter: String::from(loc.msg_args(
                "gameplay-chart-info",
                &[
                    ("key", key.to_string()),
                    ("bpm", (chart.song.tempo_bpm as u32).to_string()),
                    ("time_sig", time_signature_label(chart)),
                ],
            )),
            harp: harp_line(&harp.summary(), loc),
            description: chart.metadata.as_ref().and_then(|m| m.description.clone()),
            chart_author: chart.metadata.as_ref().and_then(|m| {
                m.author.as_ref().map(|author| {
                    String::from(
                        loc.msg_args("gameplay-chart-author", &[("author", author.clone())]),
                    )
                })
            }),
        }
    }
}

/// The meter as the song info line shows it (`"6/8"`), from the same
/// [`chart_meter`](super::bars::chart_meter) reading every other part of
/// gameplay uses, so a chart whose `time_signature_map` overrides the song
/// field shows the meter it actually plays in.
pub(super) fn time_signature_label(chart: &HarpChart) -> String {
    let meter = super::bars::chart_meter(chart);
    format!("{}/{}", meter.numerator, meter.denominator)
}

/// A chart's position string (`"2nd"`) in the player's language when it is
/// one of the named [`Position`]s, else as authored — a chart may carry
/// wording the enum doesn't know, which is still worth showing.
pub fn position_label(raw: &str, loc: &Localization) -> String {
    match Position::from_label(raw) {
        Some(position) => loc.msg(&enum_label_key("position", position.label())).into(),
        None => raw.to_string(),
    }
}

/// `"Diatonic · 10 holes · 2nd position · Richter"`, worded in the player's
/// language: the kind and the hole/position phrases come from the locale,
/// the tuning name is a proper noun and stays as-is, and a segment the
/// chart doesn't declare is simply absent.
fn harp_line(summary: &HarpSummary<'_>, loc: &Localization) -> String {
    let mut parts = vec![
        String::from(loc.msg(if summary.chromatic {
            "harp-summary-chromatic"
        } else {
            "harp-summary-diatonic"
        })),
        String::from(loc.msg_args("harp-summary-holes", &[("n", summary.holes.to_string())])),
    ];
    parts.extend(summary.position.map(|p| {
        String::from(loc.msg_args("harp-summary-position", &[("pos", position_label(p, loc))]))
    }));
    parts.extend(summary.profile.map(str::to_string));
    parts.join(HarpSummary::SEPARATOR)
}

/// The one-line "which harp to grab" hint, in the player's language —
/// `"Use a C harmonica · 2nd position · key of G"` — the localized twin of
/// `harmonicon_core::harmonica::harp_banner`, which keeps the English join
/// for logs and tests. Same derivation: a Richter harp's key is its hole-1
/// blow note, paired with the chart's declared position (when it has one)
/// and the song's key; just the key when the harp's can't be determined.
pub fn harp_banner_text(harp: &Harmonica, song_key: &str, loc: &Localization) -> String {
    let Some(harp_key) = detected_harp_key(harp) else {
        return String::from(
            loc.msg_args("harp-banner-fallback", &[("key", song_key.to_string())]),
        );
    };
    let mut parts = vec![String::from(loc.msg_args("harp-banner-use", &[("key", harp_key)]))];
    parts.extend(harp.position().map(|p| {
        String::from(loc.msg_args("harp-summary-position", &[("pos", position_label(p, loc))]))
    }));
    parts.push(String::from(loc.msg_args("harp-banner-key", &[("key", song_key.to_string())])));
    parts.join("  \u{00B7}  ")
}

/// The title alone, for the strip that stays up while notes are falling.
///
/// Everything else in [`SongInfo`] is deliberately absent here: a player
/// mid-phrase is not reading a description, and the space is worth more as
/// highway.
pub fn spawn_song_header(parent: &mut ChildSpawnerCommands, info: &SongInfo) {
    parent.spawn_empty().apply_scene(bsn! {
        Text({info.title.clone()})
        TextFont { font_size: {FontSize::Px(16.0)} }
        TextColor({Color::srgb(0.82, 0.84, 0.92)})
    });
}

/// The whole block, for the two moments the player is not playing: the
/// countdown before the first note, and the pause menu.
pub fn spawn_song_details(parent: &mut ChildSpawnerCommands, info: &SongInfo) {
    parent
        .spawn_empty()
        .apply_scene(bsn! {
            Node {
                flex_direction: {FlexDirection::Column},
                row_gap: {Val::Px(3.0)},
                max_width: {Val::Px(560.0)},
            }
        })
        .with_children(|col| {
            for (text, size, color) in [
                (Some(&info.title), 20.0, Color::WHITE),
                (Some(&info.meter), 15.0, Color::srgb(0.60, 0.65, 0.75)),
                (Some(&info.harp), 15.0, Color::srgb(0.45, 0.72, 0.55)),
                (info.description.as_ref(), 15.0, Color::srgb(0.50, 0.50, 0.55)),
                (info.chart_author.as_ref(), 15.0, Color::srgb(0.40, 0.40, 0.45)),
            ] {
                let Some(text) = text else { continue };
                col.spawn_empty().apply_scene(bsn! {
                    Text({text.clone()})
                    TextFont { font_size: {FontSize::Px(size)} }
                    TextColor({color})
                });
            }
        });
}
