// SPDX-License-Identifier: MIT
//! A scrollable full score with a beat-aligned practice selection band.
use super::pause_menu::{PauseMenuRoot, SelectedPhraseIndex};
use super::song_progress_overlay::RequestLoopRange;
use super::{LoopConfig, Paused, adaptive_difficulty::AdaptiveDifficulty};
use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, ScrollPosition};
use harmonicon_app::app::{GameplayMode, SelectedSong};
use harmonicon_core::chart::{seconds_to_tick, tick_to_seconds};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::song::SongManifest;
use harmonicon_ui::music_score::{
    self, BravuraFont, MusicScoreNotes, MusicScorePlayhead, MusicScoreSpacing,
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
pub(super) struct Band(f64);
#[derive(Component)]
pub(super) struct Zone {
    index: usize,
    start: f64,
    end: f64,
}
#[derive(Resource, Default)]
pub(super) struct SelectionDrag {
    start: Option<f64>,
    end: f64,
    suppress_click: bool,
}

fn time_beat(seconds: f64, chart: &harmonicon_core::chart::HarpChart) -> f64 {
    let timing = &chart.timing;
    (seconds_to_tick(seconds.max(0.0), timing.resolution, &timing.tempo_map)
        + super::bars::pickup_lead_ticks(chart)) as f64
        / timing.resolution.max(1) as f64
}
fn beat_time(beat: f64, chart: &harmonicon_core::chart::HarpChart) -> f64 {
    let tick = (beat.max(0.0) * chart.timing.resolution.max(1) as f64).round() as u64;
    tick_to_seconds(
        tick.saturating_sub(super::bars::pickup_lead_ticks(chart)),
        chart.timing.resolution,
        &chart.timing.tempo_map,
    )
}
fn cursor_beat(normalized: Option<Vec2>, width: f32, spacing: f32) -> Option<f64> {
    let x = normalized?.x * width;
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
    drag: Option<Res<'w, SelectionDrag>>,
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
        drag,
    } = data;
    if *mode != GameplayMode::Play2D {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let chart = &manifest.chart;
    let end = notes.0.iter().map(|n| n.start_beat + n.duration_beats).fold(1.0, f64::max);
    let width = music_score::score_beat_x(end, spacing.0) + 80.0;
    let grand =
        matches!(chart.instrument(), Some(harmonicon_core::instrument::Instrument::Piano(_)));
    let score_height = music_score::PANEL_HEIGHT * if grand { 2.0 } else { 1.0 };
    if paused.0
        && let Some(bravura) = bravura
    {
        for root in &roots {
            commands.init_resource::<SelectionDrag>();
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
                viewport = harmonicon_ui::dialogs::scroll_area::spawn_scroll_area_xy(
                    p,
                    Color::srgb(0.55, 0.60, 0.7),
                    Color::srgb(0.12, 0.14, 0.18),
                );
            });
            commands.entity(viewport).insert(Viewport).with_children(|p| {
                p.spawn((
                    Content,
                    Node {
                        width: Val::Px(width),
                        flex_shrink: 0.0,
                        height: Val::Px(score_height + 44.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ))
                .with_children(|content| {
                    content
                        .spawn((
                            Band(end),
                            RelativeCursorPosition::default(),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(44.0),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.11, 0.13, 0.18)),
                        ))
                        .observe(reset_selection_click)
                        .observe(begin_selection)
                        .observe(drag_selection)
                        .observe(end_selection)
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
                                        Text::new(section.name.clone()),
                                        TextFont { font_size: FontSize::Px(13.0), ..default() },
                                        TextColor(Color::WHITE),
                                        Pickable::IGNORE,
                                    ));
                                });
                            }
                        });
                    music_score::spawn_full_score(content, &bravura, grand);
                    content.spawn((
                        Selection,
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(0.0),
                            height: Val::Percent(100.0),
                            display: Display::None,
                            border: UiRect::horizontal(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.3, 0.65, 1.0, 0.18)),
                        BorderColor::all(Color::srgb(0.4, 0.75, 1.0)),
                        Pickable::IGNORE,
                    ));
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
        if *was_paused {
            commands.insert_resource(SelectionDrag::default());
        }
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
    let range =
        drag.as_ref().and_then(|d| d.start.map(|s| (s.min(d.end), s.max(d.end)))).or_else(|| {
            loops
                .active
                .then(|| (time_beat(loops.start_time, chart), time_beat(loops.end_time, chart)))
        });
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

fn begin_selection(
    mut ev: On<PointerDragStart>,
    paused: Res<Paused>,
    band: Query<(&RelativeCursorPosition, &ComputedNode, &Band)>,
    spacing: Res<MusicScoreSpacing>,
    mut drag: ResMut<SelectionDrag>,
) {
    if !paused.0 || ev.button != PointerButton::Primary {
        return;
    }
    let Ok((cursor, node, band)) = band.single() else { return };
    if let Some(beat) =
        cursor_beat(cursor.normalized, node.size().x * node.inverse_scale_factor(), spacing.0)
    {
        let beat = beat.min(band.0);
        drag.start = Some(beat);
        drag.end = beat;
        drag.suppress_click = true;
        ev.propagate(false);
    }
}
fn drag_selection(
    mut ev: On<PointerDrag>,
    band: Query<(&RelativeCursorPosition, &ComputedNode, &Band)>,
    spacing: Res<MusicScoreSpacing>,
    mut drag: ResMut<SelectionDrag>,
) {
    if drag.start.is_none() {
        return;
    }
    let Ok((cursor, node, band)) = band.single() else { return };
    if let Some(beat) =
        cursor_beat(cursor.normalized, node.size().x * node.inverse_scale_factor(), spacing.0)
    {
        drag.end = beat.min(band.0);
        ev.propagate(false);
    }
}
fn end_selection(
    mut ev: On<PointerDragEnd>,
    mut drag: ResMut<SelectionDrag>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut requests: MessageWriter<RequestLoopRange>,
) {
    if ev.button != PointerButton::Primary {
        return;
    }
    let Some(start) = drag.start.take() else { return };
    if let Some(manifest) = manifests.get(&selected.0) {
        requests.write(RequestLoopRange {
            start_time: beat_time(start.min(drag.end), &manifest.chart),
            end_time: beat_time(start.max(drag.end), &manifest.chart),
        });
    }
    ev.propagate(false);
}
// not-a-widget-button: these are phrase regions inside the score's drag-selection
// band, with no Button component; pointer clicks must be distinguished from
// completed range drags so a drag cannot activate its starting phrase.
fn select_zone(
    mut ev: On<PointerClick>,
    zones: Query<&Zone>,
    mut selected: ResMut<SelectedPhraseIndex>,
    drag: Res<SelectionDrag>,
    mut requests: MessageWriter<RequestLoopRange>,
) {
    if ev.button != PointerButton::Primary {
        return;
    }
    if drag.suppress_click {
        ev.propagate(false);
        return;
    }
    if let Ok(zone) = zones.get(ev.entity) {
        selected.0 = zone.index;
        requests.write(RequestLoopRange { start_time: zone.start, end_time: zone.end });
        ev.propagate(false);
    }
}

fn reset_selection_click(_: On<PointerPress>, mut drag: ResMut<SelectionDrag>) {
    drag.suppress_click = false;
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
            (cursor_beat(Some(Vec2::new(x / width, 0.5)), width, spacing).unwrap() - 4.0).abs()
                < 0.001
        );
        assert_eq!(cursor_beat(Some(Vec2::ZERO), width, spacing), Some(0.0));
        assert_eq!(cursor_beat(None, width, spacing), None);
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
