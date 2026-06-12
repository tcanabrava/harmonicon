use bevy::prelude::*;

use crate::{
    assets_management::GlobalFonts,
    menu::SelectedSong,
    song::SongManifest,
    song::chart::{Action, Modifier, PlayMode},
    song::harmonica::twelve_bar,
};

use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, ComboText,
    FeedbackText, GameplayRoot, HIT_H_PCT, HOLE_COUNT, HoleCell, HoleState, LANE_PCT, LOOKAHEAD,
    MusicStarted, NoteVisual, ScheduledNote, ScoreText, ValidHarpNotes, modifier_color,
};
use super::countdown_overlay::spawn_countdown;
use super::metronome_overlay::spawn_metronome;
use super::phrase_overlay::spawn_phrase_banner;
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut valid_notes: ResMut<ValidHarpNotes>,
    fonts: Res<GlobalFonts>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing state");
        return;
    };
    clock.0 = -COUNTDOWN;
    music_started.0 = false;
    valid_notes.0 = manifest.chart.harmonica.build_valid_notes();

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let bpm = chart.song.tempo_bpm;
    let chords = twelve_bar(key);

    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    let info = format!(
        "Key: {}   \u{2669} = {}   {}",
        key,
        bpm as u32,
        chart.song.time_signature.as_deref().unwrap_or("4/4"),
    );
    let harp_info = chart.harmonica.display();
    let description = chart
        .metadata
        .as_ref()
        .and_then(|m| m.description.as_deref());
    let chart_author = chart.metadata.as_ref().and_then(|m| m.author.as_deref());

    let beats_per_bar = {
        let ts = chart.song.time_signature.as_deref().unwrap_or("4/4");
        ts.split('/').next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(4)
    };

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            ImageNode::new(manifest.background.clone()),
            GameplayRoot,
        ))
        .with_children(|root| {
            // Dark overlay
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.70)),
            ));

            // ── Left panel: note highway + harmonica ─────────────────────────
            root.spawn(Node {
                width: Val::Percent(60.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|left| {
                // Note highway
                left.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(120.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.06, 0.06, 0.09)),
                ))
                .with_children(|hw| {
                    spawn_highway(hw, &fonts.symbols, chart);
                });

                // Harmonica holes
                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|col| {
                    spawn_harmonica_strip(col, chart, &fonts.gameplay);
                });
            });

            // ── Right panel: info + 12-bar + metronome + score ───────────────
            root.spawn(Node {
                width: Val::Percent(40.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|right| {
                // Song info
                right.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Text::new(title),
                        TextFont { font_size: FontSize::Px(18.0), font: fonts.gameplay.clone(), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    col.spawn((
                        Text::new(info),
                        TextFont { font_size: FontSize::Px(12.0), font: fonts.gameplay.clone(), ..default() },
                        TextColor(Color::srgb(0.60, 0.65, 0.75)),
                    ));
                    col.spawn((
                        Text::new(harp_info),
                        TextFont { font_size: FontSize::Px(11.0), font: fonts.gameplay.clone(), ..default() },
                        TextColor(Color::srgb(0.45, 0.72, 0.55)),
                    ));
                    if let Some(desc) = description {
                        col.spawn((
                            Text::new(desc.to_string()),
                            TextFont { font_size: FontSize::Px(10.0), font: fonts.gameplay.clone(), ..default() },
                            TextColor(Color::srgb(0.50, 0.50, 0.55)),
                        ));
                    }
                    if let Some(author) = chart_author {
                        col.spawn((
                            Text::new(format!("Chart: {author}")),
                            TextFont { font_size: FontSize::Px(9.0), font: fonts.gameplay.clone(), ..default() },
                            TextColor(Color::srgb(0.40, 0.40, 0.45)),
                        ));
                    }
                });

                // Live phrase / groove banner (driven by phrase_overlay::update_phrase)
                spawn_phrase_banner(right, &fonts.gameplay);

                // 12-bar blues grid
                right.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    ..default()
                })
                .with_children(|grid| {
                    spawn_12_bar_grid(grid, &chords, key, &fonts.gameplay, &GridConfig::for_2d());
                });

                // Metronome
                right.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|metro| {
                    spawn_metronome(metro, beats_per_bar, bpm, &fonts.gameplay);
                });

                // Score
                right.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|p| {
                    p.spawn((
                        Text::new("0"),
                        TextFont { font_size: FontSize::Px(28.0), font: fonts.gameplay.clone(), ..default() },
                        TextColor(Color::WHITE),
                        ScoreText,
                    ));
                    p.spawn((
                        Text::new(""),
                        TextFont { font_size: FontSize::Px(14.0), font: fonts.gameplay.clone(), ..default() },
                        TextColor(Color::srgb(0.90, 0.72, 0.20)),
                        ComboText,
                    ));
                    p.spawn((
                        Text::new(""),
                        TextFont { font_size: FontSize::Px(20.0), font: fonts.gameplay.clone(), ..default() },
                        TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                        FeedbackText,
                    ));
                });
            });

        });
    spawn_countdown(&mut commands, &fonts.gameplay);
}

fn note_height_pct(duration: f64) -> f32 {
    ((duration / LOOKAHEAD) as f32 * 100.0).clamp(3.5, 40.0)
}

/// Top-percentage position for a note node scrolling down the highway.
pub fn note_top_pct(note_time: f64, elapsed: f64, lookahead: f64, height_pct: f32) -> f32 {
    let remaining = note_time - elapsed;
    let progress = 1.0 - (remaining / lookahead) as f32;
    let hit_center_pct = 100.0 - HIT_H_PCT * 0.5;
    hit_center_pct * progress - height_pct
}

fn spawn_highway(
    hw: &mut ChildSpawnerCommands,
    font: &FontSource,
    chart: &crate::song::chart::HarpChart,
) {
    use crate::song::chart::Action;

    for h in 0..HOLE_COUNT {
        let left_pct = h as f32 * LANE_PCT;
        let alpha = if h % 2 == 0 { 0.04f32 } else { 0.0f32 };
        hw.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(left_pct),
                top: Val::Percent(0.0),
                width: Val::Percent(LANE_PCT),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, alpha)),
        ));
        if h > 0 {
            hw.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    top: Val::Percent(0.0),
                    width: Val::Px(1.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
            ));
        }
    }

    // Hit zone
    hw.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(0.0),
            bottom: Val::Percent(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(HIT_H_PCT),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 0.55, 0.10)),
    ));
    hw.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(0.0),
            bottom: Val::Percent(HIT_H_PCT),
            width: Val::Percent(80.0),
            height: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 0.70, 0.55)),
    ));

    for item in &chart.track {
        let t = super::resolve_item_time(item, &chart.timing);
        let h_pct = note_height_pct(item.duration);
        let play_mode = item.play_mode.as_ref();
        for event in &item.events {
            let is_blow = matches!(event.action, Action::Blow);
            let (r, g, b) = if is_blow {
                (0.25f32, 0.55, 0.95)
            } else {
                (0.95f32, 0.38, 0.15)
            };
            let left_pct = (event.hole as f32 - 1.0) * LANE_PCT + 0.3;
            let expected_pitch = event.note.clone().unwrap_or_else(|| {
                chart
                    .harmonica
                    .wind_direction_label(event.hole, &event.action)
            });
            let modifiers = event.modifiers.clone().unwrap_or_default();
            // A modifier tints the tile border so the technique reads at a glance,
            // even before the badge glyphs are legible at the top of the highway.
            let border = modifiers
                .first()
                .map(modifier_color)
                .unwrap_or(Color::srgba(1.0, 1.0, 1.0, 0.50));
            hw.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    top: Val::Percent(-h_pct),
                    width: Val::Percent(LANE_PCT - 0.6),
                    height: Val::Percent(h_pct),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(if modifiers.is_empty() { 1.5 } else { 2.5 })),
                    ..default()
                },
                BackgroundColor(Color::srgba(r, g, b, 0.88)),
                BorderColor::all(border),
                NoteVisual {
                    time: t,
                    height_pct: h_pct,
                },
                ScheduledNote {
                    time: t,
                    hole: event.hole,
                    is_blow,
                    expected_pitch,
                    hit: false,
                    missed: false,
                    modifiers: modifiers.clone(),
                },
            ))
            .with_children(|note| {
                note.spawn((
                    Text::new(if is_blow { "\u{2191}" } else { "\u{2193}" }),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        font: font.clone(),
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                ));

                // Modifier hint badges, pinned to the top edge of the note tile.
                if !modifiers.is_empty() {
                    note.spawn(Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(1.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|badges| {
                        for m in &modifiers {
                            let (label, color) = modifier_badge(m);
                            badges
                                .spawn((
                                    Node {
                                        padding: UiRect::axes(Val::Px(2.0), Val::Px(0.5)),
                                        ..default()
                                    },
                                    BackgroundColor(color),
                                ))
                                .with_children(|pill| {
                                    pill.spawn((
                                        Text::new(label),
                                        TextFont { font_size: FontSize::Px(9.0), ..default() },
                                        TextColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
                                    ));
                                });
                        }
                    });
                }

                // Chord / split play-mode badge, pinned to the bottom edge.
                if let Some(tag) = play_mode_label(play_mode) {
                    note.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(1.0),
                            ..default()
                        },
                        Text::new(tag),
                        TextFont { font_size: FontSize::Px(8.0), ..default() },
                        TextColor(Color::srgba(0.95, 0.95, 1.0, 0.75)),
                    ));
                }
            });
        }
    }
}

/// Short badge label and accent colour for a note technique modifier.
fn modifier_badge(m: &Modifier) -> (String, Color) {
    match m {
        Modifier::Bend { semitones, .. } => {
            let amt = semitones.abs();
            let txt = if (amt - amt.trunc()).abs() < 0.01 {
                format!("\u{266D}{}", amt as i32)
            } else {
                format!("\u{266D}{amt:.1}")
            };
            (txt, modifier_color(m))
        }
        Modifier::Vibrato { .. } => ("vib".into(), modifier_color(m)),
        Modifier::WahWah { .. } => ("wah".into(), modifier_color(m)),
        Modifier::Hold { .. } => ("hold".into(), modifier_color(m)),
        Modifier::Overblow => ("ob".into(), modifier_color(m)),
        Modifier::Overdraw => ("od".into(), modifier_color(m)),
    }
}

/// Label for the multi-note play modes; `single` (and absent) needs no badge.
fn play_mode_label(mode: Option<&PlayMode>) -> Option<&'static str> {
    match mode {
        Some(PlayMode::Chord) => Some("chord"),
        Some(PlayMode::Split) => Some("split"),
        Some(PlayMode::Single) | None => None,
    }
}

fn spawn_harmonica_strip(
    col: &mut ChildSpawnerCommands,
    chart: &crate::song::chart::HarpChart,
    font: &FontSource,
) {
    col.spawn(Node {
        flex_direction: FlexDirection::Row,
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|row| {
        for hole in 1u8..=10 {
            let b = chart.harmonica.wind_direction_label(hole, &Action::Blow);
            let d = chart.harmonica.wind_direction_label(hole, &Action::Draw);
            row.spawn((
                Node {
                    width: Val::Percent(LANE_PCT),
                    height: Val::Vh(9.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceAround,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.10, 0.12, 0.16)),
                BorderColor::all(Color::srgb(0.28, 0.30, 0.40)),
                HoleCell(hole),
                HoleState::default(),
            ))
            .with_children(|cell| {
                cell.spawn((
                    Text::new(b),
                    TextFont { font_size: FontSize::Px(11.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(0.50, 0.75, 1.00)),
                ));
                cell.spawn((
                    Text::new(format!("{hole}")),
                    TextFont { font_size: FontSize::Px(16.0), font: font.clone(), ..default() },
                    TextColor(Color::WHITE),
                ));
                cell.spawn((
                    Text::new(d),
                    TextFont { font_size: FontSize::Px(11.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(1.00, 0.62, 0.35)),
                ));
            });
        }
    });

    // Legend
    col.spawn(Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(20.0),
        ..default()
    })
    .with_children(|leg| {
        leg.spawn((
            Text::new("\u{25A0} BLOW"),
            TextFont { font_size: FontSize::Px(11.0), font: font.clone(), ..default() },
            TextColor(Color::srgb(0.50, 0.75, 1.00)),
        ));
        leg.spawn((
            Text::new("\u{25A0} DRAW"),
            TextFont { font_size: FontSize::Px(11.0), font: font.clone(), ..default() },
            TextColor(Color::srgb(1.00, 0.62, 0.35)),
        ));
    });
}

// ── Per-frame systems ─────────────────────────────────────────────────────────

pub fn update_notes(clock: Res<super::GameplayClock>, mut notes: Query<(&NoteVisual, &mut Node)>) {
    let elapsed = clock.0;
    for (note, mut node) in &mut notes {
        node.top = Val::Percent(note_top_pct(note.time, elapsed, LOOKAHEAD, note.height_pct));
    }
}

pub fn update_holes(
    time: Res<Time>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    targets: Res<ActiveTargets>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut cells: Query<(&HoleCell, &mut BackgroundColor, &mut HoleState)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let dt = time.delta_secs();

    let attack = 1.0 - (-dt * 25.0_f32).exp();
    let decay = 1.0 - (-dt * 4.0_f32).exp();

    let harp_pitches: Vec<&crate::pitch_detect::PitchInfo> = active
        .0
        .iter()
        .filter(|p| valid_notes.0.contains(&format!("{}{}", p.note, p.octave)))
        .collect();

    for (cell, mut bg, mut state) in &mut cells {
        let blow = chart.harmonica.wind_direction_label(cell.0, &Action::Blow);
        let draw = chart.harmonica.wind_direction_label(cell.0, &Action::Draw);

        let mut blow_hit = false;
        let mut draw_hit = false;
        for p in &harp_pitches {
            let name = format!("{}{}", p.note, p.octave);
            if name == blow {
                blow_hit = true;
            }
            if name == draw {
                draw_hit = true;
            }
        }

        let hint = targets
            .0
            .iter()
            .find(|(h, _)| *h == cell.0)
            .map(|(_, b)| *b);
        let hint_floor = if hint.is_some() { 0.18f32 } else { 0.0 };

        let (target, is_blow) = if blow_hit {
            (1.0f32, true)
        } else if draw_hit {
            (1.0f32, false)
        } else if let Some(is_blow_hint) = hint {
            (hint_floor, is_blow_hint)
        } else {
            (0.0f32, state.is_blow)
        };

        if blow_hit || draw_hit {
            state.is_blow = is_blow;
        }

        let factor = if target > state.brightness { attack } else { decay };
        state.brightness += (target - state.brightness) * factor;
        let b = state.brightness;

        let color = if state.is_blow {
            Color::srgb(0.10 + 0.18 * b, 0.12 + 0.33 * b, 0.16 + 0.72 * b)
        } else {
            Color::srgb(0.10 + 0.78 * b, 0.12 + 0.22 * b, (0.16 - 0.04 * b).max(0.0))
        };
        *bg = BackgroundColor(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_pct_clamped_to_minimum() {
        assert_eq!(note_height_pct(0.001), 3.5);
    }

    #[test]
    fn height_pct_clamped_to_maximum() {
        assert_eq!(note_height_pct(LOOKAHEAD), 40.0);
    }

    #[test]
    fn height_pct_proportional() {
        let h = note_height_pct(1.0);
        assert!((h - 33.333).abs() < 0.01, "got {h}");
    }

    #[test]
    fn note_top_pct_at_hit_line() {
        let expected = (100.0 - HIT_H_PCT * 0.5) - 10.0;
        let got = note_top_pct(1.0, 1.0, LOOKAHEAD, 10.0);
        assert!((got - expected).abs() < 0.01, "got {got}");
    }

    #[test]
    fn note_top_pct_in_future_is_negative() {
        let got = note_top_pct(LOOKAHEAD, 0.0, LOOKAHEAD, 10.0);
        assert!((got - (-10.0)).abs() < 0.01, "got {got}");
    }

    #[test]
    fn note_top_pct_moves_down_over_time() {
        let h = 5.0;
        let t0 = note_top_pct(2.0, 0.0, LOOKAHEAD, h);
        let t1 = note_top_pct(2.0, 1.0, LOOKAHEAD, h);
        assert!(t1 > t0, "note should move down (larger top%) as time advances");
    }

    // ── modifier_badge ────────────────────────────────────────────────────────

    #[test]
    fn bend_badge_shows_whole_semitone_without_decimals() {
        let (label, _) = modifier_badge(&Modifier::Bend { semitones: -1.0, intensity: None });
        assert_eq!(label, "\u{266D}1");
    }

    #[test]
    fn bend_badge_shows_half_semitone_with_decimal() {
        let (label, _) = modifier_badge(&Modifier::Bend { semitones: -0.5, intensity: None });
        assert_eq!(label, "\u{266D}0.5");
    }

    #[test]
    fn technique_badges_have_expected_labels() {
        assert_eq!(modifier_badge(&Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }).0, "vib");
        assert_eq!(modifier_badge(&Modifier::WahWah { oscillation_hz: 3.0, intensity: None }).0, "wah");
        assert_eq!(modifier_badge(&Modifier::Hold { intensity: None }).0, "hold");
        assert_eq!(modifier_badge(&Modifier::Overblow).0, "ob");
        assert_eq!(modifier_badge(&Modifier::Overdraw).0, "od");
    }

    // ── play_mode_label ───────────────────────────────────────────────────────

    #[test]
    fn play_mode_label_badges_only_multi_note_modes() {
        assert_eq!(play_mode_label(Some(&PlayMode::Chord)), Some("chord"));
        assert_eq!(play_mode_label(Some(&PlayMode::Split)), Some("split"));
        assert_eq!(play_mode_label(Some(&PlayMode::Single)), None);
        assert_eq!(play_mode_label(None), None);
    }
}
