// SPDX-License-Identifier: MIT

use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use crate::{
    assets_management::GlobalFonts,
    menu::SelectedSong,
    song::SongManifest,
    song::chart::{Action, Modifier, PlayMode},
    song::harmonica::twelve_bar,
    song::NoteHeadRect,
    song::NoteThemeConfig,
};

use super::countdown_overlay::spawn_countdown;
use super::metronome_overlay::spawn_metronome;
use super::modifier_legend::{build_legend_materials, spawn_modifier_legend};
use super::note_tail_2d::{NoteTail2dMaterial, tail_params};
use super::note_visual_2d::{NoteChildConfig, spawn_note_children};
use super::phrase_overlay::spawn_phrase_banner;
use super::song_progress_overlay::spawn_song_progress;
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, ComboText, FeedbackText, GameplayRoot, HIT_H_PCT,
    HOLE_COUNT, HoleCell, HoleState, LANE_PCT, LOOKAHEAD, MusicStarted, NoteVisual, ScheduledNote,
    ScoreText, ValidHarpNotes,
};

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut valid_notes: ResMut<ValidHarpNotes>,
    mut shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
    fonts: Res<GlobalFonts>,
    note_theme: Res<crate::assets_management::SelectedNoteTheme2d>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing state");
        return;
    };

    // Comet head: the disc image (white interior tinted per note, black rim
    // kept), paired with its tail layout. We resolve only the *path* — the
    // song's own image if it ships one, else the selected theme's default — and
    // the head's `bsn!` scene loads it. The image frees when the note entities
    // despawn on leaving the song.
    let head_image: AssetPath<'static> = match &manifest.assets_2d {
        Some(path) => path.clone().into(),
        None => format!("notes/2d/{}.png", note_theme.0).into(),
    };

    let tail_cfg = manifest.assets_2d_config.clone();

    clock.0 = -COUNTDOWN;
    music_started.0 = false;
    valid_notes.0 = manifest.chart.harmonica.build_valid_notes();

    let chart = &manifest.chart;

    // Pre-build one comet-tail shader material per note (in the same flat order
    // `spawn_highway` walks the track), so the highway closures only need a shared
    // slice — no nested mutable borrow of the asset store. The technique modifier
    // shapes the tail *and* picks its animation (`wah.z`); each note gets a
    // distinct phase so same-technique tails don't pulse in lockstep. `params.z`
    // is the animation clock, driven live by `animate_note_tails`.
    let note_materials: Vec<Handle<NoteTail2dMaterial>> = chart
        .track
        .iter()
        .flat_map(|item| {
            let h_pct = note_height_pct(item.duration);
            item.events.iter().map(move |event| {
                let (vib, bend, wah) = note_techniques(event.modifiers.as_deref());
                let mode = note_anim_mode(event.modifiers.as_deref());
                let (r, g, b) = note_rgb(matches!(event.action, Action::Blow));
                (h_pct, vib, bend, wah, mode, Color::srgba(r, g, b, 0.95))
            })
        })
        .enumerate()
        .map(|(i, (h_pct, vib, bend, wah, mode, color))| {
            let (mut params, mut wah_v) = tail_params(h_pct, vib, bend, wah);
            params.z = 0.0; // animation time, refreshed every frame
            wah_v.z = mode; // which technique animation to run
            wah_v.w = i as f32 * 0.7; // per-note phase offset
            shape_materials.add(NoteTail2dMaterial {
                color: color.to_linear(),
                params,
                wah: wah_v,
            })
        })
        .collect();

    // Animated tail previews for the techniques legend (built up front so the UI
    // closures only borrow a ready slice, not the material store).
    let legend_materials = build_legend_materials(&mut shape_materials);

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
        ts.split('/')
            .next()
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(4)
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
                    NoteHighway,
                ))
                .with_children(|hw| {
                    spawn_highway(
                        hw,
                        &fonts.symbols,
                        chart,
                        &note_materials,
                        &head_image,
                        &tail_cfg,
                    );
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
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn((
                            Text::new(title),
                            TextFont {
                                font_size: FontSize::Px(18.0),
                                font: fonts.gameplay.clone(),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                        col.spawn((
                            Text::new(info),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                font: fonts.gameplay.clone(),
                                ..default()
                            },
                            TextColor(Color::srgb(0.60, 0.65, 0.75)),
                        ));
                        col.spawn((
                            Text::new(harp_info),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                font: fonts.gameplay.clone(),
                                ..default()
                            },
                            TextColor(Color::srgb(0.45, 0.72, 0.55)),
                        ));
                        if let Some(desc) = description {
                            col.spawn((
                                Text::new(desc.to_string()),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    font: fonts.gameplay.clone(),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.50, 0.50, 0.55)),
                            ));
                        }
                        if let Some(author) = chart_author {
                            col.spawn((
                                Text::new(format!("Chart: {author}")),
                                TextFont {
                                    font_size: FontSize::Px(9.0),
                                    font: fonts.gameplay.clone(),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.40, 0.40, 0.45)),
                            ));
                        }
                    });

                // Live phrase / groove banner (driven by phrase_overlay::update_phrase)
                spawn_phrase_banner(right, &fonts.gameplay);

                // 12-bar blues grid
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    })
                    .with_children(|grid| {
                        spawn_12_bar_grid(
                            grid,
                            &chords,
                            key,
                            &fonts.gameplay,
                            &GridConfig::for_2d(),
                        );
                    });

                // Metronome
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|metro| {
                        spawn_metronome(metro, beats_per_bar, bpm, &fonts.gameplay);
                    });

                // Technique colour legend
                spawn_modifier_legend(right, &fonts.gameplay, &legend_materials);

                // Score
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|p| {
                        p.spawn((
                            Text::new("0"),
                            TextFont {
                                font_size: FontSize::Px(28.0),
                                font: fonts.gameplay.clone(),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            ScoreText,
                        ));
                        p.spawn((
                            Text::new(""),
                            TextFont {
                                font_size: FontSize::Px(14.0),
                                font: fonts.gameplay.clone(),
                                ..default()
                            },
                            TextColor(Color::srgb(0.90, 0.72, 0.20)),
                            ComboText,
                        ));
                        p.spawn((
                            Text::new(""),
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                font: fonts.gameplay.clone(),
                                ..default()
                            },
                            TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                            FeedbackText,
                        ));
                    });
            });
        });
    spawn_song_progress(&mut commands);
    spawn_countdown(&mut commands, &fonts.gameplay);
}

fn note_height_pct(duration: f64) -> f32 {
    ((duration / LOOKAHEAD) as f32 * 100.0).clamp(3.5, 40.0)
}

/// The note highway (the clipping container notes scroll inside). Marked so the
/// recycle logic can measure its height and convert head-heights to a fraction.
#[derive(Component)]
pub(super) struct NoteHighway;

/// The round comet head (an `ImageNode`), child of a note. Tinted on hit/miss.
#[derive(Component)]
pub(super) struct NoteHead;

/// The comet tail (a shader `MaterialNode`), child of a note. Tinted on hit/miss
/// and sized each frame to be time-accurate. Carries the note's duration as a
/// fraction of `LOOKAHEAD` so `size_note_tails` can length it correctly.
#[derive(Component)]
pub(super) struct NoteTail {
    duration_frac: f32,
}


/// The highway distance (in %) a note scrolls from entering at the top to its
/// head reaching the hit line — i.e. the span covered in `LOOKAHEAD` seconds.
/// A tail representing `duration` seconds is `SCROLL_SPAN * duration / LOOKAHEAD`
/// percent long, which is what makes the tail time-accurate.
const SCROLL_SPAN: f32 = 100.0 - HIT_H_PCT * 0.5;

/// Which tail animation a note runs, picked from its (first) technique modifier
/// and passed to the shader as `wah.z`. Plain notes get the gentle default flow.
/// Indices must match the `mode` branches in the note-tail shaders (2D + 3D).
pub(super) fn note_anim_mode(modifiers: Option<&[Modifier]>) -> f32 {
    match modifiers.and_then(|m| m.first()) {
        Some(Modifier::Bend { .. }) => 1.0,
        Some(Modifier::Vibrato { .. }) => 2.0,
        Some(Modifier::WahWah { .. }) => 3.0,
        Some(Modifier::Overblow) => 4.0,
        Some(Modifier::Overdraw) => 5.0,
        None => 0.0,
    }
}

/// Distance (in %) from the bottom of the highway to a note head's bottom edge.
/// The head's bottom reaches the hit line exactly at `note_time`; it decreases
/// as the note falls, going negative once the head drops past the hit line.
pub fn note_head_bottom_pct(note_time: f64, elapsed: f64, lookahead: f64) -> f32 {
    let progress = 1.0 - (note_time - elapsed) / lookahead;
    let hit_center_pct = 100.0 - (HIT_H_PCT as f64) * 0.5;
    (100.0 - hit_center_pct * progress) as f32
}

fn spawn_highway(
    hw: &mut ChildSpawnerCommands,
    font: &FontSource,
    chart: &crate::song::chart::HarpChart,
    note_materials: &[Handle<NoteTail2dMaterial>],
    head_image: &AssetPath<'static>,
    tail_cfg: &NoteThemeConfig,
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

    let mut note_idx = 0usize;
    for item in &chart.track {
        let t = super::resolve_item_time(item, &chart.timing);
        let play_mode = item.play_mode.as_ref();
        for event in &item.events {
            let idx = note_idx;
            note_idx += 1;

            let is_blow = matches!(event.action, Action::Blow);
            let (r, g, b) = note_rgb(is_blow);
            let note_color = Color::srgba(r, g, b, 1.0);
            let left_pct = (event.hole as f32 - 1.0) * LANE_PCT;
            let modifiers = event.modifiers.clone().unwrap_or_default();
            let natural_pitch = event.note.clone().unwrap_or_else(|| {
                chart
                    .harmonica
                    .wind_direction_label(event.hole, &event.action)
            });
            // The pitch the player must actually produce — a bend targets the bent
            // pitch, so the technique is scored, not just shown.
            let expected_pitch = super::target_pitch(&natural_pitch, &modifiers);
            // The note entity IS the comet head: a lane-width square (kept round
            // by the disc image + aspect_ratio), positioned each frame by its
            // bottom edge so it reaches the hit line on time. The tail hangs off
            // the head's attach point (from the theme JSON) and trails upward; it
            // is drawn first so the head image layers over its base and hides the
            // join.
            let material = note_materials[idx].clone();
            let duration_frac = (item.duration / LOOKAHEAD) as f32;

            hw.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    bottom: Val::Percent(150.0), // placeholder; set in update_notes
                    width: Val::Percent(LANE_PCT),
                    aspect_ratio: Some(1.0),
                    ..default()
                },
                NoteVisual {
                    time: t,
                    duration_frac,
                },
                ScheduledNote {
                    time: t,
                    duration: item.duration,
                    hole: event.hole,
                    is_blow,
                    expected_pitch,
                    hit: false,
                    missed: false,
                    held: 0.0,
                    sustain_scored: false,
                    modifiers: modifiers.clone(),
                },
            ))
            .with_children(|note| {
                // Tail + head layout shared with the note_editor binary via
                // note_visual_2d::spawn_note_children. Game-specific markers and
                // the direction arrow are added in the callbacks.
                spawn_note_children(
                    note,
                    &NoteChildConfig {
                        tail_x: tail_cfg.tail_x,
                        tail_y: tail_cfg.tail_y,
                        tail_width: tail_cfg.tail_width,
                        // Placeholder height; resized each frame by size_note_tails.
                        tail_height: Val::Percent(100.0),
                        tail_material: material,
                        head_image: head_image.clone(),
                        head_color: note_color,
                        head_left: tail_cfg.head.x,
                        head_top: tail_cfg.head.y,
                        head_width: tail_cfg.head.width,
                        head_height: tail_cfg.head.height,
                    },
                    |cmd| { cmd.insert(NoteTail { duration_frac }); },
                    |cmd| {
                        cmd.insert(NoteHead).with_children(|head| {
                            head.spawn((
                                Text::new(if is_blow { "\u{2191}" } else { "\u{2193}" }),
                                TextFont {
                                    font_size: FontSize::Px(13.0),
                                    font: font.clone(),
                                    ..default()
                                },
                                TextColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
                            ));
                        });
                    },
                );

                // Chord / split play-mode badge, pinned to the bottom edge.
                if let Some(tag) = play_mode_label(play_mode) {
                    note.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(1.0),
                            ..default()
                        },
                        Text::new(tag),
                        TextFont {
                            font_size: FontSize::Px(8.0),
                            ..default()
                        },
                        TextColor(Color::srgba(0.95, 0.95, 1.0, 0.75)),
                    ));
                }
            });
        }
    }
}

/// Extracts the shape-driving techniques from a note's modifiers:
/// `(vibrato_intensity, pitch_shift_semitones, wah_intensity)`. The pitch shift is
/// negative for bends (pitch down) and positive for overblow/overdraw (pitch up);
/// its sign drives the arc direction and its magnitude the arc depth. Wah pulses
/// the note width. Any may be absent.
pub(super) fn note_techniques(
    modifiers: Option<&[Modifier]>,
) -> (Option<f32>, Option<f32>, Option<f32>) {
    let mut vib = None;
    let mut shift = None;
    let mut wah = None;
    for m in modifiers.unwrap_or(&[]) {
        match m {
            Modifier::Vibrato { intensity, .. } => vib = Some(intensity.unwrap_or(0.5)),
            Modifier::WahWah { intensity, .. } => wah = Some(intensity.unwrap_or(0.5)),
            Modifier::Bend { semitones, .. } => shift = Some(*semitones),
            // Overblow/overdraw raise pitch ~1 semitone — represent as an up-bend.
            Modifier::Overblow | Modifier::Overdraw => shift = Some(1.0),
        }
    }
    (vib, shift, wah)
}

/// Blow/draw fill colour for a note tile (blue for blow, orange for draw).
fn note_rgb(is_blow: bool) -> (f32, f32, f32) {
    if is_blow {
        (0.25, 0.55, 0.95)
    } else {
        (0.95, 0.38, 0.15)
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
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        font: font.clone(),
                        ..default()
                    },
                    TextColor(Color::srgb(0.50, 0.75, 1.00)),
                ));
                cell.spawn((
                    Text::new(format!("{hole}")),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        font: font.clone(),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                cell.spawn((
                    Text::new(d),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        font: font.clone(),
                        ..default()
                    },
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
            TextFont {
                font_size: FontSize::Px(11.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::srgb(0.50, 0.75, 1.00)),
        ));
        leg.spawn((
            Text::new("\u{25A0} DRAW"),
            TextFont {
                font_size: FontSize::Px(11.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::srgb(1.00, 0.62, 0.35)),
        ));
    });
}

// ── Per-frame systems ─────────────────────────────────────────────────────────

pub fn update_notes(
    clock: Res<super::GameplayClock>,
    loop_cfg: Res<super::LoopConfig>,
    mut commands: Commands,
    mut notes: Query<(Entity, &NoteVisual, &mut Node)>,
) {
    let elapsed = clock.0;
    for (entity, note, mut node) in &mut notes {
        let bottom = note_head_bottom_pct(note.time, elapsed, LOOKAHEAD);

        // Recycle only once the whole comet has fallen past the bottom. The tail
        // tip sits `SCROLL_SPAN * duration_frac` % above the head, so a long note
        // lingers exactly as long as its tail needs. Not while looping.
        let tail_pct = SCROLL_SPAN * note.duration_frac;
        if !loop_cfg.active && bottom < -(tail_pct + 15.0) {
            commands.entity(entity).despawn();
            continue;
        }
        node.bottom = Val::Percent(bottom);
    }
}

/// Lengths every comet tail to be time-accurate: its tip meets the hit line at
/// the note's end. The on-screen length is the highway distance scrolled during
/// the note's duration, so it's measured against the live highway height.
pub fn size_note_tails(
    highway: Query<&ComputedNode, With<NoteHighway>>,
    mut tails: Query<(&NoteTail, &mut Node)>,
) {
    let Some(hw) = highway.iter().next() else {
        return;
    };
    let height_px = hw.size().y;
    if height_px <= 0.0 {
        return;
    }
    // ComputedNode sizes are physical px; Node lengths are logical px.
    let logical = height_px * hw.inverse_scale_factor();
    for (tail, mut node) in &mut tails {
        let len = (SCROLL_SPAN / 100.0) * tail.duration_frac * logical;
        node.height = Val::Px(len.max(1.0));
    }
}

/// Tints a note's head image and tail material when it is hit or missed. Mirrors
/// the 3D path: a hit flashes gold, a miss dims to red so a whiff never looks
/// like a clean hit. Reacts only to `ScheduledNote` changes (set by scoring), so
/// it runs the frame a note's outcome lands, not every frame.
pub fn update_note_visuals(
    notes: Query<(&ScheduledNote, &Children), Changed<ScheduledNote>>,
    mut heads: Query<&mut ImageNode, With<NoteHead>>,
    tails: Query<&MaterialNode<NoteTail2dMaterial>, With<NoteTail>>,
    mut shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
) {
    for (scheduled, children) in &notes {
        let tint = if scheduled.hit {
            Color::srgba(1.0, 0.85, 0.25, 1.0)
        } else if scheduled.missed {
            Color::srgba(0.5, 0.13, 0.13, 1.0)
        } else {
            continue;
        };
        for child in children {
            if let Ok(mut head) = heads.get_mut(*child) {
                head.color = tint;
            }
            if let Ok(tail) = tails.get(*child)
                && let Some(mut material) = shape_materials.get_mut(&tail.0)
            {
                material.color = tint.to_linear();
            }
        }
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

    let harp_pitches: Vec<&crate::audio_system::pitch_detect::PitchInfo> = active
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

        let factor = if target > state.brightness {
            attack
        } else {
            decay
        };
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
    fn head_bottom_at_hit_line_on_time() {
        // At the note's time, the head's bottom sits at the hit-line center.
        let expected = HIT_H_PCT * 0.5;
        let got = note_head_bottom_pct(1.0, 1.0, LOOKAHEAD);
        assert!((got - expected).abs() < 0.01, "got {got}");
    }

    #[test]
    fn head_bottom_high_in_the_future() {
        // A note a full lookahead away enters at the top of the highway.
        let got = note_head_bottom_pct(LOOKAHEAD, 0.0, LOOKAHEAD);
        assert!((got - 100.0).abs() < 0.01, "got {got}");
    }

    #[test]
    fn head_bottom_descends_over_time() {
        let b0 = note_head_bottom_pct(2.0, 0.0, LOOKAHEAD);
        let b1 = note_head_bottom_pct(2.0, 1.0, LOOKAHEAD);
        assert!(
            b1 < b0,
            "head should fall (smaller bottom%) as time advances"
        );
    }

    #[test]
    fn head_bottom_goes_negative_past_the_line() {
        // Well after its time, the head has dropped below the hit line.
        let got = note_head_bottom_pct(0.0, 3.0, LOOKAHEAD);
        assert!(got < 0.0, "got {got}");
    }

    // ── play_mode_label ───────────────────────────────────────────────────────

    #[test]
    fn play_mode_label_badges_only_multi_note_modes() {
        assert_eq!(play_mode_label(Some(&PlayMode::Chord)), Some("chord"));
        assert_eq!(play_mode_label(Some(&PlayMode::Split)), Some("split"));
        assert_eq!(play_mode_label(Some(&PlayMode::Single)), None);
        assert_eq!(play_mode_label(None), None);
    }

    // ── note_techniques ───────────────────────────────────────────────────────

    #[test]
    fn techniques_extract_each_dimension() {
        let mods = [
            Modifier::Vibrato {
                oscillation_hz: 5.0,
                intensity: Some(0.8),
            },
            Modifier::Bend {
                semitones: -2.0,
                intensity: None,
            },
            Modifier::WahWah {
                oscillation_hz: 3.0,
                intensity: Some(0.4),
            },
        ];
        let (vib, shift, wah) = note_techniques(Some(&mods));
        assert_eq!(vib, Some(0.8));
        assert_eq!(shift, Some(-2.0));
        assert_eq!(wah, Some(0.4));
    }

    #[test]
    fn techniques_default_intensity_when_omitted() {
        let (vib, _, _) = note_techniques(Some(&[Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        }]));
        assert_eq!(vib, Some(0.5));
    }

    #[test]
    fn overblow_overdraw_read_as_an_up_shift() {
        assert_eq!(note_techniques(Some(&[Modifier::Overblow])).1, Some(1.0));
        assert_eq!(note_techniques(Some(&[Modifier::Overdraw])).1, Some(1.0));
    }

    #[test]
    fn no_modifiers_yield_no_techniques() {
        assert_eq!(note_techniques(None), (None, None, None));
        assert_eq!(note_techniques(Some(&[])), (None, None, None));
    }

    // ── note_anim_mode ────────────────────────────────────────────────────────

    #[test]
    fn anim_mode_maps_each_technique() {
        assert_eq!(note_anim_mode(None), 0.0);
        assert_eq!(
            note_anim_mode(Some(&[Modifier::Bend {
                semitones: -1.0,
                intensity: None
            }])),
            1.0
        );
        assert_eq!(
            note_anim_mode(Some(&[Modifier::Vibrato {
                oscillation_hz: 5.0,
                intensity: None
            }])),
            2.0
        );
        assert_eq!(
            note_anim_mode(Some(&[Modifier::WahWah {
                oscillation_hz: 3.0,
                intensity: None
            }])),
            3.0
        );
        assert_eq!(note_anim_mode(Some(&[Modifier::Overblow])), 4.0);
        assert_eq!(note_anim_mode(Some(&[Modifier::Overdraw])), 5.0);
    }

    #[test]
    fn anim_mode_uses_the_first_modifier() {
        let mods = [
            Modifier::WahWah {
                oscillation_hz: 3.0,
                intensity: None,
            },
            Modifier::Bend {
                semitones: -1.0,
                intensity: None,
            },
        ];
        assert_eq!(note_anim_mode(Some(&mods)), 3.0);
    }

    // ── note_rgb ──────────────────────────────────────────────────────────────

    #[test]
    fn blow_and_draw_have_distinct_colors() {
        assert_ne!(note_rgb(true), note_rgb(false));
    }
}
