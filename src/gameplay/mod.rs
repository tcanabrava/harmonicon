use bevy::{audio::AudioSource, prelude::*};

use crate::{
    menu::{AppState, SelectedSong},
    pitch_detect::{PitchEvent, PitchInfo},
    song::{
        chart::{Action, HarpChart, Harmonica},
        SongManifest,
    },
};

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameplayClock>()
            .init_resource::<ActivePitches>()
            .init_resource::<MusicStarted>()
            .add_systems(OnEnter(AppState::Playing), setup_gameplay)
            .add_systems(OnExit(AppState::Playing), cleanup_gameplay)
            .add_systems(
                Update,
                (
                    tick_clock,
                    collect_pitches,
                    update_countdown,
                    update_notes,
                    update_bar,
                    update_holes,
                )
                    .chain()
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

#[derive(Resource, Default)]
struct GameplayClock(f64);

#[derive(Resource, Default)]
struct ActivePitches(Vec<PitchInfo>);

#[derive(Component)]
struct GameplayRoot;

// height_pct is a percentage of the highway height (0–100)
#[derive(Component)]
struct NoteVisual {
    time: f64,
    height_pct: f32,
}

#[derive(Component)]
struct BarCell(usize);

#[derive(Component)]
struct HoleCell(u8);

#[derive(Component)]
struct CountdownOverlay;

#[derive(Component)]
struct CountdownText;

#[derive(Resource, Default)]
struct MusicStarted(bool);

const HOLE_COUNT: usize = 10;
const COUNTDOWN: f64 = 3.0;
const LANE_PCT: f32 = 100.0 / HOLE_COUNT as f32; // 10 % per lane
const HIT_H_PCT: f32 = 7.0; // hit zone height as % of highway
const LOOKAHEAD: f64 = 3.0;

fn note_height_pct(duration: f64) -> f32 {
    ((duration / LOOKAHEAD) as f32 * 100.0).clamp(3.5, 40.0)
}

fn twelve_bar(key: &str) -> [String; 12] {
    let iv = semitone(key, 5);
    let v = semitone(key, 7);
    [
        key.into(), key.into(), key.into(), key.into(),
        iv.clone(), iv.clone(), key.into(), key.into(),
        v.clone(),  iv.clone(), key.into(), v.clone(),
    ]
}

fn semitone(root: &str, n: i32) -> String {
    const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let i = NOTES.iter().position(|&x| x == root).unwrap_or(0);
    NOTES[((i as i32 + n).rem_euclid(12)) as usize].to_string()
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

fn blow_label(hole: u8, chart: &HarpChart) -> String {
    if let Harmonica::Diatonic { layout: Some(ref l), .. } = chart.harmonica {
        if let Some(ref notes) = l.blow {
            if let Some(n) = notes.get(hole as usize - 1) {
                return n.clone();
            }
        }
    }
    "\u{2014}".into()
}

fn draw_label(hole: u8, chart: &HarpChart) -> String {
    if let Harmonica::Diatonic { layout: Some(ref l), .. } = chart.harmonica {
        if let Some(ref notes) = l.draw {
            if let Some(n) = notes.get(hole as usize - 1) {
                return n.clone();
            }
        }
    }
    "\u{2014}".into()
}

fn setup_gameplay(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing state");
        return;
    };
    // Start the clock negative so notes are already visible during the
    // countdown and music begins exactly when the clock reaches 0.
    clock.0 = -COUNTDOWN;
    music_started.0 = false;

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
            BackgroundColor(Color::srgb(0.04, 0.04, 0.06)),
            GameplayRoot,
        ))
        .with_children(|root| {
            // ── Title / info ──────────────────────────────────────────────
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    Text::new(title),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::WHITE),
                ));
                col.spawn((
                    Text::new(info),
                    TextFont { font_size: 13.0, ..default() },
                    TextColor(Color::srgb(0.60, 0.65, 0.75)),
                ));
            });

            // ── 12-bar blues grid (4 × 3) ─────────────────────────────────
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|grid| {
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
                                BarCell(idx),
                            ))
                            .with_children(|cell| {
                                cell.spawn((
                                    Text::new(chord),
                                    TextFont { font_size: 17.0, ..default() },
                                    TextColor(Color::WHITE),
                                ));
                                cell.spawn((
                                    Text::new(format!("{}", idx + 1)),
                                    TextFont { font_size: 9.0, ..default() },
                                    TextColor(Color::srgb(0.45, 0.45, 0.55)),
                                ));
                            });
                        }
                    });
                }
            });

            // ── Note highway ──────────────────────────────────────────────
            // flex_grow: 1 fills all remaining vertical space between the
            // bar grid above and the harmonica strip below.
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
                // alternating lane shading + dividers
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
                    // lane divider line (skip leftmost)
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

                // hit zone fill
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
                // hit zone top edge (bright line)
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

                // note visuals
                for item in &chart.track {
                    let t = item.time.unwrap_or(0.0);
                    let h_pct = note_height_pct(item.duration);
                    for event in &item.events {
                        let is_blow = matches!(event.action, Action::Blow);
                        let (r, g, b) = if is_blow {
                            (0.25f32, 0.55, 0.95)
                        } else {
                            (0.95f32, 0.38, 0.15)
                        };
                        let left_pct = (event.hole as f32 - 1.0) * LANE_PCT + 0.3;
                        hw.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(left_pct),
                                top: Val::Percent(-h_pct), // off-screen until first update
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
                        .with_children(|note_node| {
                            note_node.spawn((
                                Text::new(if is_blow { "\u{2191}" } else { "\u{2193}" }),
                                TextFont { font_size: 12.0, ..default() },
                                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                            ));
                        });
                    }
                }
            });

            // ── Harmonica holes ───────────────────────────────────────────
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                width: Val::Percent(80.0),
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|harp_col| {
                // hole cells — each takes exactly 1/10 of the full width
                harp_col.spawn(Node {
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
                        ))
                        .with_children(|cell| {
                            cell.spawn((
                                Text::new(b),
                                TextFont { font_size: 11.0, ..default() },
                                TextColor(Color::srgb(0.50, 0.75, 1.00)),
                            ));
                            cell.spawn((
                                Text::new(format!("{hole}")),
                                TextFont { font_size: 16.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                            cell.spawn((
                                Text::new(d),
                                TextFont { font_size: 11.0, ..default() },
                                TextColor(Color::srgb(1.00, 0.62, 0.35)),
                            ));
                        });
                    }
                });

                // blow/draw colour legend
                harp_col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(20.0),
                    ..default()
                })
                .with_children(|leg| {
                    leg.spawn((
                        Text::new("\u{25A0} BLOW"),
                        TextFont { font_size: 11.0, ..default() },
                        TextColor(Color::srgb(0.50, 0.75, 1.00)),
                    ));
                    leg.spawn((
                        Text::new("\u{25A0} DRAW"),
                        TextFont { font_size: 11.0, ..default() },
                        TextColor(Color::srgb(1.00, 0.62, 0.35)),
                    ));
                });
            });

            // ── Countdown overlay (covers the whole gameplay area) ────────
            // Absolute positioning + high GlobalZIndex keeps it on top of
            // the note highway while remaining semi-transparent so the player
            // can see the first notes already approaching.
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
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgba(0.85, 0.85, 1.0, 0.80)),
                ));
                ov.spawn((
                    Text::new("3"),
                    TextFont { font_size: 120.0, ..default() },
                    TextColor(Color::WHITE),
                    CountdownText,
                ));
            });
        });
}

fn update_countdown(
    clock: Res<GameplayClock>,
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

    // frac goes 0 → 1 over each second; font pulses large → normal
    let remaining = -clock.0; // positive, counts down
    let n = remaining.ceil() as u32;
    let frac = remaining.fract() as f32; // 0 = just changed, 1 = about to change
    let font_size = 80.0 + (1.0 - frac) * 80.0; // 160 → 80 px over each second

    for (mut t, mut font) in &mut text {
        t.0 = format!("{n}");
        font.font_size = font_size;
    }
}

fn cleanup_gameplay(mut commands: Commands, roots: Query<Entity, With<GameplayRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

fn tick_clock(mut clock: ResMut<GameplayClock>, time: Res<Time>) {
    clock.0 += time.delta_secs_f64();
}

fn collect_pitches(mut reader: MessageReader<PitchEvent>, mut active: ResMut<ActivePitches>) {
    for ev in reader.read() {
        active.0 = ev.0.clone();
    }
}

fn update_notes(clock: Res<GameplayClock>, mut notes: Query<(&NoteVisual, &mut Node)>) {
    let elapsed = clock.0;
    for (note, mut node) in &mut notes {
        let remaining = note.time - elapsed;
        let progress = 1.0 - (remaining / LOOKAHEAD) as f32;
        // At progress=1 the bottom of the note aligns with the hit zone centre.
        // Val::Percent is relative to the highway's computed height, so this
        // automatically scales as the window is resized.
        let hit_center_pct = 100.0 - HIT_H_PCT * 0.5;
        let top_pct = hit_center_pct * progress - note.height_pct;
        node.top = Val::Percent(top_pct);
    }
}

fn update_bar(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut cells: Query<(&BarCell, &mut BackgroundColor)>,
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
    let current = (clock.0 / secs_per_bar) as usize % 12;
    let key = manifest.chart.song.key.as_str();

    for (cell, mut bg) in &mut cells {
        *bg = if cell.0 == current {
            BackgroundColor(Color::srgb(0.75, 0.55, 0.08))
        } else {
            BackgroundColor(bar_bg(cell.0, key))
        };
    }
}

fn update_holes(
    active: Res<ActivePitches>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut cells: Query<(&HoleCell, &mut BackgroundColor)>,
) {
    let Some(manifest) = manifests.get(&selected.0) else { return };
    let chart = &manifest.chart;

    for (cell, mut bg) in &mut cells {
        let b = blow_label(cell.0, chart);
        let d = draw_label(cell.0, chart);
        let hit = active.0.iter().any(|p| {
            let name = format!("{}{}", p.note, p.octave);
            name == b || name == d
        });
        *bg = if hit {
            BackgroundColor(Color::srgb(0.15, 0.85, 0.35))
        } else {
            BackgroundColor(Color::srgb(0.10, 0.12, 0.16))
        };
    }
}
