// SPDX-License-Identifier: MIT

//! One lesson's page: the instructional body, a Start button for
//! chart-backed lessons, Mark-as-Done for instructional-only ones, and the
//! row of five training tiers under it. Reached from a node of
//! `lesson_tree`, which is the only way in — the curriculum used to
//! have a flat list page here as well, and now has one home per lesson.
//!
//! Discovery/unlock/pass logic lives in `harmonicon_song::lessons`; this
//! module is only the menu surface.

use bevy::ui_widgets::Activate;
use bevy::{audio::Volume, prelude::*};
use bevy_fluent::Localization;

use harmonicon_app::profile::{PlayerProfile, record_lesson, save_profile, training_key};
use harmonicon_audio::AudioSettings;
use harmonicon_core::chart::Scale;
use harmonicon_core::harmonica::{Position, Progression, progression_bars, semitone};
use harmonicon_core::pitch_map::{HarpKind, harp_for_key};
use harmonicon_core::training::{Tier, drill_chart};
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::lessons::training_criteria;
use harmonicon_song::lessons::{
    AvailableLessons, LessonContext, LessonEntry, LessonWidget, PassCriteria,
};
use harmonicon_song::song::{SongManifest, training_manifest};
use harmonicon_ui::dialogs::circle_of_fifths::spawn_circle_of_fifths;
use harmonicon_ui::dialogs::metronome::{
    MetronomeClock, MetronomeFeel, click_for_tick, is_downbeat, twelve_bar_for_tick,
};
use harmonicon_ui::dialogs::twelve_bar_grid::{GridConfig, bar_bg, spawn_12_bar_grid};

use harmonicon_app::app::{
    AppState, GameplayMode, GeneratedSong, JamPositionCycle, JamProgression, JamScale, SelectedSong,
};
use harmonicon_menu::menu::MenuPage;
use harmonicon_menu::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root};

/// The lesson this page shows — set by a skill-tree node right before it
/// switches to [`MenuPage::LessonReader`].
#[derive(Resource, Default)]
pub(crate) struct SelectedLesson(pub Option<String>);

#[derive(Component)]
pub(crate) struct LessonMetronome {
    clock: MetronomeClock,
    bpm: f32,
    pattern: LessonMetronomePattern,
    beats_per_bar: usize,
    muted: bool,
    sync_group: Option<String>,
    label: Entity,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LessonMetronomePattern {
    Straight,
    Shuffle,
    Triplet,
}

impl LessonMetronomePattern {
    const fn clock_feel(self) -> MetronomeFeel {
        match self {
            Self::Straight => MetronomeFeel::Straight,
            Self::Shuffle | Self::Triplet => MetronomeFeel::Shuffle,
        }
    }

    const fn click(self, tick: i64, beats_per_bar: f64) -> Option<(bool, f32)> {
        match self {
            Self::Straight => click_for_tick(tick, beats_per_bar, MetronomeFeel::Straight),
            Self::Shuffle => click_for_tick(tick, beats_per_bar, MetronomeFeel::Shuffle),
            Self::Triplet => Some((
                is_downbeat(tick.div_euclid(3), beats_per_bar) && tick.rem_euclid(3) == 0,
                if tick.rem_euclid(3) == 0 { 1.0 } else { 0.55 },
            )),
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Straight => Self::Shuffle,
            Self::Shuffle => Self::Triplet,
            Self::Triplet => Self::Straight,
        }
    }
}

#[derive(Component)]
pub(crate) struct LessonGrid {
    cells: Vec<Entity>,
    key: String,
    progression: Progression,
    sync_group: Option<String>,
    current_bar: usize,
}

#[derive(Component)]
pub(crate) struct LessonMetronomeAudio;

pub(crate) fn cleanup_lesson_audio(
    mut commands: Commands,
    audio: Query<Entity, With<LessonMetronomeAudio>>,
) {
    for entity in &audio {
        commands.entity(entity).despawn();
    }
}

fn highlight_lesson_grid(
    grid: &LessonGrid,
    bar: usize,
    colors: harmonicon_platform::theme::TwelveBarColors,
    backgrounds: &mut Query<&mut BackgroundColor>,
) {
    for (index, entity) in grid.cells.iter().enumerate() {
        if let Ok(mut bg) = backgrounds.get_mut(*entity) {
            *bg = if index == bar {
                BackgroundColor(Color::srgba(0.75, 0.55, 0.08, 0.95))
            } else {
                BackgroundColor(bar_bg(index, &grid.key, grid.progression, colors))
            };
        }
    }
}

const fn stepped_bar(current: usize, delta: i32) -> usize {
    (current as i32 + delta).rem_euclid(12) as usize
}

#[derive(Component)]
struct LessonCircle {
    diagram: Entity,
    host: Entity,
    harp_key: String,
    positions: Vec<Position>,
}

pub(crate) fn update_lesson_metronomes(
    time: Res<Time>,
    mut metronomes: Query<&mut LessonMetronome>,
    mut grids: Query<&mut LessonGrid>,
    mut labels: Query<&mut Text>,
    mut backgrounds: Query<&mut BackgroundColor>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioSettings>,
    theme: Res<LoadedTheme>,
    mut commands: Commands,
) {
    for mut metronome in &mut metronomes {
        let bpm = metronome.bpm;
        let pattern = metronome.pattern;
        let feel = pattern.clock_feel();
        let tick = metronome
            .clock
            .advance(time.delta_secs_f64(), f64::from(bpm), feel);
        if let Ok(mut text) = labels.get_mut(metronome.label) {
            *text = Text::new(format!("\u{2669} = {}", bpm as u32));
        }
        let Some(tick) = tick else { continue };
        if !metronome.muted
            && let Some((accent, gain)) = pattern.click(tick, metronome.beats_per_bar as f64)
        {
            let sample = if accent {
                "sounds/metronome_high.ogg"
            } else {
                "sounds/metronome_low.ogg"
            };
            commands.spawn((
                AudioPlayer::<AudioSource>(asset_server.load(sample)),
                PlaybackSettings::DESPAWN
                    .with_volume(Volume::Linear(audio.metronome_volume * gain)),
                LessonMetronomeAudio,
            ));
        }
        if tick == 0 || (feel == MetronomeFeel::Shuffle && tick.rem_euclid(3) != 0) {
            continue;
        }
        let bar = twelve_bar_for_tick(tick, metronome.beats_per_bar, feel);
        for mut grid in &mut grids {
            if grid.sync_group.is_none() || grid.sync_group != metronome.sync_group {
                continue;
            }
            grid.current_bar = bar;
            highlight_lesson_grid(&grid, bar, theme.twelve_bar_colors(), &mut backgrounds);
        }
    }
}

/// Looks a lesson up by id. The tree always sets [`SelectedLesson`] before
/// opening this page, so a miss only happens if something desyncs — the
/// reader degrades to an empty page with a Back button rather than
/// panicking.
fn find_lesson<'a>(lessons: &'a AvailableLessons, id: &str) -> Option<&'a LessonEntry> {
    lessons.0.iter().find(|l| l.manifest.id == id)
}

/// One localized "Goal: ..." line for a lesson's pass criteria, or the
/// finish-to-pass wording when it has none but is still playable.
fn goal_line(loc: &Localization, entry: &LessonEntry) -> Option<String> {
    let pct = |t: f32| format!("{:.0}", t * 100.0);
    match &entry.manifest.pass_criteria {
        Some(PassCriteria::Accuracy { threshold }) => Some(
            loc.msg_args("lesson-goal-accuracy", &[("pct", pct(*threshold))])
                .into(),
        ),
        Some(PassCriteria::Technique {
            technique,
            threshold,
        }) => Some(
            loc.msg_args(
                "lesson-goal-technique",
                &[("pct", pct(*threshold)), ("technique", technique.clone())],
            )
            .into(),
        ),
        Some(PassCriteria::ScaleAdherence { threshold }) => Some(
            loc.msg_args("lesson-goal-scale-adherence", &[("pct", pct(*threshold))])
                .into(),
        ),
        Some(PassCriteria::ChordToneAdherence { threshold }) => Some(
            loc.msg_args(
                "lesson-goal-chord-tone-adherence",
                &[("pct", pct(*threshold))],
            )
            .into(),
        ),
        Some(PassCriteria::PhraseDiscipline { threshold }) => Some(
            loc.msg_args("lesson-goal-phrase-discipline", &[("pct", pct(*threshold))])
                .into(),
        ),
        None if entry.chart_asset_path.is_some() => Some(loc.msg("lesson-goal-finish").into()),
        None => None,
    }
}

/// Whether `criteria` routes a lesson into an open jam (`GameplayMode::
/// JamSession`) instead of the ordinary chart pipeline — every criterion
/// judged from `jam::improv::ImprovStats` rather than a chart run, not just
/// `ScaleAdherence`. Pure so it's directly unit-testable.
fn is_jam_criteria(criteria: Option<&PassCriteria>) -> bool {
    matches!(
        criteria,
        Some(PassCriteria::ScaleAdherence { .. })
            | Some(PassCriteria::ChordToneAdherence { .. })
            | Some(PassCriteria::PhraseDiscipline { .. })
    )
}

/// Parses a lesson manifest's `progression` field (schema-enforced to
/// `"standard"`/`"quick-change"`/`"minor"`/`"jazz-blues"` when present) into
/// the `Progression` it names. Absent or unrecognized both fall back to
/// `Standard` — the same "don't let a stale pick linger" default the
/// real-song Jam Session button applies.
pub(crate) fn parse_progression(s: Option<&str>) -> Progression {
    match s {
        Some("quick-change") => Progression::QuickChange,
        Some("minor") => Progression::Minor,
        Some("jazz-blues") => Progression::JazzBlues,
        _ => Progression::Standard,
    }
}

/// Parses a lesson manifest's `scale` field (schema-enforced to
/// `"first-position"`/`"second-position"`/`"third-position"`/`"major"`/
/// `"minor-pentatonic"`/`"country"` when present) into the `Scale` it
/// names. Absent or unrecognized both fall back to `FirstPosition` (the
/// blues hexatonic) — same "don't let a stale pick linger" reasoning as
/// [`parse_progression`].
pub(crate) fn parse_scale(s: Option<&str>) -> Scale {
    match s {
        Some("second-position") => Scale::SecondPosition,
        Some("third-position") => Scale::ThirdPosition,
        Some("major") => Scale::Major,
        Some("minor-pentatonic") => Scale::MinorPentatonic,
        Some("country") => Scale::Country,
        _ => Scale::FirstPosition,
    }
}

/// The lesson's five training tiers, as a row of buttons under Start.
///
/// A lesson with no `training` block gets nothing — that is the honest
/// answer for anything instructional-only, where the microphone cannot
/// verify the technique and five drills would only pretend to check it.
///
/// Tiers are **not gated on each other**. Practising tier 4 before tier 2
/// is the player's business, and locking them would remove the one choice
/// the ladder offers; the label carries the tick so progress is still
/// visible.
fn spawn_training_row(
    commands: &mut Commands,
    root: Entity,
    entry: &LessonEntry,
    profile: &PlayerProfile,
    loc: &Localization,
) {
    if entry.manifest.training.is_none() {
        return;
    }
    let lesson_id = entry.manifest.id.clone();
    spawn_reader_line(
        commands,
        root,
        String::from(loc.msg_args(
            "lesson-training-heading",
            &[(
                "percent",
                ((profile.mastery(&lesson_id, Tier::ALL.len()) * 100.0).round() as u32).to_string(),
            )],
        )),
        Color::srgb(0.80, 0.82, 0.90),
    );

    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(row);

    for tier in Tier::ALL {
        let done = profile
            .trainings
            .get(&training_key(&lesson_id, tier.number()))
            .is_some_and(|r| r.passed);
        let label = if done {
            format!("\u{2713} {}", tier.number())
        } else {
            tier.number().to_string()
        };
        let spec = entry.manifest.drill_spec(tier);
        let criteria = entry.manifest.pass_criteria.clone();
        let id = lesson_id.clone();
        spawn_button(
            commands,
            row,
            &label,
            move |_: On<Activate>,
                  mut manifests: ResMut<Assets<SongManifest>>,
                  mut mode: ResMut<GameplayMode>,
                  mut state: ResMut<NextState<AppState>>,
                  mut commands: Commands| {
                let Some(spec) = spec.clone() else { return };
                let harp = harp_for_key("C", HarpKind::Diatonic);
                let Some(chart) = drill_chart(&spec, &harp, &id, "") else {
                    // No hole in the lesson can do the technique on this
                    // harp. Nothing to play, so stay put rather than open a
                    // drill of plain notes that trains nothing.
                    return;
                };
                commands.insert_resource(SelectedSong(manifests.add(training_manifest(chart))));
                commands.insert_resource(LessonContext {
                    lesson_id: id.clone(),
                    pass_criteria: Some(training_criteria(criteria.as_ref(), tier)),
                    aural: false,
                    tier: Some(tier.number()),
                });
                // Built by `Assets::add`, so it has no `LoadState` and
                // `SongLoading` would wait on it forever — see
                // `app::GeneratedSong`.
                commands.insert_resource(GeneratedSong);
                *mode = GameplayMode::Play2D;
                state.set(AppState::Playing);
            },
        );
    }
}

/// One plain text line appended directly to `root` (no card/box around
/// it) — the shared shape the lesson reader's goal-progress line and its
/// "Passed" badge both use, differing only in text/color.
fn spawn_reader_line(commands: &mut Commands, root: Entity, text: String, color: Color) {
    let line = commands
        .spawn((
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(color),
        ))
        .id();
    commands.entity(root).add_child(line);
}

// ── Lesson reader page ────────────────────────────────────────────────────────

pub(crate) fn setup_lesson_reader(
    mut commands: Commands,
    selected: Res<SelectedLesson>,
    lessons: Res<AvailableLessons>,
    profile: Res<PlayerProfile>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    let entry = selected
        .0
        .as_deref()
        .and_then(|id| find_lesson(&lessons, id));

    let title = entry
        .map(|e| String::from(loc.msg(&e.manifest.title_key)))
        .unwrap_or_default();
    let (root, header, _page_root) =
        spawn_menu_root(&mut commands, &title, None, &theme, "Lessons");

    let Some(entry) = entry else {
        spawn_back_to_tree(&mut commands, header, &loc);
        return;
    };

    // Instructional body — width-capped so long text wraps like a page
    // rather than spanning the whole window, on a dark translucent card so
    // it stays readable over a busy background image.
    let body = commands
        .spawn((
            Node {
                max_width: Val::Px(760.0),
                margin: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.95)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(String::from(loc.msg(&entry.manifest.body_key))),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.82, 0.84, 0.90)),
            ));
        })
        .id();
    commands.entity(root).add_child(body);

    // Reusable teaching aids sit below the body and above goal/pass lines.
    // An empty position list means "show every position", which preserves
    // the original diagram's useful overview without verbose manifest data.
    for widget in &entry.manifest.widgets {
        match widget {
            LessonWidget::CircleOfFifths {
                harp_key,
                positions,
            } => {
                let selected: Vec<Position> = if positions.is_empty() {
                    Position::all().to_vec()
                } else {
                    positions
                        .iter()
                        .filter_map(|name| match name.as_str() {
                            "first" => Some(Position::First),
                            "second" => Some(Position::Second),
                            "third" => Some(Position::Third),
                            "fourth" => Some(Position::Fourth),
                            "fifth" => Some(Position::Fifth),
                            "twelfth" => Some(Position::Twelfth),
                            _ => None,
                        })
                        .collect()
                };
                let mut diagram = Entity::PLACEHOLDER;
                commands.entity(root).with_children(|parent| {
                    diagram = spawn_circle_of_fifths(
                        parent,
                        harp_key,
                        &selected,
                        theme.circle_of_fifths_colors(),
                    );
                });
                let state = commands
                    .spawn(LessonCircle {
                        diagram,
                        host: root,
                        harp_key: harp_key.clone(),
                        positions: selected,
                    })
                    .id();
                commands.entity(root).add_child(state);
                let target = state;
                spawn_button(
                    &mut commands,
                    root,
                    &loc.msg("lesson-widget-key-previous"),
                    move |_: On<Activate>,
                          mut commands: Commands,
                          theme: Res<LoadedTheme>,
                          mut q: Query<&mut LessonCircle>| {
                        let Ok(mut circle) = q.get_mut(target) else {
                            return;
                        };
                        commands.entity(circle.diagram).despawn();
                        circle.harp_key = semitone(&circle.harp_key, -1);
                        let mut diagram = Entity::PLACEHOLDER;
                        commands.entity(circle.host).with_children(|parent| {
                            diagram = spawn_circle_of_fifths(
                                parent,
                                &circle.harp_key,
                                &circle.positions,
                                theme.circle_of_fifths_colors(),
                            );
                        });
                        circle.diagram = diagram;
                    },
                );
                let target = state;
                spawn_button(
                    &mut commands,
                    root,
                    &loc.msg("lesson-widget-key-next"),
                    move |_: On<Activate>,
                          mut commands: Commands,
                          theme: Res<LoadedTheme>,
                          mut q: Query<&mut LessonCircle>| {
                        let Ok(mut circle) = q.get_mut(target) else {
                            return;
                        };
                        commands.entity(circle.diagram).despawn();
                        circle.harp_key = semitone(&circle.harp_key, 1);
                        let mut diagram = Entity::PLACEHOLDER;
                        commands.entity(circle.host).with_children(|parent| {
                            diagram = spawn_circle_of_fifths(
                                parent,
                                &circle.harp_key,
                                &circle.positions,
                                theme.circle_of_fifths_colors(),
                            );
                        });
                        circle.diagram = diagram;
                    },
                );
            }
            LessonWidget::TwelveBarGrid {
                key,
                progression,
                sync_group,
            } => {
                let progression = parse_progression(Some(progression));
                let chords: Vec<String> = progression_bars(key, progression)
                    .into_iter()
                    .map(|(root, _)| root)
                    .collect();
                let mut cells = Vec::new();
                commands.entity(root).with_children(|parent| {
                    cells = spawn_12_bar_grid(
                        parent,
                        &chords,
                        key,
                        progression,
                        &GridConfig::for_3d(),
                        theme.twelve_bar_colors(),
                    );
                });
                let marker = commands
                    .spawn(LessonGrid {
                        cells,
                        key: key.clone(),
                        progression,
                        sync_group: sync_group.clone(),
                        current_bar: 0,
                    })
                    .id();
                commands.entity(root).add_child(marker);
                let target = marker;
                spawn_button(
                    &mut commands,
                    root,
                    &loc.msg("lesson-widget-bar-previous"),
                    move |_: On<Activate>,
                          theme: Res<LoadedTheme>,
                          mut grids: Query<&mut LessonGrid>,
                          mut backgrounds: Query<&mut BackgroundColor>| {
                        let Ok(mut grid) = grids.get_mut(target) else {
                            return;
                        };
                        grid.current_bar = stepped_bar(grid.current_bar, -1);
                        highlight_lesson_grid(
                            &grid,
                            grid.current_bar,
                            theme.twelve_bar_colors(),
                            &mut backgrounds,
                        );
                    },
                );
                let target = marker;
                spawn_button(
                    &mut commands,
                    root,
                    &loc.msg("lesson-widget-bar-next"),
                    move |_: On<Activate>,
                          theme: Res<LoadedTheme>,
                          mut grids: Query<&mut LessonGrid>,
                          mut backgrounds: Query<&mut BackgroundColor>| {
                        let Ok(mut grid) = grids.get_mut(target) else {
                            return;
                        };
                        grid.current_bar = stepped_bar(grid.current_bar, 1);
                        highlight_lesson_grid(
                            &grid,
                            grid.current_bar,
                            theme.twelve_bar_colors(),
                            &mut backgrounds,
                        );
                    },
                );
                let target = marker;
                spawn_button(
                    &mut commands,
                    root,
                    &loc.msg("lesson-widget-bar-reset"),
                    move |_: On<Activate>,
                          theme: Res<LoadedTheme>,
                          mut grids: Query<&mut LessonGrid>,
                          mut backgrounds: Query<&mut BackgroundColor>| {
                        let Ok(mut grid) = grids.get_mut(target) else {
                            return;
                        };
                        grid.current_bar = 0;
                        highlight_lesson_grid(
                            &grid,
                            0,
                            theme.twelve_bar_colors(),
                            &mut backgrounds,
                        );
                    },
                );
            }
            LessonWidget::Metronome {
                bpm,
                beats_per_bar,
                feel,
                sync_group,
            } => {
                let label = commands
                    .spawn((
                        Text::new(format!("\u{2669} = {}", *bpm as u32)),
                        TextFont {
                            font_size: FontSize::Px(22.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ))
                    .id();
                let metronome = commands
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(8.0),
                            ..default()
                        },
                        LessonMetronome {
                            clock: MetronomeClock::default(),
                            bpm: *bpm,
                            pattern: match feel.as_str() {
                                "shuffle" => LessonMetronomePattern::Shuffle,
                                "triplet" => LessonMetronomePattern::Triplet,
                                _ => LessonMetronomePattern::Straight,
                            },
                            beats_per_bar: *beats_per_bar,
                            muted: false,
                            sync_group: sync_group.clone(),
                            label,
                        },
                    ))
                    .add_child(label)
                    .id();
                commands.entity(root).add_child(metronome);
                let target = metronome;
                spawn_button(
                    &mut commands,
                    metronome,
                    &loc.msg("lesson-widget-metronome-toggle"),
                    move |_: On<Activate>, mut q: Query<&mut LessonMetronome>| {
                        if let Ok(mut metronome) = q.get_mut(target) {
                            metronome.clock.running = !metronome.clock.running;
                        }
                    },
                );
                let target = metronome;
                spawn_button(
                    &mut commands,
                    metronome,
                    &loc.msg("lesson-widget-tempo-decrease"),
                    move |_: On<Activate>, mut q: Query<&mut LessonMetronome>| {
                        if let Ok(mut metronome) = q.get_mut(target) {
                            metronome.bpm = (metronome.bpm - 5.0).max(30.0);
                        }
                    },
                );
                let target = metronome;
                spawn_button(
                    &mut commands,
                    metronome,
                    &loc.msg("lesson-widget-tempo-increase"),
                    move |_: On<Activate>, mut q: Query<&mut LessonMetronome>| {
                        if let Ok(mut metronome) = q.get_mut(target) {
                            metronome.bpm = (metronome.bpm + 5.0).min(300.0);
                        }
                    },
                );
                let target = metronome;
                spawn_button(
                    &mut commands,
                    metronome,
                    &loc.msg("lesson-widget-feel-toggle"),
                    move |_: On<Activate>, mut q: Query<&mut LessonMetronome>| {
                        if let Ok(mut metronome) = q.get_mut(target) {
                            metronome.pattern = metronome.pattern.next();
                            metronome.clock.reset();
                        }
                    },
                );
                let target = metronome;
                spawn_button(
                    &mut commands,
                    metronome,
                    &loc.msg("lesson-widget-sound-toggle"),
                    move |_: On<Activate>, mut q: Query<&mut LessonMetronome>| {
                        if let Ok(mut metronome) = q.get_mut(target) {
                            metronome.muted = !metronome.muted;
                        }
                    },
                );
            }
        }
    }

    // Compatibility for externally-authored lessons using the original
    // single-purpose field. Bundled lessons use `widgets`.
    if entry.manifest.widgets.is_empty()
        && entry.manifest.diagram.as_deref() == Some("circle-of-fifths")
    {
        commands.entity(root).with_children(|parent| {
            let _ = spawn_circle_of_fifths(
                parent,
                "C",
                Position::all(),
                theme.circle_of_fifths_colors(),
            );
        });
    }

    if let Some(goal) = goal_line(&loc, entry) {
        spawn_reader_line(&mut commands, root, goal, Color::srgb(0.85, 0.72, 0.35));
    }

    let record = profile.lessons.get(&entry.manifest.id);
    if record.is_some_and(|r| r.passed) {
        spawn_reader_line(
            &mut commands,
            root,
            format!("\u{2713} {}", loc.msg("lesson-passed")),
            Color::srgb(0.45, 0.95, 0.50),
        );
    }

    match &entry.chart_asset_path {
        // Chart-backed lesson: Start launches the chart through the normal
        // song pipeline, with a LessonContext so results judge the pass
        // criteria (and adaptive difficulty leaves every note unlocked).
        Some(chart_path) => {
            let chart_path = chart_path.clone();
            let lesson_id = entry.manifest.id.clone();
            let criteria = entry.manifest.pass_criteria.clone();
            let progression = entry.manifest.progression.clone();
            let scale = entry.manifest.scale.clone();
            let position_cycle = entry.manifest.position_cycle;
            let aural = entry.manifest.aural;
            spawn_button(
                &mut commands,
                root,
                &loc.msg("lesson-start"),
                move |_: On<Activate>,
                      asset_server: Res<AssetServer>,
                      mut mode: ResMut<GameplayMode>,
                      mut jam_progression: ResMut<JamProgression>,
                      mut jam_scale: ResMut<JamScale>,
                      mut jam_position_cycle: ResMut<JamPositionCycle>,
                      mut state: ResMut<NextState<AppState>>,
                      mut commands: Commands| {
                    commands.insert_resource(SelectedSong(
                        asset_server.load::<SongManifest>(chart_path.clone()),
                    ));
                    commands.insert_resource(LessonContext {
                        lesson_id: lesson_id.clone(),
                        pass_criteria: criteria.clone(),
                        aural,
                        tier: None,
                    });
                    // A jam-based lesson (scale-adherence/chord-tone-
                    // adherence/phrase-discipline) is an open jam, not a
                    // chart to play through — see `is_jam_criteria`.
                    if is_jam_criteria(criteria.as_ref()) {
                        *mode = GameplayMode::JamSession;
                        jam_progression.0 = parse_progression(progression.as_deref());
                        jam_scale.0 = parse_scale(scale.as_deref());
                        jam_position_cycle.0 = position_cycle;
                    } else {
                        *mode = GameplayMode::Play2D;
                    }
                    state.set(AppState::SongLoading);
                },
            );
            spawn_training_row(&mut commands, root, entry, &profile, &loc);
        }
        // Instructional-only lesson: nothing to score — reading it and
        // saying "done" is the pass (see docs/lessons_plan.md on what's
        // honestly verifiable). Hidden once passed.
        None if !record.is_some_and(|r| r.passed) => {
            let lesson_id = entry.manifest.id.clone();
            spawn_button(
                &mut commands,
                root,
                &loc.msg("lesson-mark-done"),
                move |_: On<Activate>,
                      mut profile: ResMut<PlayerProfile>,
                      mut page: ResMut<NextState<MenuPage>>| {
                    let record = profile.lessons.entry(lesson_id.clone()).or_default();
                    record_lesson(record, true, 0.0);
                    save_profile(&profile);
                    // Back to the tree, which re-spawns with this node
                    // passed and anything it unlocked now lit.
                    page.set(MenuPage::LessonTree);
                },
            );
        }
        None => {}
    }

    spawn_back_to_tree(&mut commands, header, &loc);
}

fn spawn_back_to_tree(commands: &mut Commands, header: Entity, loc: &Localization) {
    spawn_back_button(
        commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::LessonTree),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lesson_grid_steps_and_wraps_in_both_directions() {
        assert_eq!(stepped_bar(0, -1), 11);
        assert_eq!(stepped_bar(11, 1), 0);
        assert_eq!(stepped_bar(5, 1), 6);
    }

    #[test]
    fn triplet_pattern_clicks_three_equal_subdivisions() {
        let pattern = LessonMetronomePattern::Triplet;
        assert_eq!(pattern.click(0, 4.0), Some((true, 1.0)));
        assert_eq!(pattern.click(1, 4.0), Some((false, 0.55)));
        assert_eq!(pattern.click(2, 4.0), Some((false, 0.55)));
        assert_eq!(pattern.click(3, 4.0), Some((false, 1.0)));
    }

    #[test]
    fn lesson_metronome_pattern_cycles_through_every_mode() {
        assert!(matches!(
            LessonMetronomePattern::Straight.next(),
            LessonMetronomePattern::Shuffle
        ));
        assert!(matches!(
            LessonMetronomePattern::Shuffle.next(),
            LessonMetronomePattern::Triplet
        ));
        assert!(matches!(
            LessonMetronomePattern::Triplet.next(),
            LessonMetronomePattern::Straight
        ));
    }

    #[test]
    fn lesson_audio_cleanup_despawns_every_active_click() {
        let mut app = App::new();
        app.add_systems(Update, cleanup_lesson_audio);
        let first = app.world_mut().spawn(LessonMetronomeAudio).id();
        let second = app.world_mut().spawn(LessonMetronomeAudio).id();
        app.update();
        assert!(app.world().get_entity(first).is_err());
        assert!(app.world().get_entity(second).is_err());
    }

    // ── is_jam_criteria ───────────────────────────────────────────────────────

    #[test]
    fn every_jam_based_criterion_routes_into_jam_session() {
        for c in [
            PassCriteria::ScaleAdherence { threshold: 0.1 },
            PassCriteria::ChordToneAdherence { threshold: 0.1 },
            PassCriteria::PhraseDiscipline { threshold: 0.1 },
        ] {
            assert!(is_jam_criteria(Some(&c)));
        }
    }

    #[test]
    fn chart_based_criteria_and_none_stay_on_the_ordinary_pipeline() {
        assert!(!is_jam_criteria(None));
        assert!(!is_jam_criteria(Some(&PassCriteria::Accuracy {
            threshold: 0.5
        })));
        assert!(!is_jam_criteria(Some(&PassCriteria::Technique {
            technique: "bend".into(),
            threshold: 0.5
        })));
    }

    // ── parse_progression ─────────────────────────────────────────────────────

    #[test]
    fn parse_progression_reads_each_known_value() {
        assert_eq!(parse_progression(Some("standard")), Progression::Standard);
        assert_eq!(
            parse_progression(Some("quick-change")),
            Progression::QuickChange
        );
        assert_eq!(parse_progression(Some("minor")), Progression::Minor);
        assert_eq!(
            parse_progression(Some("jazz-blues")),
            Progression::JazzBlues
        );
    }

    #[test]
    fn parse_progression_defaults_to_standard_when_absent_or_unknown() {
        assert_eq!(parse_progression(None), Progression::Standard);
        assert_eq!(parse_progression(Some("jazz")), Progression::Standard);
    }

    // ── parse_scale ────────────────────────────────────────────────────────────

    #[test]
    fn parse_scale_reads_each_known_value() {
        assert_eq!(parse_scale(Some("first-position")), Scale::FirstPosition);
        assert_eq!(parse_scale(Some("second-position")), Scale::SecondPosition);
        assert_eq!(parse_scale(Some("third-position")), Scale::ThirdPosition);
        assert_eq!(parse_scale(Some("major")), Scale::Major);
        assert_eq!(
            parse_scale(Some("minor-pentatonic")),
            Scale::MinorPentatonic
        );
        assert_eq!(parse_scale(Some("country")), Scale::Country);
    }

    #[test]
    fn parse_scale_defaults_to_first_position_when_absent_or_unknown() {
        assert_eq!(parse_scale(None), Scale::FirstPosition);
        assert_eq!(parse_scale(Some("dorian")), Scale::FirstPosition);
    }
}
