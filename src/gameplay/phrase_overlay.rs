// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use crate::{
    menu::{AppState, SelectedSong},
    song::SongManifest,
};

use super::{GameplayClock, Paused, resolve_item_time};

/// Live banner text showing the current phrase and groove from the chart.
#[derive(Component)]
pub struct PhraseText;

pub fn spawn_phrase_banner(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgb(0.80, 0.70, 0.95)),
        PhraseText,
    ));
}

/// Selects the phrase/groove in effect at `clock` from a time-ordered stream of
/// `(start_time, phrase, groove)`. The phrase persists from the most recent item
/// that declared one; the groove follows the most recent item. Returns nothing
/// before the song starts (`clock < 0`).
pub fn active_phrase_groove<'a>(
    items: impl Iterator<Item = (f64, Option<&'a str>, Option<&'a str>)>,
    clock: f64,
) -> (Option<&'a str>, Option<&'a str>) {
    if clock < 0.0 {
        return (None, None);
    }
    let mut phrase = None;
    let mut groove = None;
    for (t, p, g) in items {
        if t > clock {
            break; // track is time-ordered; nothing later has started yet
        }
        if p.is_some() {
            phrase = p;
        }
        if g.is_some() {
            groove = g;
        }
    }
    (phrase, groove)
}

/// Renders the banner text for a phrase/groove pair. Underscores in phrase names
/// become spaces so chart ids like `boom_shuffle` read naturally.
pub fn format_phrase_label(phrase: Option<&str>, groove: Option<&str>) -> String {
    match (phrase, groove) {
        (Some(p), Some(g)) => format!("\u{266A} {}  \u{00B7}  {}", p.replace('_', " "), g),
        (Some(p), None) => format!("\u{266A} {}", p.replace('_', " ")),
        (None, Some(g)) => format!("\u{266A} {g}"),
        (None, None) => String::new(),
    }
}

pub fn update_phrase(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut banner: Query<&mut Text, With<PhraseText>>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;

    let (phrase, groove) = active_phrase_groove(
        chart.track.iter().map(|item| {
            (
                resolve_item_time(item, &chart.timing),
                item.phrase.as_deref(),
                item.groove.as_deref(),
            )
        }),
        clock.0,
    );
    let label = format_phrase_label(phrase, groove);

    for mut text in &mut banner {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

pub struct PhrasePlugin;

impl Plugin for PhrasePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_phrase.run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<(f64, Option<&'static str>, Option<&'static str>)> {
        vec![
            (0.0, Some("intro"), Some("shuffle")),
            (1.0, None, Some("shuffle")),
            (2.0, Some("turnaround"), Some("straight")),
            (3.0, None, None),
        ]
    }

    #[test]
    fn negative_clock_is_empty() {
        let (p, g) = active_phrase_groove(items().into_iter(), -0.5);
        assert_eq!((p, g), (None, None));
    }

    #[test]
    fn before_first_item_is_empty() {
        // clock is past 0 but the first item starts exactly at 0, so it counts
        let (p, g) = active_phrase_groove(items().into_iter(), 0.0);
        assert_eq!((p, g), (Some("intro"), Some("shuffle")));
    }

    #[test]
    fn phrase_persists_when_later_item_omits_it() {
        // at t=1.5 the active item (t=1.0) has no phrase; keep "intro"
        let (p, g) = active_phrase_groove(items().into_iter(), 1.5);
        assert_eq!(p, Some("intro"));
        assert_eq!(g, Some("shuffle"));
    }

    #[test]
    fn phrase_and_groove_update_to_latest() {
        let (p, g) = active_phrase_groove(items().into_iter(), 2.5);
        assert_eq!(p, Some("turnaround"));
        assert_eq!(g, Some("straight"));
    }

    #[test]
    fn groove_persists_when_later_item_omits_it() {
        // item at t=3.0 has neither; both stick to the last known values
        let (p, g) = active_phrase_groove(items().into_iter(), 3.0);
        assert_eq!(p, Some("turnaround"));
        assert_eq!(g, Some("straight"));
    }

    #[test]
    fn label_combines_phrase_and_groove() {
        assert_eq!(
            format_phrase_label(Some("boom_shuffle"), Some("shuffle")),
            "\u{266A} boom shuffle  \u{00B7}  shuffle"
        );
    }

    #[test]
    fn label_phrase_only() {
        assert_eq!(
            format_phrase_label(Some("call_high"), None),
            "\u{266A} call high"
        );
    }

    #[test]
    fn label_groove_only() {
        assert_eq!(
            format_phrase_label(None, Some("shuffle")),
            "\u{266A} shuffle"
        );
    }

    #[test]
    fn label_empty_when_nothing_active() {
        assert_eq!(format_phrase_label(None, None), "");
    }
}
