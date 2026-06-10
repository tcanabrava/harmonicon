use bevy::{audio::AudioSource, prelude::*};

use crate::{
    assets_management::GlobalFonts,
    menu::{AppState, SelectedSong},
    song::SongManifest,
    harmonica::{blow_label, draw_label, harp_display, semitone, twelve_bar},
};

use super::{
    ActivePitches, CountdownOverlay, CountdownText, GameplayRoot, HoleCell, HoleState,
    MusicStarted, NoteVisual, ValidHarpNotes, COUNTDOWN, HOLE_COUNT, HIT_H_PCT, LANE_PCT,
    LOOKAHEAD,
};

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
    valid_notes.0 = crate::harmonica::build_valid_notes(&manifest.chart);

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
    let harp_info = harp_display(chart);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(0.0), Val::Px(10.0)),
                ..default()
            },
            ImageNode::new(manifest.background.clone()),
            GameplayRoot,
        ))
        .with_children(|root| {
            // Dark overlay so text/notes remain readable over the background art
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.70)),
            ));

            // Title / info
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    Text::new(title),
                    TextFont { font_size: FontSize::Px(22.0), font: fonts.gameplay.clone(), ..default() },
                    TextColor(Color::WHITE),
                ));
                col.spawn((
                    Text::new(info),
                    TextFont { font_size: FontSize::Px(13.0), font: fonts.gameplay.clone(), ..default() },
                    TextColor(Color::srgb(0.60, 0.65, 0.75)),
                ));
                col.spawn((
                    Text::new(harp_info),
                    TextFont { font_size: FontSize::Px(12.0), font: fonts.gameplay.clone(), ..default() },
                    TextColor(Color::srgb(0.45, 0.72, 0.55)),
                ));
            });

            // 12-bar blues grid
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|grid| {
                spawn_12_bar_grid(grid, &chords, key, &fonts.gameplay);
            });

            // Note highway
            root.spawn((
                Node {
                    width: Val::Percent(80.0),
                    flex_grow: 0.8,
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
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                width: Val::Percent(80.0),
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|col| {
                spawn_harmonica_strip(col, chart, &fonts.gameplay);
            });

            // Countdown overlay
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.05, 0.55)),
                GlobalZIndex(100),
                CountdownOverlay,
            ))
            .with_children(|ov| {
                ov.spawn((
                    Text::new("GET READY"),
                    TextFont { font_size: FontSize::Px(22.0), font: fonts.gameplay.clone(), ..default() },
                    TextColor(Color::srgba(0.85, 0.85, 1.0, 0.80)),
                ));
                ov.spawn((
                    Text::new("3"),
                    TextFont { font_size: FontSize::Px(120.0), font: fonts.gameplay.clone(), ..default() },
                    TextColor(Color::WHITE),
                    CountdownText,
                ));
            });
        });
}

fn bar_bg(bar: usize, key: &str) -> Color {
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    let chords = twelve_bar(key);
    if chords[bar] == v {
        Color::srgb(0.20, 0.10, 0.14)
    } else if chords[bar] == iv {
        Color::srgb(0.10, 0.20, 0.14)
    } else {
        Color::srgb(0.10, 0.16, 0.26)
    }
}

fn note_height_pct(duration: f64) -> f32 {
    ((duration / LOOKAHEAD) as f32 * 100.0).clamp(3.5, 40.0)
}

fn spawn_12_bar_grid(
    grid: &mut ChildSpawnerCommands,
    chords: &[String],
    key: &str,
    font: &FontSource,
) {
    for row in 0..3usize {
        grid.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|r| {
            for col in 0..4usize {
                let idx = row * 4 + col;
                let chord = chords[idx].clone();
                r.spawn((
                    Node {
                        width: Val::Vw(6.5),
                        height: Val::Vh(5.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(bar_bg(idx, key)),
                    BorderColor::all(Color::srgb(0.25, 0.25, 0.38)),
                    super::BarCell(idx),
                ))
                .with_children(|cell| {
                    cell.spawn((
                        Text::new(chord),
                        TextFont { font_size: FontSize::Px(17.0), font: font.clone(), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    cell.spawn((
                        Text::new(format!("{}", idx + 1)),
                        TextFont { font_size: FontSize::Px(9.0), font: font.clone(), ..default() },
                        TextColor(Color::srgb(0.45, 0.45, 0.55)),
                    ));
                });
            }
        });
    }
}

fn spawn_highway(hw: &mut ChildSpawnerCommands, font: &FontSource, chart: &crate::song::chart::HarpChart) {
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
        let t = item.time.unwrap_or(0.0);
        let h_pct = note_height_pct(item.duration);
        for event in &item.events {
            let is_blow = matches!(event.action, Action::Blow);
            let (r, g, b) = if is_blow { (0.25f32, 0.55, 0.95) } else { (0.95f32, 0.38, 0.15) };
            let left_pct = (event.hole as f32 - 1.0) * LANE_PCT + 0.3;
            hw.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    top: Val::Percent(-h_pct),
                    width: Val::Percent(LANE_PCT - 0.6),
                    height: Val::Percent(h_pct),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(r, g, b, 0.88)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.50)),
                NoteVisual { time: t, height_pct: h_pct },
            ))
            .with_children(|note| {
                note.spawn((
                    Text::new(if is_blow { "\u{2191}" } else { "\u{2193}" }),
                    TextFont { font_size: FontSize::Px(12.0), font: font.clone(), ..default() },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                ));
            });
        }
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
            let b = blow_label(hole, chart);
            let d = draw_label(hole, chart);
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

pub fn update_countdown(
    clock: Res<super::GameplayClock>,
    mut overlay: Query<&mut Visibility, With<CountdownOverlay>>,
    mut text: Query<(&mut Text, &mut TextFont), With<CountdownText>>,
    mut music_started: ResMut<MusicStarted>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut commands: Commands,
) {
    if clock.0 >= 0.0 {
        for mut vis in &mut overlay {
            *vis = Visibility::Hidden;
        }
        if !music_started.0 {
            music_started.0 = true;
            if let Some(manifest) = manifests.get(&selected.0) {
                commands.spawn((
                    AudioPlayer::<AudioSource>(manifest.music.clone()),
                    PlaybackSettings::ONCE,
                ));
            }
        }
        return;
    }

    for mut vis in &mut overlay {
        *vis = Visibility::Visible;
    }

    let remaining = -clock.0;
    let n = remaining.ceil() as u32;
    let frac = remaining.fract() as f32;
    let font_size = 80.0 + (1.0 - frac) * 80.0;

    for (mut t, mut font) in &mut text {
        t.0 = format!("{n}");
        font.font_size = FontSize::Px(font_size);
    }
}

pub fn update_notes(
    clock: Res<super::GameplayClock>,
    mut notes: Query<(&NoteVisual, &mut Node)>,
) {
    let elapsed = clock.0;
    for (note, mut node) in &mut notes {
        let remaining = note.time - elapsed;
        let progress = 1.0 - (remaining / LOOKAHEAD) as f32;
        let hit_center_pct = 100.0 - HIT_H_PCT * 0.5;
        let top_pct = hit_center_pct * progress - note.height_pct;
        node.top = Val::Percent(top_pct);
    }
}

pub fn update_bar(
    clock: Res<super::GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut cells: Query<(&super::BarCell, &mut BackgroundColor)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let bpm = manifest.chart.song.tempo_bpm as f64;
    let beats = manifest
        .chart
        .song
        .time_signature
        .as_deref()
        .and_then(|s| s.split('/').next())
        .and_then(|n| n.parse::<f64>().ok())
        .unwrap_or(4.0);
    let secs_per_bar = (60.0 / bpm) * beats;
    let current = (clock.0.max(0.0) / secs_per_bar) as usize % 12;
    let key = manifest.chart.song.key.as_str();

    for (cell, mut bg) in &mut cells {
        *bg = if cell.0 == current {
            BackgroundColor(Color::srgb(0.75, 0.55, 0.08))
        } else {
            BackgroundColor(bar_bg(cell.0, key))
        };
    }
}

pub fn update_holes(
    time: Res<Time>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut cells: Query<(&HoleCell, &mut BackgroundColor, &mut HoleState)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let chart = &manifest.chart;
    let dt = time.delta_secs();

    let attack = 1.0 - (-dt * 25.0_f32).exp();
    let decay  = 1.0 - (-dt *  4.0_f32).exp();

    let harp_pitches: Vec<&crate::pitch_detect::PitchInfo> = active
        .0
        .iter()
        .filter(|p| valid_notes.0.contains(&format!("{}{}", p.note, p.octave)))
        .collect();

    for (cell, mut bg, mut state) in &mut cells {
        let blow = blow_label(cell.0, chart);
        let draw = draw_label(cell.0, chart);

        let mut blow_hit = false;
        let mut draw_hit = false;
        for p in &harp_pitches {
            let name = format!("{}{}", p.note, p.octave);
            if name == blow { blow_hit = true; }
            if name == draw { draw_hit = true; }
        }

        let (target, is_blow) = if blow_hit {
            (1.0f32, true)
        } else if draw_hit {
            (1.0f32, false)
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
