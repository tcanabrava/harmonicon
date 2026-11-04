// SPDX-License-Identifier: MIT
//! A scrollable full score with a beat-aligned practice selection band.
use super::pause_menu::{PauseMenuRoot, SelectedPhraseIndex};
use super::{LoopConfig, Paused, adaptive_difficulty::AdaptiveDifficulty};
use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, ScrollPosition};
use harmonicon_app::app::{GameplayMode, SelectedSong};
use harmonicon_core::chart::seconds_to_tick;
#[cfg(test)]
use harmonicon_core::chart::tick_to_seconds;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::song::SongManifest;
use harmonicon_ui::music_score::{
    self, BravuraFont, MusicScoreBarMap, MusicScoreNotes, MusicScorePlayhead, MusicScoreSpacing,
};

#[derive(Component)]
pub(super) struct Mounted;
#[derive(Component)]
pub(super) struct Content;
#[derive(Component)]
pub(super) struct Viewport;
#[derive(Component)]
pub(super) struct Playhead;
#[derive(Component)]
pub(super) struct Selection;
#[derive(Component)]
pub(super) struct SelectionSummary;
#[derive(Component)]
pub(super) struct Band;
#[derive(Component)]
pub(super) struct Zone {
    index: usize,
    start: f64,
    end: f64,
}

fn time_beat(seconds: f64, chart: &harmonicon_core::chart::HarpChart) -> f64 {
    let timing = &chart.timing;
    (seconds_to_tick(seconds.max(0.0), timing.resolution, &timing.tempo_map)
        + super::bars::pickup_lead_ticks(chart)) as f64
        / timing.resolution.max(1) as f64
}
#[cfg(test)]
fn beat_time(beat: f64, chart: &harmonicon_core::chart::HarpChart) -> f64 {
    let tick = (beat.max(0.0) * chart.timing.resolution.max(1) as f64).round() as u64;
    tick_to_seconds(
        tick.saturating_sub(super::bars::pickup_lead_ticks(chart)),
        chart.timing.resolution,
        &chart.timing.tempo_map,
    )
}
#[cfg(test)]
fn cursor_beat(normalized: Option<Vec2>, width: f32, spacing: f32) -> Option<f64> {
    // Bevy's node coordinates are centred: -0.5 is the left edge,
    // +0.5 the right edge. Convert to a left-based score coordinate.
    let x = (normalized?.x + 0.5) * width;
    Some(((x - music_score::score_beat_x(0.0, spacing)) / spacing.max(1.0)).max(0.0) as f64)
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct ScoreData<'w> {
    selected: Res<'w, SelectedSong>,
    manifests: Res<'w, Assets<SongManifest>>,
    bravura: Option<Res<'w, BravuraFont>>,
    notes: Res<'w, MusicScoreNotes>,
    spacing: Res<'w, MusicScoreSpacing>,
    playhead: Res<'w, MusicScorePlayhead>,
    adaptive: Res<'w, AdaptiveDifficulty>,
    loops: Res<'w, LoopConfig>,
    loc: Res<'w, Localization>,
    bars: Res<'w, MusicScoreBarMap>,
    selected_phrase: Res<'w, SelectedPhraseIndex>,
}

pub(super) fn update_pause_score(
    mut commands: Commands,
    paused: Res<Paused>,
    mode: Res<GameplayMode>,
    data: ScoreData,
    roots: Query<Entity, (With<PauseMenuRoot>, Without<Mounted>)>,
    mut positions: ParamSet<(
        Query<&mut Node, With<Content>>,
        Query<&mut Node, With<Playhead>>,
        Query<&mut Node, With<Selection>>,
    )>,
    mut viewports: Query<(Ref<ComputedNode>, &mut ScrollPosition), With<Viewport>>,
    mut was_paused: Local<bool>,
    mut viewport_width: Local<f32>,
    mut summaries: Query<&mut Text, With<SelectionSummary>>,
) {
    let ScoreData {
        selected,
        manifests,
        bravura,
        notes,
        spacing,
        playhead,
        adaptive,
        loops,
        loc,
        bars,
        selected_phrase,
    } = data;
    if *mode != GameplayMode::Play2D {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let chart = &manifest.chart;
    let end = adaptive
        .sections
        .iter()
        .map(|section| time_beat(section.end_time, chart))
        .chain(notes.0.iter().map(|n| n.start_beat + n.duration_beats))
        .fold(1.0, f64::max);
    let width = music_score::score_beat_x(end, spacing.0) + 80.0;
    let grand =
        matches!(chart.instrument(), Some(harmonicon_core::instrument::Instrument::Piano(_)));
    let score_height = music_score::PANEL_HEIGHT * if grand { 2.0 } else { 1.0 };
    if paused.0
        && let Some(bravura) = bravura
    {
        for root in &roots {
            commands.entity(root).insert(Mounted);
            let panel = commands
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(220.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .id();
            commands.entity(root).insert_children(0, &[panel]);
            let mut viewport = Entity::PLACEHOLDER;
            commands.entity(panel).with_children(|p| {
                p.spawn((
                    Text::new(loc.msg("pause-score-instructions").to_string()),
                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                    TextColor(Color::WHITE),
                ));
                p.spawn((
                    SelectionSummary,
                    Text::new(loc.msg("pause-score-no-selection").to_string()),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(Color::srgb(0.65, 0.82, 1.0)),
                ));
                viewport = harmonicon_ui::dialogs::scroll_area::spawn_scroll_area_xy(
                    p,
                    Color::srgb(0.55, 0.60, 0.7),
                    Color::srgb(0.12, 0.14, 0.18),
                );
            });
            commands
                .entity(viewport)
                .insert((Viewport, BackgroundColor(Color::srgb(0.06, 0.07, 0.09))))
                .with_children(|p| {
                    p.spawn((
                        Content,
                        Node {
                            width: Val::Px(width),
                            flex_shrink: 0.0,
                            height: Val::Px(score_height + 32.0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                    ))
                    .with_children(|content| {
                        content
                            .spawn((
                                Band,
                                RelativeCursorPosition::default(),
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(32.0),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.11, 0.13, 0.18)),
                            ))
                            .with_children(|band| {
                                for (index, section) in adaptive.sections.iter().enumerate() {
                                    let left = music_score::score_beat_x(
                                        time_beat(section.start_time, chart),
                                        spacing.0,
                                    );
                                    let right = music_score::score_beat_x(
                                        time_beat(section.end_time, chart),
                                        spacing.0,
                                    );
                                    band.spawn((
                                        bevy::ui_widgets::Button,
                                        bevy::input_focus::tab_navigation::TabIndex(0),
                                        Zone {
                                            index,
                                            start: section.start_time,
                                            end: section.end_time,
                                        },
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(left),
                                            width: Val::Px((right - left).max(1.0)),
                                            height: Val::Percent(100.0),
                                            border: UiRect::all(Val::Px(1.0)),
                                            overflow: Overflow::clip(),
                                            ..default()
                                        },
                                        BorderColor::all(Color::srgb(0.45, 0.5, 0.6)),
                                        BackgroundColor(Color::srgb(0.16, 0.21, 0.28)),
                                    ))
                                    .observe(select_zone)
                                    .with_children(|p| {
                                        p.spawn((
                                            Text::new((index + 1).to_string()),
                                            TextLayout::no_wrap(),
                                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                                            TextColor(Color::WHITE),
                                            Pickable::IGNORE,
                                        ));
                                    });
                                }
                            });
                        music_score::spawn_full_score(content, &bravura, grand, end);
                        content
                            .spawn((
                                Selection,
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Px(0.0),
                                    height: Val::Percent(100.0),
                                    display: Display::None,
                                    border: UiRect::horizontal(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.3, 0.65, 1.0, 0.06)),
                                BorderColor::all(Color::srgb(0.4, 0.75, 1.0)),
                                Pickable::IGNORE,
                            ))
                            .with_children(|p| {
                                for (right, text) in [(false, "A"), (true, "B")] {
                                    p.spawn((
                                        Text::new(text),
                                        TextFont { font_size: FontSize::Px(13.0), ..default() },
                                        TextColor(Color::srgb(0.4, 0.75, 1.0)),
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: if right { Val::Auto } else { Val::Px(2.0) },
                                            right: if right { Val::Px(2.0) } else { Val::Auto },
                                            top: Val::Px(0.0),
                                            ..default()
                                        },
                                        Pickable::IGNORE,
                                    ));
                                }
                            });
                        content.spawn((
                            Playhead,
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(0.0),
                                height: Val::Percent(100.0),
                                width: Val::Px(3.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(1.0, 0.15, 0.18)),
                            Pickable::IGNORE,
                        ));
                    });
                });
        }
    }
    if !paused.0 {
        *was_paused = false;
        return;
    }
    let x = music_score::score_beat_x(playhead.0, spacing.0);
    for mut node in &mut positions.p0() {
        node.width = Val::Px(width);
    }
    for mut node in &mut positions.p1() {
        node.left = Val::Px(x);
    }
    let section = adaptive.sections.get(selected_phrase.0);
    let range = section
        .map(|section| (time_beat(section.start_time, chart), time_beat(section.end_time, chart)));
    for mut text in &mut summaries {
        text.0 = if let Some(section) = section {
            let start_tick = seconds_to_tick(
                section.start_time,
                chart.timing.resolution,
                &chart.timing.tempo_map,
            );
            let end_tick =
                seconds_to_tick(section.end_time, chart.timing.resolution, &chart.timing.tempo_map)
                    .saturating_sub(1);
            let number = |tick| {
                bars.meter
                    .bar_number(bars.meter.position(tick).bar)
                    .map(|bar| bar.to_string())
                    .unwrap_or_else(|| loc.msg("pause-score-pickup").to_string())
            };
            loc.msg_args(
                "pause-score-section",
                &[
                    ("name", section.name.clone()),
                    ("first", number(start_tick)),
                    ("last", number(end_tick)),
                    ("start", format!("{:.1}", section.start_time)),
                    ("end", format!("{:.1}", section.end_time)),
                    (
                        "state",
                        loc.msg(if loops.active { "pause-repeat-on" } else { "pause-repeat-off" })
                            .to_string(),
                    ),
                ],
            )
            .to_string()
        } else {
            loc.msg("pause-score-no-selection").to_string()
        };
    }
    for mut node in &mut positions.p2() {
        node.display = if range.is_some() { Display::Flex } else { Display::None };
        if let Some((a, b)) = range {
            let left = music_score::score_beat_x(a, spacing.0);
            node.left = Val::Px(left);
            node.width = Val::Px(music_score::score_beat_x(b, spacing.0) - left);
        }
    }
    let current_width = viewports
        .iter()
        .next()
        .map(|(node, _)| node.size().x * node.inverse_scale_factor())
        .unwrap_or(0.0);
    if !*was_paused || current_width != *viewport_width {
        for (node, mut scroll) in &mut viewports {
            let visible = node.size().x * node.inverse_scale_factor();
            scroll.x = (x - visible * 0.35).clamp(0.0, (width - visible).max(0.0));
        }
    }
    *viewport_width = current_width;
    *was_paused = true;
}

fn select_zone(
    ev: On<bevy::ui_widgets::Activate>,
    zones: Query<&Zone>,
    mut selected: ResMut<SelectedPhraseIndex>,
    mut loops: ResMut<LoopConfig>,
) {
    if let Ok(zone) = zones.get(ev.entity) {
        selected.0 = zone.index;
        if loops.active {
            loops.start_time = zone.start;
            loops.end_time = zone.end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_band_uses_the_same_beat_origin_as_notation() {
        let width = 1000.0;
        let spacing = 40.0;
        let x = music_score::score_beat_x(4.0, spacing);
        assert!(
            (cursor_beat(Some(Vec2::new(x / width - 0.5, 0.0)), width, spacing).unwrap() - 4.0)
                .abs()
                < 0.001
        );
        assert_eq!(cursor_beat(Some(Vec2::new(-0.5, 0.0)), width, spacing), Some(0.0));
        assert_eq!(cursor_beat(None, width, spacing), None);
    }
    #[test]
    fn drag_positions_on_both_halves_do_not_jump_to_zero() {
        let spacing = 40.0;
        let width = 2000.0;
        for beat in [2.0, 8.0, 20.0, 32.0, 44.0] {
            let x = music_score::score_beat_x(beat, spacing);
            let normalized = Vec2::new(x / width - 0.5, 0.0);
            let actual = cursor_beat(Some(normalized), width, spacing).unwrap();
            assert!((actual - beat).abs() < 0.001, "beat {beat} became {actual}");
        }
        // Scrolling changes the point's screen coordinate, not its
        // normalized position within the full score content.
        let scroll = 700.0;
        let screen_x = 156.0;
        let normalized = Vec2::new((scroll + screen_x) / width - 0.5, 0.0);
        assert!((cursor_beat(Some(normalized), width, spacing).unwrap() - 20.0).abs() < 0.001);
    }

    #[test]
    fn practice_selection_round_trips_through_tempo_changes_and_pickup() {
        let mut chart: harmonicon_core::chart::HarpChart = serde_json::from_str(include_str!(
            "../../../harmonicon-song/tests/fixtures/piano-chords.harpchart"
        ))
        .unwrap();
        chart.timing.tempo_map = vec![
            harmonicon_core::chart::TempoPoint { tick: 0, bpm: 80.0 },
            harmonicon_core::chart::TempoPoint { tick: 48, bpm: 120.0 },
        ];
        chart.timing.pickup_ticks = Some(12);
        for time in [0.0, 1.5, 3.0, 4.0, 8.0] {
            assert!((beat_time(time_beat(time, &chart), &chart) - time).abs() < 0.001);
        }
    }
}
