// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use crate::{
    menu::SelectedSong,
    song::NoteThemeConfig,
    song::SongManifest,
    song::chart::{Action, Modifier, PlayMode},
    song::harmonica::twelve_bar,
};
use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use super::countdown_overlay::spawn_countdown;
use super::metronome_overlay::spawn_metronome;
use super::modifier_legend::{build_legend_materials, spawn_modifier_legend};
use super::note_tail_2d::{NoteTail2dMaterial, tail_params};
use super::note_visual_2d::{NoteChildConfig, spawn_note_children};
use super::adaptive_difficulty::{AdaptiveDifficulty, track_items, unlocked_flags};
use super::phrase_overlay::{spawn_phrase_banner, spawn_tab_ribbon};
use super::song_progress_overlay::{BAR_HEIGHT, spawn_song_progress};
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, ComboText, FeedbackText, GameplayRoot, HIT_H_PCT,
    HoleCell, HoleState, LOOKAHEAD, MusicStarted, NoteVisual, ScheduledNote, ScoreText, SongNotes,
    ValidHarpNotes,
};

/// Chart-level (not per-note) rendering config `spawn_visible_notes` needs
/// once a note's `LOOKAHEAD` window arrives — set once at song load
/// alongside `SongNotes`, since neither changes for the rest of the song.
#[derive(Resource, Default)]
pub(super) struct NoteRenderAssets {
    head_image: Option<AssetPath<'static>>,
    tail_cfg: Option<NoteThemeConfig>,
    /// Chord/split play-mode badge text, parallel to `SongNotes::notes`
    /// (same index = same note) — the one piece of per-note render data that
    /// doesn't already live on `ScheduledNote` itself.
    play_mode_tags: Vec<Option<&'static str>>,
}

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut valid_notes: ResMut<ValidHarpNotes>,
    mut song_notes: ResMut<SongNotes>,
    mut render_assets: ResMut<NoteRenderAssets>,
    mut shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
    note_theme: Res<crate::assets_management::SelectedNoteTheme2d>,
    theme: Res<crate::theme::LoadedTheme>,
    adaptive: Res<AdaptiveDifficulty>,
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
        Some(path) => path.clone(),
        None => format!("notes/2d/{}.png", note_theme.0).into(),
    };

    let tail_cfg = manifest.assets_2d_config.clone();

    clock.set_free(-COUNTDOWN);
    music_started.0 = false;
    valid_notes.0 = manifest.chart.harmonica.build_valid_notes();

    let chart = &manifest.chart;

    // Build every note's score state up front (cheap — plain data, no
    // entities/materials yet) plus the one piece of render data that isn't
    // already on `ScheduledNote` (the chord/split badge). Actual note
    // *visuals* are spawned later, lazily, by `spawn_visible_notes` as each
    // one enters the `LOOKAHEAD` window — a long/dense chart no longer pays
    // for every note's UI subtree (and comet-tail material) at song load.
    let items = track_items(&chart.track, &chart.timing);
    let flags = unlocked_flags(&items, &adaptive.sections, &adaptive.learned, adaptive.enabled);
    let mut flags = flags.into_iter();
    let mut combined: Vec<(ScheduledNote, Option<&'static str>)> = Vec::new();
    for item in &chart.track {
        let t = super::resolve_item_time(item, &chart.timing);
        let tag = play_mode_label(item.play_mode.as_ref());
        for event in &item.events {
            let (unlocked, section) = flags.next().unwrap_or((true, 0));
            if !unlocked {
                continue;
            }
            let is_blow = matches!(event.action, Action::Blow);
            let modifiers = event.modifiers.clone().unwrap_or_default();
            let natural_pitch = event.note.clone().unwrap_or_else(|| {
                chart
                    .harmonica
                    .wind_direction_label(event.hole, &event.action)
            });
            let expected_pitch = super::target_pitch(&natural_pitch, &modifiers);
            combined.push((
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
                    modifiers,
                    pitch_samples: Vec::new(),
                    amp_samples: Vec::new(),
                    phrase_section: section,
                },
                tag,
            ));
        }
    }
    // `score_notes`/`spawn_visible_notes` both rely on this being sorted —
    // charts are assumed authored in time order, but this makes that an
    // actual guarantee instead of an assumption.
    combined.sort_by(|a, b| {
        a.0.time
            .partial_cmp(&b.0.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (notes, play_mode_tags): (Vec<ScheduledNote>, Vec<Option<&'static str>>) =
        combined.into_iter().unzip();
    *song_notes = SongNotes { notes, cursor: 0 };
    *render_assets = NoteRenderAssets {
        head_image: Some(head_image.clone()),
        tail_cfg: Some(tail_cfg.clone()),
        play_mode_tags,
    };

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
            // Background painted first (this node itself), Main Layout second
            // — everything else here is a child, so it always paints above
            // the background. The song-progress bar (`BAR_Z_INDEX`) still
            // paints above this whole layout; panels below reserve
            // `BAR_HEIGHT` of top space so it doesn't cover their text.
            GlobalZIndex(1),
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
                padding: UiRect {
                    top: Val::Px(8.0 + BAR_HEIGHT),
                    ..UiRect::all(Val::Px(8.0))
                },
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
                    spawn_highway(hw, chart);
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
                    spawn_harmonica_strip(col, chart);
                });
            });

            // ── Right panel: info + 12-bar + metronome + score ───────────────
            root.spawn(Node {
                width: Val::Percent(40.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    top: Val::Px(12.0 + BAR_HEIGHT),
                    ..UiRect::all(Val::Px(12.0))
                },
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
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                        col.spawn((
                            Text::new(info),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.60, 0.65, 0.75)),
                        ));
                        col.spawn((
                            Text::new(harp_info),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.45, 0.72, 0.55)),
                        ));
                        if let Some(desc) = description {
                            col.spawn((
                                Text::new(desc.to_string()),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.50, 0.50, 0.55)),
                            ));
                        }
                        if let Some(author) = chart_author {
                            col.spawn((
                                Text::new(format!("Chart: {author}")),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.40, 0.40, 0.45)),
                            ));
                        }
                    });

                // Live phrase / groove banner (driven by phrase_overlay::update_phrase)
                spawn_phrase_banner(right);
                // Tab-notation ribbon for the current phrase (phrase_overlay::update_tab_ribbon)
                spawn_tab_ribbon(right);

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
                            &GridConfig::for_2d(),
                            theme.twelve_bar_colors(),
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
                        spawn_metronome(metro, beats_per_bar, bpm);
                    });

                // Technique colour legend
                spawn_modifier_legend(right, &legend_materials);

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
                                ..default()
                            },
                            TextColor(Color::WHITE),
                            ScoreText,
                        ));
                        p.spawn((
                            Text::new(""),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.90, 0.72, 0.20)),
                            ComboText,
                        ));
                        p.spawn((
                            Text::new(""),
                            TextFont {
                                font_size: FontSize::Px(20.0),
                                ..default()
                            },
                            TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                            FeedbackText,
                        ));
                    });
            });
        });
    let note_times: Vec<f64> = song_notes.notes.iter().map(|n| n.time).collect();
    spawn_song_progress(
        &mut commands,
        &manifest.waveform,
        manifest.music_duration_secs,
        &note_times,
        &adaptive.sections,
        &adaptive.learned,
    );
    super::wait_freeze_overlay::spawn_wait_freeze_prompt(&mut commands);
    let harp_hint = crate::song::harmonica::harp_banner(&chart.harmonica, key);
    spawn_countdown(&mut commands, Some(&harp_hint));
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
        // No dedicated shader animation yet — falls through to the shader's
        // default/last branch (currently "overdraw"'s), which is harmless.
        Some(Modifier::Slide) => 6.0,
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

/// Spawns the static highway furniture (lane stripes, dividers, hit zone) —
/// no notes. Notes are spawned later, lazily, by `spawn_visible_notes`.
fn spawn_highway(hw: &mut ChildSpawnerCommands, chart: &crate::song::chart::HarpChart) {
    // Lane count/width come from the loaded harmonica, not a fixed 10 —
    // a chromatic chart's 12+ holes need proportionally narrower lanes.
    let hole_count = chart.harmonica.hole_count() as usize;
    let lane_pct = 100.0 / hole_count as f32;

    for h in 0..hole_count {
        let left_pct = h as f32 * lane_pct;
        let alpha = if h % 2 == 0 { 0.04f32 } else { 0.0f32 };
        hw.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(left_pct),
                top: Val::Percent(0.0),
                width: Val::Percent(lane_pct),
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
}

/// Spawns note visuals for any note that has newly entered the `LOOKAHEAD`
/// window and doesn't have one yet. Runs every frame; cost is bounded by how
/// many notes are near the playhead, not the song length. Self-healing
/// across a loop wrap (no cursor to keep in sync): it just compares "notes
/// whose window could plausibly be open" against "notes that currently have
/// a visual", so notes reappear correctly once the (rewound) clock nears
/// them again.
pub fn spawn_visible_notes(
    mut commands: Commands,
    clock: Res<super::GameplayClock>,
    song_notes: Res<SongNotes>,
    render_assets: Res<NoteRenderAssets>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    highway: Query<Entity, With<NoteHighway>>,
    existing: Query<&NoteVisual>,
    mut shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
    show_numbers: Res<crate::assets_management::ShowNoteNumbers>,
) {
    let (Some(manifest), Ok(highway_entity), Some(head_image), Some(tail_cfg)) = (
        manifests.get(&selected.0),
        highway.single(),
        &render_assets.head_image,
        &render_assets.tail_cfg,
    ) else {
        return;
    };

    let hole_count = manifest.chart.harmonica.hole_count() as usize;
    let lane_pct = 100.0 / hole_count as f32;
    let elapsed = clock.get();

    let already_spawned: HashSet<usize> = existing.iter().map(|v| v.note_id).collect();
    let to_spawn = super::notes_needing_spawn(&song_notes.notes, &already_spawned, elapsed);
    if to_spawn.is_empty() {
        return;
    }

    commands.entity(highway_entity).with_children(|hw| {
        for i in to_spawn {
            spawn_note_visual(
                hw,
                i,
                &song_notes.notes[i],
                lane_pct,
                head_image,
                tail_cfg,
                render_assets.play_mode_tags.get(i).copied().flatten(),
                &mut shape_materials,
                show_numbers.0,
            );
        }
    });
}

/// Spawns one note's visual: the comet head (a lane-width square, kept round
/// by the disc image + aspect_ratio) plus its trailing tail. Positioned each
/// frame by `update_notes`; the material's shape/animation is driven by the
/// note's own technique modifiers, matching `size_note_tails`'s time-accurate
/// length.
fn spawn_note_visual(
    hw: &mut ChildSpawnerCommands,
    note_id: usize,
    note: &ScheduledNote,
    lane_pct: f32,
    head_image: &AssetPath<'static>,
    tail_cfg: &NoteThemeConfig,
    play_mode_tag: Option<&'static str>,
    shape_materials: &mut Assets<NoteTail2dMaterial>,
    show_numbers: bool,
) {
    let is_blow = note.is_blow;
    let hole = note.hole;
    let (r, g, b) = note_rgb(is_blow);
    let note_color = Color::srgba(r, g, b, 1.0);
    let left_pct = (note.hole as f32 - 1.0) * lane_pct;
    let duration_frac = (note.duration / LOOKAHEAD) as f32;

    let h_pct = note_height_pct(note.duration);
    let (vib, bend, wah) = note_techniques(Some(&note.modifiers));
    let mode = note_anim_mode(Some(&note.modifiers));
    let (mut params, mut wah_v) = tail_params(h_pct, vib, bend, wah);
    params.z = 0.0; // animation time, refreshed every frame
    wah_v.z = mode; // which technique animation to run
    wah_v.w = note_id as f32 * 0.7; // per-note phase offset
    let material = shape_materials.add(NoteTail2dMaterial {
        color: Color::srgba(r, g, b, 0.95).to_linear(),
        params,
        wah: wah_v,
    });

    hw.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(left_pct),
            bottom: Val::Percent(150.0), // placeholder; set in update_notes
            width: Val::Percent(lane_pct),
            aspect_ratio: Some(1.0),
            ..default()
        },
        NoteVisual { note_id },
    ))
    .with_children(|note_e| {
        // Tail + head layout shared with the note_editor binary via
        // note_visual_2d::spawn_note_children. Game-specific markers and
        // the direction arrow are added in the callbacks.
        spawn_note_children(
            note_e,
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
            |cmd| {
                cmd.insert(NoteTail { duration_frac });
            },
            |cmd| {
                cmd.insert(NoteHead).with_children(|head| {
                    let label = if show_numbers {
                        // `+`/`-` for blow/draw, no bend/overblow/slide
                        // suffix — that level of detail lives in the tab
                        // ribbon (`phrase_overlay`); the note-head label is
                        // just "which hole, which direction".
                        super::phrase_overlay::tab_label(hole, is_blow, &[])
                    } else if is_blow {
                        "\u{2191}".to_string()
                    } else {
                        "\u{2193}".to_string()
                    };
                    head.spawn((
                        Text::new(label),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
                    ));
                });
            },
        );

        // Chord / split play-mode badge, pinned to the bottom edge.
        if let Some(tag) = play_mode_tag {
            note_e.spawn((
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
            // Overblow/overdraw/slide all raise pitch ~1 semitone — represent
            // as an up-bend.
            Modifier::Overblow | Modifier::Overdraw | Modifier::Slide => shift = Some(1.0),
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

fn spawn_harmonica_strip(col: &mut ChildSpawnerCommands, chart: &crate::song::chart::HarpChart) {
    let hole_count = chart.harmonica.hole_count();
    let lane_pct = 100.0 / hole_count as f32;
    col.spawn(Node {
        flex_direction: FlexDirection::Row,
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|row| {
        for hole in 1u8..=hole_count {
            let b = chart.harmonica.wind_direction_label(hole, &Action::Blow);
            let d = chart.harmonica.wind_direction_label(hole, &Action::Draw);
            row.spawn((
                Node {
                    width: Val::Percent(lane_pct),
                    // Fixed px, not Vh — Vh resolves from the physical
                    // viewport and doesn't respond to `UiScale`, unlike this
                    // cell's own text, so the cell would stay a fixed size on
                    // screen while its labels scaled independently.
                    height: Val::Px(96.0),
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
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.50, 0.75, 1.00)),
                ));
                cell.spawn((
                    Text::new(format!("{hole}")),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                cell.spawn((
                    Text::new(d),
                    TextFont {
                        font_size: FontSize::Px(15.0),
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
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.50, 0.75, 1.00)),
        ));
        leg.spawn((
            Text::new("\u{25A0} DRAW"),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(1.00, 0.62, 0.35)),
        ));
    });
}

// ── Per-frame systems ─────────────────────────────────────────────────────────

pub fn update_notes(
    clock: Res<super::GameplayClock>,
    song_notes: Res<SongNotes>,
    mut commands: Commands,
    mut notes: Query<(Entity, &NoteVisual, &mut Node)>,
) {
    let elapsed = clock.get();
    for (entity, visual, mut node) in &mut notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let bottom = note_head_bottom_pct(note.time, elapsed, LOOKAHEAD);
        let duration_frac = (note.duration / LOOKAHEAD) as f32;

        // Recycle once the whole comet has fallen past the bottom. The tail
        // tip sits `SCROLL_SPAN * duration_frac` % above the head, so a long
        // note lingers exactly as long as its tail needs. Score state lives
        // independently in `SongNotes` now, so this despawns unconditionally
        // even while looping — `spawn_visible_notes` respawns it once the
        // (rewound) clock nears it again, with no state to lose.
        let tail_pct = SCROLL_SPAN * duration_frac;
        if bottom < -(tail_pct + 15.0) {
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

/// Head/tail tint for a note visual: gold while hit, dim red while missed,
/// otherwise its base blow/draw colour (head at full alpha, tail slightly
/// under, matching the alphas `spawn_note_visual` gives a freshly-spawned
/// note). Pulled out of `update_note_visuals` so the tint decision is
/// unit-testable without spinning up rendering.
fn note_tint(hit: bool, missed: bool, is_blow: bool) -> (Color, Color) {
    if hit {
        let tint = Color::srgba(1.0, 0.85, 0.25, 1.0);
        (tint, tint)
    } else if missed {
        let tint = Color::srgba(0.5, 0.13, 0.13, 1.0);
        (tint, tint)
    } else {
        let (r, g, b) = note_rgb(is_blow);
        (Color::srgba(r, g, b, 1.0), Color::srgba(r, g, b, 0.95))
    }
}

/// Tints a note's head image and tail material when it is hit or missed, and
/// restores its base blow/draw colour otherwise (see [`note_tint`]).
/// `ScheduledNote` isn't an ECS component (score state lives in
/// `SongNotes`), so this re-syncs every currently-spawned note's tint each
/// frame rather than reacting to `Changed<ScheduledNote>` — cheap since only
/// a `LOOKAHEAD` window's worth of notes are ever spawned.
pub fn update_note_visuals(
    song_notes: Res<SongNotes>,
    notes: Query<(&NoteVisual, &Children)>,
    mut heads: Query<&mut ImageNode, With<NoteHead>>,
    tails: Query<&MaterialNode<NoteTail2dMaterial>, With<NoteTail>>,
    mut shape_materials: ResMut<Assets<NoteTail2dMaterial>>,
) {
    for (visual, children) in &notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let (head_tint, tail_tint) = note_tint(note.hit, note.missed, note.is_blow);
        for child in children {
            if let Ok(mut head) = heads.get_mut(*child) {
                head.color = head_tint;
            }
            if let Ok(tail) = tails.get(*child)
                && let Some(mut material) = shape_materials.get_mut(&tail.0)
            {
                material.color = tail_tint.to_linear();
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

    let harp_pitches: HashSet<u8> = active
        .0
        .iter()
        .map(|p| p.midi)
        .filter(|m| valid_notes.0.contains(m))
        .collect();

    for (cell, mut bg, mut state) in &mut cells {
        let blow = chart.harmonica.wind_direction_midi(cell.0, &Action::Blow);
        let draw = chart.harmonica.wind_direction_midi(cell.0, &Action::Draw);

        let blow_hit = blow.is_some_and(|m| harp_pitches.contains(&m));
        let draw_hit = draw.is_some_and(|m| harp_pitches.contains(&m));

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

    // ── note_tint ──────────────────────────────────────────────────────────────

    #[test]
    fn note_tint_is_gold_when_hit() {
        let (head, tail) = note_tint(true, false, true);
        assert_eq!(head, Color::srgba(1.0, 0.85, 0.25, 1.0));
        assert_eq!(tail, head);
    }

    #[test]
    fn note_tint_is_dark_red_when_missed() {
        let (head, tail) = note_tint(false, true, true);
        assert_eq!(head, Color::srgba(0.5, 0.13, 0.13, 1.0));
        assert_eq!(tail, head);
    }

    #[test]
    fn note_tint_hit_wins_over_missed() {
        // Shouldn't happen in practice (score_notes never sets both), but
        // the tint decision itself should still be unambiguous.
        let (head, _) = note_tint(true, true, true);
        assert_eq!(head, Color::srgba(1.0, 0.85, 0.25, 1.0));
    }

    #[test]
    fn note_tint_restores_the_base_blow_draw_colour_once_neither() {
        let (blow_head, blow_tail) = note_tint(false, false, true);
        let (r, g, b) = note_rgb(true);
        assert_eq!(blow_head, Color::srgba(r, g, b, 1.0));
        assert_eq!(blow_tail, Color::srgba(r, g, b, 0.95));

        let (draw_head, draw_tail) = note_tint(false, false, false);
        let (r, g, b) = note_rgb(false);
        assert_eq!(draw_head, Color::srgba(r, g, b, 1.0));
        assert_eq!(draw_tail, Color::srgba(r, g, b, 0.95));
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
