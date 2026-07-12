// SPDX-License-Identifier: MIT

use bevy::prelude::*;

use crate::{
    menu::{AppState, SelectedSong},
    song::SongManifest,
    song::chart::{Action, Modifier, NoteEvent},
};

use super::{GameplayClock, Paused, resolve_item_time};

/// Live banner text showing the current phrase and groove from the chart.
#[derive(Component)]
pub struct PhraseText;

pub fn spawn_phrase_banner(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.80, 0.70, 0.95)),
        PhraseText,
    ));
}

/// Live tab-notation ribbon for the phrase currently active — see
/// [`tab_label`]/[`phrase_tab_sequence`]. Spawned as a sibling right below
/// [`PhraseText`] so the two "what's happening right now" readouts sit
/// together.
#[derive(Component)]
pub struct TabRibbonText;

pub fn spawn_tab_ribbon(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.90, 0.90, 0.75)),
        TabRibbonText,
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

/// Renders one note as conventional harmonica tab: `+N` for a blow, `-N` for
/// a draw, an apostrophe per semitone of bend depth (e.g. `-4''` for a
/// whole-step draw bend), an `o` suffix for an overblow/overdraw, or a `*`
/// suffix for a chromatic slide.
pub fn tab_label(hole: u8, is_blow: bool, modifiers: &[Modifier]) -> String {
    let mut s = format!("{}{hole}", if is_blow { "+" } else { "-" });
    for m in modifiers {
        match m {
            Modifier::Bend { semitones, .. } => {
                let depth = (semitones.abs().round() as usize).max(1);
                s.push_str(&"'".repeat(depth));
            }
            Modifier::Overblow | Modifier::Overdraw => s.push('o'),
            Modifier::Slide => s.push('*'),
            Modifier::Vibrato { .. } | Modifier::WahWah { .. } => {}
        }
    }
    s
}

/// The tab-notation sequence (space-separated [`tab_label`]s) for every note
/// event in the phrase currently active at `clock` — the same
/// phrase-boundary rule as [`active_phrase_groove`] (persists from the most
/// recent item that declared a phrase), but collecting *every* event from
/// the phrase's start up to the next phrase's start (or the end of the
/// track), not just the ones the clock has already reached — the point is
/// letting a player read the whole phrase ahead, like sheet music.
pub fn phrase_tab_sequence(items: &[(f64, Option<&str>, &[NoteEvent])], clock: f64) -> String {
    if clock < 0.0 {
        return String::new();
    }
    let start_idx = items
        .iter()
        .enumerate()
        .rfind(|(_, (t, p, _))| *t <= clock && p.is_some())
        .map(|(i, _)| i);
    let Some(start_idx) = start_idx else {
        return String::new();
    };
    let end_idx = items[start_idx + 1..]
        .iter()
        .position(|(_, p, _)| p.is_some())
        .map(|offset| start_idx + 1 + offset)
        .unwrap_or(items.len());

    items[start_idx..end_idx]
        .iter()
        .flat_map(|(_, _, events)| events.iter())
        .map(|e| {
            tab_label(
                e.hole,
                matches!(e.action, Action::Blow),
                e.modifiers.as_deref().unwrap_or(&[]),
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Times of track items that can move the banner ([`update_phrase`]) or the
/// tab ribbon ([`update_tab_ribbon`]) off their current label — precomputed
/// once at song start so [`watch_phrase_boundaries`] can locate the active
/// boundary with a `partition_point` instead of the two label systems
/// scanning (`update_phrase`) or fully re-collecting (`update_tab_ribbon`)
/// the whole chart track every frame.
#[derive(Resource, Default)]
struct PhraseBoundaries {
    /// Items that declare a phrase and/or a groove — the banner changes
    /// whenever either does.
    banner: Vec<f64>,
    /// Items that declare a phrase — the tab ribbon only tracks phrase
    /// boundaries, not groove ones.
    ribbon: Vec<f64>,
}

fn setup_phrase_boundaries(
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut boundaries: ResMut<PhraseBoundaries>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        *boundaries = PhraseBoundaries::default();
        return;
    };
    let chart = &manifest.chart;
    *boundaries = PhraseBoundaries {
        banner: chart
            .track
            .iter()
            .filter(|item| item.phrase.is_some() || item.groove.is_some())
            .map(|item| resolve_item_time(item, &chart.timing))
            .collect(),
        ribbon: chart
            .track
            .iter()
            .filter(|item| item.phrase.is_some())
            .map(|item| resolve_item_time(item, &chart.timing))
            .collect(),
    };
}

/// Emitted whenever the clock crosses a phrase/groove boundary, in either
/// direction — a `handle_loop_boundary` rewind crosses boundaries backward
/// and must re-emit too, which falls out for free here since the boundary
/// index is recomputed from the clock each frame (not advanced
/// incrementally), so a rewind simply produces a smaller index that still
/// compares unequal to the previous frame's.
#[derive(Message)]
struct PhraseChanged;

fn watch_phrase_boundaries(
    clock: Res<GameplayClock>,
    boundaries: Res<PhraseBoundaries>,
    mut last_banner: Local<Option<usize>>,
    mut last_ribbon: Local<Option<usize>>,
    mut changed: MessageWriter<PhraseChanged>,
) {
    let banner_idx = boundaries.banner.partition_point(|&t| t <= clock.get());
    let ribbon_idx = boundaries.ribbon.partition_point(|&t| t <= clock.get());
    let moved = *last_banner != Some(banner_idx) || *last_ribbon != Some(ribbon_idx);
    *last_banner = Some(banner_idx);
    *last_ribbon = Some(ribbon_idx);
    if moved {
        changed.write(PhraseChanged);
    }
}

fn update_tab_ribbon(
    mut changed: MessageReader<PhraseChanged>,
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut ribbon: Query<&mut Text, With<TabRibbonText>>,
) {
    if changed.read().count() == 0 {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let items: Vec<(f64, Option<&str>, &[NoteEvent])> = chart
        .track
        .iter()
        .map(|item| {
            (
                resolve_item_time(item, &chart.timing),
                item.phrase.as_deref(),
                item.events.as_slice(),
            )
        })
        .collect();
    let label = phrase_tab_sequence(&items, clock.get());

    for mut text in &mut ribbon {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn update_phrase(
    mut changed: MessageReader<PhraseChanged>,
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut banner: Query<&mut Text, With<PhraseText>>,
) {
    if changed.read().count() == 0 {
        return;
    }
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
        clock.get(),
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
        app.init_resource::<PhraseBoundaries>()
            .add_message::<PhraseChanged>()
            .add_systems(OnEnter(AppState::Playing), setup_phrase_boundaries)
            .add_systems(
                Update,
                (watch_phrase_boundaries, update_phrase, update_tab_ribbon)
                    .chain()
                    .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
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

    // ── tab_label ────────────────────────────────────────────────────────────

    #[test]
    fn tab_label_plain_blow_and_draw() {
        assert_eq!(tab_label(4, true, &[]), "+4");
        assert_eq!(tab_label(4, false, &[]), "-4");
    }

    #[test]
    fn tab_label_bend_depth_is_one_apostrophe_per_semitone() {
        let bend = |semitones| {
            vec![Modifier::Bend {
                semitones,
                intensity: None,
            }]
        };
        assert_eq!(tab_label(4, false, &bend(-1.0)), "-4'");
        assert_eq!(tab_label(4, false, &bend(-2.0)), "-4''");
        // Rounds to the nearest semitone rather than truncating.
        assert_eq!(tab_label(3, false, &bend(-1.5)), "-3''");
    }

    #[test]
    fn tab_label_overblow_and_overdraw_get_an_o_suffix() {
        assert_eq!(tab_label(5, true, &[Modifier::Overblow]), "+5o");
        assert_eq!(tab_label(7, false, &[Modifier::Overdraw]), "-7o");
    }

    #[test]
    fn tab_label_slide_gets_an_asterisk_suffix() {
        assert_eq!(tab_label(4, true, &[Modifier::Slide]), "+4*");
    }

    #[test]
    fn tab_label_ignores_vibrato_and_wah() {
        assert_eq!(
            tab_label(
                2,
                false,
                &[Modifier::Vibrato {
                    oscillation_hz: 5.0,
                    intensity: None
                }]
            ),
            "-2"
        );
    }

    // ── phrase_tab_sequence ────────────────────────────────────────────────────

    fn note_event(hole: u8, action: Action) -> NoteEvent {
        NoteEvent {
            hole,
            action,
            note: None,
            modifiers: None,
        }
    }

    #[test]
    fn phrase_tab_sequence_negative_clock_is_empty() {
        let e = [note_event(4, Action::Draw)];
        let items = [(0.0, Some("intro"), &e[..])];
        assert_eq!(phrase_tab_sequence(&items, -0.5), "");
    }

    #[test]
    fn phrase_tab_sequence_empty_before_any_phrase_starts() {
        let e = [note_event(4, Action::Draw)];
        let items = [(1.0, Some("intro"), &e[..])];
        assert_eq!(phrase_tab_sequence(&items, 0.5), "");
    }

    #[test]
    fn phrase_tab_sequence_includes_later_items_still_in_the_same_phrase() {
        // The whole phrase reads ahead, not just what's already played —
        // items after the phrase declaration but before the next one are
        // included even if the clock hasn't reached them yet.
        let e1 = [note_event(4, Action::Draw)];
        let e2 = [note_event(5, Action::Blow)];
        let e3 = [note_event(4, Action::Draw)];
        let items = [
            (0.0, Some("intro"), &e1[..]),
            (1.0, None, &e2[..]),
            (2.0, None, &e3[..]),
        ];
        assert_eq!(phrase_tab_sequence(&items, 0.5), "-4 +5 -4");
    }

    #[test]
    fn phrase_tab_sequence_stops_at_the_next_phrase() {
        let e1 = [note_event(4, Action::Draw)];
        let e2 = [note_event(5, Action::Blow)];
        let items = [
            (0.0, Some("intro"), &e1[..]),
            (2.0, Some("turnaround"), &e2[..]),
        ];
        assert_eq!(phrase_tab_sequence(&items, 0.5), "-4");
        assert_eq!(phrase_tab_sequence(&items, 2.5), "+5");
    }

    #[test]
    fn phrase_tab_sequence_formats_each_event_as_tab() {
        let e = [note_event(4, Action::Draw)];
        let mut with_bend = e.clone();
        with_bend[0].modifiers = Some(vec![Modifier::Bend {
            semitones: -1.0,
            intensity: None,
        }]);
        let items = [(0.0, Some("call"), &with_bend[..])];
        assert_eq!(phrase_tab_sequence(&items, 0.0), "-4'");
    }

    // ── watch_phrase_boundaries ─────────────────────────────────────────────

    #[derive(Resource, Default)]
    struct PhraseChangeLog(u32);

    fn log_phrase_changes(mut changed: MessageReader<PhraseChanged>, mut log: ResMut<PhraseChangeLog>) {
        log.0 += changed.read().count() as u32;
    }

    #[test]
    fn watch_phrase_boundaries_emits_only_on_a_boundary_crossing() {
        let mut world = World::new();
        world.insert_resource(PhraseBoundaries {
            banner: vec![0.0, 4.0],
            ribbon: vec![0.0, 2.0],
        });
        world.insert_resource(GameplayClock::new(-1.0));
        world.init_resource::<Messages<PhraseChanged>>();
        world.init_resource::<PhraseChangeLog>();

        let mut schedule = Schedule::default();
        schedule.add_systems((watch_phrase_boundaries, log_phrase_changes).chain());

        // First run always emits: `Local` state starts empty.
        schedule.run(&mut world);
        assert_eq!(world.resource::<PhraseChangeLog>().0, 1);

        // Crosses both the banner's and the ribbon's first boundary (t=0.0).
        world.resource_mut::<GameplayClock>().set_free(0.5);
        schedule.run(&mut world);
        assert_eq!(world.resource::<PhraseChangeLog>().0, 2);

        // No boundary crossed since the last run — no new emission.
        world.resource_mut::<GameplayClock>().set_free(1.0);
        schedule.run(&mut world);
        assert_eq!(world.resource::<PhraseChangeLog>().0, 2);

        // Crosses only the ribbon's boundary at t=2.0; still emits.
        world.resource_mut::<GameplayClock>().set_free(2.5);
        schedule.run(&mut world);
        assert_eq!(world.resource::<PhraseChangeLog>().0, 3);

        // A loop rewind moves the clock backward across the ribbon boundary
        // it just crossed — must re-emit, not stay silent because the
        // index is merely lower than before.
        world.resource_mut::<GameplayClock>().set_free(0.5);
        schedule.run(&mut world);
        assert_eq!(world.resource::<PhraseChangeLog>().0, 4);
    }
}
