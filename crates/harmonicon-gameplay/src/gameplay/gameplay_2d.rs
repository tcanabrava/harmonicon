// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use harmonicon_app::app::{EffectiveHarmonica, SelectedSong};
use harmonicon_core::chart::{Action, Modifier};
use harmonicon_song::song::SongManifest;

use harmonicon_ui::music_score::{self, BravuraFont};

use super::adaptive_difficulty::AdaptiveDifficulty;
use super::beat_guides;
use super::countdown_overlay::spawn_countdown;
use super::highway_2d::{spawn_harmonica_strip, spawn_highway};
use super::hud_panel::{HudPanel, LaneSurface, contextual_panels, spawn_hud_panel, used_modifiers};
use super::judge::{judged_instant, live_technique_status};
use super::modifier_legend::build_legend_materials;
use super::note_feedback::{
    Judged, JudgedState, head_label_color, hold_uniform, judged_now, judged_scale, judged_stamp,
};
use super::note_ribbon::ribbon_technique;
use super::note_ribbon_2d::NoteRibbon2dMaterial;
use super::song_progress_overlay::{BAR_HEIGHT, NoteMarker, spawn_song_progress};
use super::technique_cue;
use super::{
    ActivePitches, ActiveTargets, COUNTDOWN, GameplayRoot, HoleCell, HoleState, LOOKAHEAD,
    MusicStarted, NoteVisual, PlayedHarp, ScheduledNote, ScoreReadoutAnchor, SongInfo, SongNotes,
    ValidHarpNotes, spawn_score_readout,
};
use harmonicon_platform::localization::Localization;
use harmonicon_platform::theme::{LoadedTheme, NoteColors, effective_note_colors};

/// Height of the hit line, as a percentage of the play area — the region a
/// note's leading edge must reach to be judged. Only ever used by the 2D
/// scroll layout (the 3D lane has no equivalent strip).
pub const HIT_H_PCT: f32 = 7.0;

/// Chart-level (not per-note) rendering config `spawn_visible_notes` needs
/// once a note's `LOOKAHEAD` window arrives — set once at song load
/// alongside `SongNotes`, since neither changes for the rest of the song.
#[derive(Resource, Default)]
pub(super) struct NoteRenderAssets {
    /// Chord/split play-mode badge text, parallel to `SongNotes::notes`
    /// (same index = same note) — the one piece of per-note render data that
    /// doesn't already live on `ScheduledNote` itself.
    pub(super) play_mode_tags: Vec<Option<&'static str>>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct LessonDisplayContext<'w> {
    compact: Res<'w, harmonicon_platform::responsive::CompactLayout>,
    bravura: Option<Res<'w, BravuraFont>>,
    lesson: Option<Res<'w, harmonicon_song::lessons::LessonContext>>,
}

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    effective: Res<EffectiveHarmonica>,
    mut valid_notes: ResMut<ValidHarpNotes>,
    mut played_harp: ResMut<PlayedHarp>,
    song_info: Res<SongInfo>,
    mut song_notes: ResMut<SongNotes>,
    mut render_assets: ResMut<NoteRenderAssets>,
    mut shape_materials: ResMut<Assets<NoteRibbon2dMaterial>>,
    adaptive: Res<AdaptiveDifficulty>,
    loc: Res<Localization>,
    display: LessonDisplayContext,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Playing state");
        return;
    };

    clock.set_free(-COUNTDOWN);
    music_started.0 = false;
    (*valid_notes, *played_harp) = ValidHarpNotes::for_played_harp(&effective, &manifest.chart);

    let chart = &manifest.chart;
    // Everything the player *sees* about the instrument — lane count, the
    // hole strip's notes, the "grab a C harp" hint — follows the harp they
    // are holding, exactly as the judge does.
    let played = effective.harp_for(chart);

    // Build every note's score state up front (cheap — plain data, no
    // entities/materials yet) plus the one piece of render data that isn't
    // already on `ScheduledNote` (the chord/split badge). Actual note
    // *visuals* are spawned later, lazily, by `spawn_visible_notes` as each
    // one enters the `LOOKAHEAD` window — a long/dense chart no longer pays
    // for every note's UI subtree (and comet-tail material) at song load.
    let (notes, play_mode_tags) = super::build_scheduled_notes(&effective, chart, &adaptive);
    *song_notes = SongNotes { notes, cursor: 0 };
    *render_assets = NoteRenderAssets { play_mode_tags };

    let compact = display.compact.0;
    let aural = display.lesson.is_some_and(|lesson| lesson.aural);
    let modifiers = used_modifiers(chart);
    let lyrics = harmonicon_core::lyrics::lyric_lines(chart);
    let panels = contextual_panels(
        LaneSurface::Highway2d,
        compact,
        aural,
        !modifiers.is_empty(),
        !lyrics.is_empty(),
    );

    // Animated tail previews for the techniques legend (built up front so the UI
    // closures only borrow a ready slice, not the material store).
    let legend_materials = if panels.technique_legend {
        build_legend_materials(&mut shape_materials, &modifiers)
    } else {
        Vec::new()
    };

    let bpm = chart.song.tempo_bpm;

    // The meter's own beat count, for the HUD's beat dots — from the one
    // reading of the chart's meter gameplay has (`bars::chart_meter`).
    let beats_per_bar = usize::from(super::bars::chart_meter(chart).numerator.max(1));
    // Filled in below; the shared score readout hangs off the highway so it
    // can sit a fixed distance above that node's own hit line.
    let mut highway = Entity::PLACEHOLDER;
    // Background painted first (this node itself), Main Layout second
    // — everything else here is a child, so it always paints above
    // the background. The song-progress bar (`BAR_Z_INDEX`) still
    // paints above this whole layout; panels below reserve
    // `BAR_HEIGHT` of top space so it doesn't cover their text.
    commands
        .spawn_empty()
        .apply_scene(bsn! {
            Node {
                width: {Val::Percent(100.0)},
                height: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Row},
            }
            GlobalZIndex(1)
        })
        .insert((ImageNode::new(manifest.background.clone()), GameplayRoot))
        .with_children(|root| {
            // Dark overlay
            root.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    width: {Val::Percent(100.0)},
                    height: {Val::Percent(100.0)},
                }
                BackgroundColor({Color::srgba(0.04, 0.04, 0.06, 0.70)})
            });

            // ── Left panel: note highway + harmonica ─────────────────────────
            let left_top_padding = if compact {
                8.0 + BAR_HEIGHT
            } else {
                8.0 + BAR_HEIGHT + music_score::PANEL_HEIGHT
            };
            root.spawn(Node {
                width: if compact {
                    Val::Percent(88.0)
                } else {
                    Val::Percent(74.0)
                },
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    top: Val::Px(left_top_padding),
                    ..UiRect::all(Val::Px(8.0))
                },
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|left| {
                // Note highway
                highway = left
                    .spawn((
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
                        spawn_highway(hw, played);
                    })
                    .id();

                // The technique coach sits at the end of the track, where
                // the note it coaches is landing.
                if !aural && modifiers.iter().any(technique_cue::is_coachable) {
                    super::technique_coach::spawn_technique_coach(left);
                }

                // Harmonica holes
                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|col| {
                    spawn_harmonica_strip(col, played, &loc);
                });
            });

            // ── Right panel: info + 12-bar + metronome + score ───────────────
            let right_top_padding = if compact {
                12.0 + BAR_HEIGHT
            } else {
                12.0 + BAR_HEIGHT + music_score::PANEL_HEIGHT
            };
            root.spawn(Node {
                // Narrower than the 40% it held when the song's description
                // lived here. What's left is live material only — phrase
                // banner, tab ribbon, metronome, technique legend — and the
                // width it gave up goes to the highway, which is the thing
                // the player is actually reading.
                width: if compact {
                    Val::Px(140.0)
                } else {
                    Val::Percent(26.0)
                },
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    top: Val::Px(right_top_padding),
                    ..UiRect::all(Val::Px(12.0))
                },
                row_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|right| {
                if panels.side_panel {
                    spawn_hud_panel(
                        right,
                        HudPanel {
                            song_info: &song_info,
                            loc: &loc,
                            beats_per_bar,
                            bpm,
                            legend_materials: &legend_materials,
                            // False here: 2D's key goes under the hole
                            // strip, beside the colours it explains.
                            blow_draw_legend: panels.blow_draw_in_panel,
                        },
                    );
                }
            });
        });

    // Beat/downbeat guides scroll inside the highway, behind the notes.
    beat_guides::spawn_beat_guides(&mut commands, highway);

    // The wait-for-note coaching card, just clear of the hit band so the
    // frozen note's head stays visible beneath it.
    super::wait_freeze_overlay::spawn_wait_freeze_prompt(
        &mut commands,
        highway,
        Val::Percent(HIT_H_PCT + 6.0),
    );

    // Score/combo/judgment, sitting just above the highway's own hit band
    // rather than in a corner of the screen — the judgment has to be
    // readable without looking away from the notes being judged.
    spawn_score_readout(
        &mut commands,
        highway,
        ScoreReadoutAnchor {
            left: Val::Percent(0.0),
            width: Val::Percent(100.0),
            bottom: Val::Percent(HIT_H_PCT),
        },
    );

    let note_markers: Vec<NoteMarker> = if !panels.progress_notes {
        Vec::new()
    } else {
        song_notes
            .notes
            .iter()
            .map(|n| NoteMarker {
                time: n.time,
                duration: n.duration,
                hole: n.hole,
                is_blow: n.is_blow,
            })
            .collect()
    };
    spawn_song_progress(
        &mut commands,
        &manifest.waveform,
        manifest.music_duration_secs,
        &note_markers,
        played.hole_count(),
        &adaptive.sections,
        &adaptive.learned,
    );
    let staff = panels.notation_staff && display.bravura.is_some();
    if staff && let Some(bravura) = &display.bravura {
        spawn_gameplay_music_score(&mut commands, bravura);
    }
    if panels.lyrics {
        super::karaoke::spawn_karaoke(&mut commands, lyrics, super::karaoke::strip_top(staff));
    }
    let harp_hint =
        super::song_info::harp_banner_text(played, &effective.song_key_for(chart), &loc);
    spawn_countdown(&mut commands, &loc, Some(&harp_hint), Some(&song_info));
}

/// Wraps `music_score::spawn_music_score` in its own absolutely-positioned,
/// full-width strip pinned directly below the song-progress bar (`BAR_HEIGHT`
/// down from the top) — shared by both `gameplay_2d::setup` and
/// `gameplay_3d::setup`, the same "helper lives in `gameplay_2d`, `gameplay_3d`
/// reuses it" precedent as `harp_pitches`/`head_label`.
///
/// A sibling top-level entity, not a child of the background-image root each
/// gameplay mode paints at `GlobalZIndex(1)` (see that root's own comment),
/// so it needs its own `GlobalZIndex` to render above it — same "strictly
/// between the background layer and the pause overlay's `GlobalZIndex(200)`"
/// reasoning `pause_menu`'s always-visible pause button already documents,
/// reusing the identical `GlobalZIndex(100)` so pausing still covers it too.
/// Without this it defaulted to `GlobalZIndex(0)`, below the background's
/// `1`, and the background image painted over the whole panel.
pub(super) fn spawn_gameplay_music_score(commands: &mut Commands, bravura: &BravuraFont) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(BAR_HEIGHT),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(music_score::PANEL_HEIGHT),
                ..default()
            },
            GlobalZIndex(100),
            GameplayRoot,
        ))
        .with_children(|panel| {
            music_score::spawn_music_score(panel, bravura);
        });
}

/// The note highway (the clipping container notes scroll inside). Marked so the
/// recycle logic can measure its height and convert head-heights to a fraction.
#[derive(Component)]
pub(super) struct NoteHighway;

/// The tab text in a note's cap (`+4`, `-3''`, `↓`), child of the note.
/// Replaced by a check or cross once the note is judged — a cue that doesn't
/// depend on telling gold from red.
#[derive(Component)]
pub(super) struct NoteCapLabel;

/// A note's ribbon: sized each frame to be time-accurate. Carries the
/// note's duration as a fraction of `LOOKAHEAD` so `size_note_ribbons` can
/// length it against the live highway height.
#[derive(Component)]
pub(super) struct NoteRibbon {
    duration_frac: f32,
}

/// Height of a ribbon's bright cap — the attack, holding the tab label — in
/// logical px, and so also the shortest a ribbon is drawn: a very short
/// note is always at least its cap.
const CAP_PX: f32 = 26.0;

/// A note's width as a fraction of its lane, leaving a gap so neighbouring
/// lanes stay distinct.
const NOTE_W: f32 = 0.85;

/// The highway distance (in %) a note scrolls from entering at the top to
/// its attack reaching the hit line — i.e. the span covered in `LOOKAHEAD`
/// seconds. A ribbon representing `duration` seconds is
/// `SCROLL_SPAN * duration / LOOKAHEAD` percent long, which is what makes it
/// time-accurate.
const SCROLL_SPAN: f32 = 100.0 - HIT_H_PCT;

/// Distance (in %) from the bottom of the highway to a note's attack — the
/// bottom edge of its ribbon. It reaches the drawn hit line (the top of the
/// hit band, `HIT_H_PCT` up) exactly at `note_time`, and goes below it as
/// the note carries on. Beat guides use the same mapping, so they cross the
/// line on the beat.
pub fn note_attack_pct(note_time: f64, elapsed: f64, lookahead: f64) -> f32 {
    let progress = 1.0 - (note_time - elapsed) / lookahead;
    (100.0 - f64::from(SCROLL_SPAN) * progress) as f32
}

/// Spawns note visuals for any note that has newly entered the `LOOKAHEAD`
/// window and doesn't have one yet. Runs every frame; cost is bounded by how
/// many notes are near the playhead, not the song length. Self-healing
/// across a loop wrap (no cursor to keep in sync): it just compares "notes
/// whose window could plausibly be open" against "notes that currently have
/// a visual", so notes reappear correctly once the (rewound) clock nears
/// them again.
pub fn spawn_visible_notes(
    mut already_spawned: Local<HashSet<usize>>,
    mut commands: Commands,
    clock: Res<super::GameplayClock>,
    song_notes: Res<SongNotes>,
    render_assets: Res<NoteRenderAssets>,
    played: Res<PlayedHarp>,
    highway: Query<Entity, With<NoteHighway>>,
    existing: Query<&NoteVisual>,
    mut shape_materials: ResMut<Assets<NoteRibbon2dMaterial>>,
    show_numbers: Res<harmonicon_platform::assets_management::ShowNoteNumbers>,
    theme: Res<LoadedTheme>,
    colorblind: Res<harmonicon_platform::settings::ColorblindPalette>,
    lesson: Option<Res<harmonicon_song::lessons::LessonContext>>,
    loc: Res<Localization>,
) {
    if lesson.is_some_and(|lesson| lesson.aural) {
        return;
    }
    let (Some(harp), Ok(highway_entity)) = (played.0.as_ref(), highway.single()) else {
        return;
    };
    let colors = effective_note_colors(theme.note_colors(), colorblind.0);

    // Lanes are the played harp's holes — the same count the strip and
    // highway were laid out with.
    let hole_count = harp.hole_count() as usize;
    let lane_pct = 100.0 / hole_count as f32;
    let elapsed = clock.get();

    already_spawned.clear();
    already_spawned.extend(existing.iter().map(|v| v.note_id));
    let mut to_spawn =
        super::notes_needing_spawn(&song_notes.notes, &already_spawned, elapsed).peekable();
    if to_spawn.peek().is_none() {
        return;
    }

    commands.entity(highway_entity).with_children(|hw| {
        for i in to_spawn {
            let note = &song_notes.notes[i];
            if note_has_left_view(note, elapsed) {
                continue;
            }
            let cue = technique_cue::note_cue(&loc, note).map(|text| NoteCue {
                text,
                on_right: technique_cue::beside_lane(note.hole, harp.hole_count()) > note.hole,
            });
            spawn_note_visual(
                hw,
                i,
                note,
                cue,
                lane_pct,
                render_assets.play_mode_tags.get(i).copied().flatten(),
                &mut shape_materials,
                show_numbers.0,
                colors,
            );
        }
    });
}

/// Spawns one note's visual: a ribbon down its lane, as long as the note
/// lasts, whose bright cap at the bottom is the attack and carries the tab
/// label. Positioned each frame by `update_notes` and lengthened by
/// `size_note_ribbons`; the ribbon's technique drawing comes from the
/// note's modifiers (`note_ribbon::ribbon_technique`).
fn spawn_note_visual(
    hw: &mut ChildSpawnerCommands,
    note_id: usize,
    note: &ScheduledNote,
    cue: Option<NoteCue>,
    lane_pct: f32,
    play_mode_tag: Option<&'static str>,
    shape_materials: &mut Assets<NoteRibbon2dMaterial>,
    show_numbers: bool,
    colors: NoteColors,
) {
    let label = head_label(note.hole, note.is_blow, &note.modifiers, show_numbers);
    let width_pct = lane_pct * NOTE_W;
    let left_pct = (note.hole as f32 - 1.0) * lane_pct + (lane_pct - width_pct) * 0.5;
    let material = shape_materials.add(NoteRibbon2dMaterial {
        color: ribbon_color(false, note.is_blow, colors),
        technique: ribbon_technique(&note.modifiers),
        shape: Vec4::new(note.duration as f32, CAP_PX, 0.0, 0.0),
        hold: Vec4::ZERO,
    });

    hw.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(left_pct),
            bottom: Val::Percent(150.0), // placeholder; set in update_notes
            width: Val::Percent(width_pct),
            height: Val::Px(CAP_PX), // placeholder; set in size_note_ribbons
            ..default()
        },
        MaterialNode(material),
        NoteVisual { note_id },
        NoteRibbon {
            duration_frac: (note.duration / LOOKAHEAD) as f32,
        },
        JudgedState::default(),
    ))
    .with_children(|note_e| {
        // The tab label, centred in the cap at the bottom.
        note_e.spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(CAP_PX),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            children![(
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(head_label_color(None)),
                NoteCapLabel,
            )],
        ));

        // What the technique asks for, in the lane beside the cap.
        if let Some(cue) = cue {
            note_e.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    bottom: {Val::Px(2.0)},
                    left: {if cue.on_right {
                        Val::Percent(100.0)
                    } else {
                        Val::Auto
                    }},
                    right: {if cue.on_right {
                        Val::Auto
                    } else {
                        Val::Percent(100.0)
                    }},
                    margin: {UiRect::horizontal(Val::Px(4.0))},
                    padding: {UiRect::axes(Val::Px(4.0), Val::Px(1.0))},
                    border_radius: {BorderRadius::all(Val::Px(4.0))},
                }
                BackgroundColor({Color::srgba(0.04, 0.05, 0.1, 0.72)})
                Children [
                    Text({cue.text})
                    TextFont { font_size: {FontSize::Px(18.0)} }
                    ~{TextLayout::no_wrap()}
                    TextColor({Color::srgba(0.95, 0.95, 1.0, 0.95)})
                ]
            });
        }

        // Chord / split play-mode badge, in the cap's corner.
        if let Some(tag) = play_mode_tag {
            note_e.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    bottom: {Val::Px(1.0)},
                    right: {Val::Px(3.0)},
                }
                Text({tag})
                TextFont { font_size: {FontSize::Px(9.0)} }
                TextColor({Color::srgba(0.05, 0.05, 0.08, 0.8)})
            });
        }
    });
}

/// A note ribbon's colour: its blow/draw colour from `colors` (the active
/// theme's note colors, or the fixed colorblind-safe pair — see
/// `theme::effective_note_colors`), or dim red once missed. A hit keeps the
/// base colour: the shader's hold state turns it gold while the pitch is
/// held and greys it when it drops, so the tint must not also claim it.
fn ribbon_color(missed: bool, is_blow: bool, colors: NoteColors) -> LinearRgba {
    if missed {
        return Color::srgba(0.5, 0.13, 0.13, 0.7).to_linear();
    }
    let c = if is_blow { colors.blow } else { colors.draw }.to_srgba();
    Color::srgba(c.red, c.green, c.blue, 0.95).to_linear()
}

// ── Per-frame systems ─────────────────────────────────────────────────────────

fn note_has_left_view(note: &ScheduledNote, elapsed: f64) -> bool {
    let ribbon_pct = SCROLL_SPAN * (note.duration / LOOKAHEAD) as f32;
    note_attack_pct(note.time, elapsed, LOOKAHEAD) < -(ribbon_pct + 15.0)
}

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
        // Recycle once the whole ribbon has fallen past the bottom. Score
        // state lives independently in `SongNotes`, so this despawns
        // unconditionally even while looping — `spawn_visible_notes`
        // respawns it once the (rewound) clock nears it again, with no
        // state to lose.
        if note_has_left_view(note, elapsed) {
            commands.entity(entity).despawn();
            continue;
        }
        node.bottom = Val::Percent(note_attack_pct(note.time, elapsed, LOOKAHEAD));
    }
}

/// A ribbon's on-screen height in logical px for a highway `highway_px`
/// tall: the distance scrolled during the note, so its top meets the hit
/// line as the note ends — but never shorter than its cap.
fn ribbon_height_px(duration_frac: f32, highway_px: f32) -> f32 {
    ((SCROLL_SPAN / 100.0) * duration_frac * highway_px).max(CAP_PX)
}

/// Lengths every ribbon to be time-accurate against the live highway height
/// (see [`ribbon_height_px`]).
pub fn size_note_ribbons(
    highway: Query<&ComputedNode, With<NoteHighway>>,
    mut ribbons: Query<(&NoteRibbon, &mut Node)>,
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
    for (ribbon, mut node) in &mut ribbons {
        let height = Val::Px(ribbon_height_px(ribbon.duration_frac, logical));
        if node.height != height {
            node.height = height;
        }
    }
}

/// Keeps each ribbon's colour ([`ribbon_color`]) and live hold state in step
/// with its note. `ScheduledNote` isn't an ECS component (score state lives
/// in `SongNotes`), so this re-syncs every currently-spawned note each frame
/// rather than reacting to a change — cheap, since only a `LOOKAHEAD`
/// window's worth of notes are ever spawned.
pub fn update_note_visuals(
    mut sounding: Local<HashSet<u8>>,
    song_notes: Res<SongNotes>,
    clock: Res<super::GameplayClock>,
    audio: Res<harmonicon_audio::AudioSettings>,
    pitch_filter: Res<super::HarmonicaPitchFilter>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    notes: Query<(&NoteVisual, &MaterialNode<NoteRibbon2dMaterial>)>,
    mut ribbons: ResMut<Assets<NoteRibbon2dMaterial>>,
    theme: Res<LoadedTheme>,
    colorblind: Res<harmonicon_platform::settings::ColorblindPalette>,
) {
    let colors = effective_note_colors(theme.note_colors(), colorblind.0);
    let judged = judged_instant(clock.get(), &audio, Some(&pitch_filter));
    harp_pitches(&active, &valid_notes, &mut sounding);
    for (visual, material) in &notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let color = ribbon_color(note.missed, note.is_blow, colors);
        // Live hold progress for the shader, from the same samples the judge
        // will verify the technique from when the hold ends.
        let hold = hold_uniform(
            note,
            judged,
            note.expected_pitch.is_some_and(|m| sounding.contains(&m)),
            live_technique_status(&note.modifiers, &note.pitch_samples, &note.amp_samples),
        );
        // Writing through `get_mut` queues `AssetEvent::Modified` and a GPU
        // re-upload even for an unchanged value, so compare first.
        if ribbons
            .get(&material.0)
            .is_some_and(|m| m.color != color || m.hold != hold)
            && let Some(mut m) = ribbons.get_mut(&material.0)
        {
            m.color = color;
            m.hold = hold;
        }
    }
}

/// A ribbon's `(left, width)` in percent of the highway when scaled by
/// `scale` about its own centre — the lane's `(left, width)` at scale 1.
fn scaled_span(lane_left: f32, width: f32, scale: f32) -> (f32, f32) {
    let scaled = width * scale;
    (lane_left + (width - scaled) * 0.5, scaled)
}

/// Widens a ribbon the instant its note is hit, narrows it on a miss, and
/// stamps its cap label with a check or cross. Width only, and through
/// layout rather than a transform: its length is the note's duration and
/// must stay true, and a transform would squash the label with it. The
/// transition is noticed through [`JudgedState`] rather than per-frame, so it
/// fires exactly once per judgment, and an A–B loop clearing the note's state
/// puts the ribbon back (full width, tab label) the same way.
pub fn animate_judged_notes(
    mut commands: Commands,
    song_notes: Res<SongNotes>,
    clock: Res<super::GameplayClock>,
    show_numbers: Res<harmonicon_platform::assets_management::ShowNoteNumbers>,
    reduced_motion: Res<harmonicon_platform::settings::ReducedMotion>,
    played: Res<PlayedHarp>,
    mut notes: Query<(
        Entity,
        &NoteVisual,
        &mut JudgedState,
        Option<&Judged>,
        &mut Node,
        &Children,
    )>,
    caps: Query<&Children, Without<NoteVisual>>,
    mut labels: Query<(&mut Text, &mut TextColor), With<NoteCapLabel>>,
) {
    let Some(harp) = played.0.as_ref() else {
        return;
    };
    let lane_pct = 100.0 / f32::from(harp.hole_count().max(1));
    let now = clock.get();
    for (entity, visual, mut state, judged, mut node, children) in &mut notes {
        let Some(note) = song_notes.notes.get(visual.note_id) else {
            continue;
        };
        let current = judged_now(note);
        let transitioned = current != state.0;
        let judged = if transitioned {
            state.0 = current;
            match current {
                Some(hit) => {
                    let j = Judged { hit, at: now };
                    commands.entity(entity).insert(j);
                    Some(j)
                }
                None => {
                    commands.entity(entity).remove::<Judged>();
                    None
                }
            }
        } else {
            judged.copied()
        };
        let scale = judged.map_or(1.0, |j| {
            judged_scale(j.hit, (now - j.at) as f32, reduced_motion.0)
        });
        let width = lane_pct * NOTE_W;
        let lane_left = (note.hole as f32 - 1.0) * lane_pct + (lane_pct - width) * 0.5;
        let (left, scaled) = scaled_span(lane_left, width, scale);
        if node.width != Val::Percent(scaled) {
            node.width = Val::Percent(scaled);
            node.left = Val::Percent(left);
        }
        if !transitioned {
            continue;
        }
        let wanted = match current {
            Some(hit) => judged_stamp(hit).to_string(),
            None => head_label(note.hole, note.is_blow, &note.modifiers, show_numbers.0),
        };
        for cap in children.iter().filter_map(|c| caps.get(c).ok()) {
            for label in cap {
                if let Ok((mut text, mut color)) = labels.get_mut(*label) {
                    text.0 = wanted.clone();
                    color.0 = head_label_color(current);
                }
            }
        }
    }
}

/// The tab shown in a note's cap: `+`/`-` and the hole with numbers on, a
/// direction arrow with them off — followed either way by the tab ribbon's
/// technique suffix (`'` per bent semitone, `o`, `*`), so `-3''` reads as a
/// whole-step bend right where the player is looking.
pub(super) fn head_label(
    hole: u8,
    is_blow: bool,
    modifiers: &[Modifier],
    show_numbers: bool,
) -> String {
    let tab = super::phrase_overlay::tab_label(hole, is_blow, modifiers);
    if show_numbers {
        return tab;
    }
    let plain = super::phrase_overlay::tab_label(hole, is_blow, &[]);
    let arrow = if is_blow { "\u{2191}" } else { "\u{2193}" };
    format!("{arrow}{}", &tab[plain.len()..])
}

/// The technique cue for one note head (see `technique_cue::note_cue`) and
/// which side of the head it sits on.
struct NoteCue {
    text: String,
    on_right: bool,
}

/// The set of currently-sounding MIDI pitches that are actually producible
/// on this harp — shared `harp_pitches` builder `update_holes`/
/// `update_holes_3d` each need before their per-cell glow loop. Refills
/// `out` so a caller's per-frame set keeps its storage.
pub(super) fn harp_pitches(
    active: &ActivePitches,
    valid_notes: &ValidHarpNotes,
    out: &mut HashSet<u8>,
) {
    out.clear();
    out.extend(
        active
            .0
            .iter()
            .map(|p| p.midi)
            .filter(|m| valid_notes.0.contains(m)),
    );
}

/// One hole cell's brightness/hint glow step for one frame — the shared
/// core of `update_holes`/`update_holes_3d`, which differ only in the final
/// paint (each caller applies the resulting `state.brightness`/
/// `state.is_blow` to its own `BackgroundColor`/`StandardMaterial`).
/// Matches `cell`'s blow/draw MIDI notes against `harp_pitches` (an actual
/// hit always wins), falls back to a dimmer "hint" floor from the scoring
/// overlay's `ActiveTargets` if neither reed sounds, and smooths
/// `state.brightness` toward that target — fast attack, slower decay, so a
/// hit flashes up instantly but fades out naturally.
pub(super) fn step_hole_glow(
    state: &mut HoleState,
    blow: Option<u8>,
    draw: Option<u8>,
    hint: Option<bool>,
    harp_pitches: &HashSet<u8>,
    attack: f32,
    decay: f32,
) {
    let blow_hit = blow.is_some_and(|m| harp_pitches.contains(&m));
    let draw_hit = draw.is_some_and(|m| harp_pitches.contains(&m));
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
    // Exponential easing never lands exactly; settle once the remaining gap
    // is far below one 8-bit colour step, so an idle cell stops repainting.
    if (target - state.brightness).abs() < GLOW_SETTLE {
        state.brightness = target;
    }
}

/// Brightness gap below which [`step_hole_glow`] snaps to its target.
const GLOW_SETTLE: f32 = 1e-3;

pub fn update_holes(
    mut sounding: Local<HashSet<u8>>,
    time: Res<Time>,
    active: Res<ActivePitches>,
    valid_notes: Res<ValidHarpNotes>,
    targets: Res<ActiveTargets>,
    played: Res<PlayedHarp>,
    lesson: Option<Res<harmonicon_song::lessons::LessonContext>>,
    mut cells: Query<(&HoleCell, &mut BackgroundColor, &mut HoleState)>,
) {
    // The harp the player is holding — the one the detected pitches belong
    // to — not the chart's, or a substituted harp's holes never light.
    let Some(harp) = played.0.as_ref() else {
        return;
    };
    let dt = time.delta_secs();

    let attack = 1.0 - (-dt * 25.0_f32).exp();
    let decay = 1.0 - (-dt * 4.0_f32).exp();
    harp_pitches(&active, &valid_notes, &mut sounding);

    for (cell, mut bg, mut state) in &mut cells {
        let blow = harp.wind_direction_midi(cell.0, &Action::Blow);
        let draw = harp.wind_direction_midi(cell.0, &Action::Draw);
        let hint = if lesson.as_ref().is_some_and(|lesson| lesson.aural) {
            None
        } else {
            targets
                .0
                .iter()
                .find(|(h, _)| *h == cell.0)
                .map(|(_, b)| *b)
        };

        step_hole_glow(&mut state, blow, draw, hint, &sounding, attack, decay);
        let b = state.brightness;

        let color = if state.is_blow {
            Color::srgb(0.10 + 0.18 * b, 0.12 + 0.33 * b, 0.16 + 0.72 * b)
        } else {
            Color::srgb(0.10 + 0.78 * b, 0.12 + 0.22 * b, (0.16 - 0.04 * b).max(0.0))
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ribbon geometry ───────────────────────────────────────────────────────

    #[test]
    fn a_ribbon_is_as_long_as_its_note_but_never_shorter_than_its_cap() {
        let highway_px = 900.0;
        let one_sec = ribbon_height_px(1.0 / LOOKAHEAD as f32, highway_px);
        assert!((one_sec - SCROLL_SPAN / 100.0 * highway_px / LOOKAHEAD as f32).abs() < 1e-3);
        assert_eq!(ribbon_height_px(0.0, highway_px), CAP_PX);
    }

    #[test]
    fn a_scaled_ribbon_stays_centred_in_its_lane() {
        let (left, width) = scaled_span(10.0, 8.0, 1.25);
        assert_eq!(width, 10.0);
        assert_eq!(left + width * 0.5, 10.0 + 8.0 * 0.5);
        assert_eq!(scaled_span(10.0, 8.0, 1.0), (10.0, 8.0));
    }

    #[test]
    fn attack_reaches_the_drawn_hit_line_on_time() {
        // The hit line is drawn at the top of the hit band, `HIT_H_PCT` up;
        // a ribbon's bottom edge is the attack and must meet it exactly.
        let got = note_attack_pct(1.0, 1.0, LOOKAHEAD);
        assert!((got - HIT_H_PCT).abs() < 0.01, "got {got}");
    }

    #[test]
    fn attack_enters_at_the_top_a_lookahead_early() {
        let got = note_attack_pct(LOOKAHEAD, 0.0, LOOKAHEAD);
        assert!((got - 100.0).abs() < 0.01, "got {got}");
    }

    #[test]
    fn attack_descends_over_time() {
        let b0 = note_attack_pct(2.0, 0.0, LOOKAHEAD);
        let b1 = note_attack_pct(2.0, 1.0, LOOKAHEAD);
        assert!(
            b1 < b0,
            "a note should fall (smaller bottom%) as time advances"
        );
    }

    #[test]
    fn attack_goes_negative_past_the_line() {
        let got = note_attack_pct(0.0, 3.0, LOOKAHEAD);
        assert!(got < 0.0, "got {got}");
    }

    // ── ribbon_color ──────────────────────────────────────────────────────────

    #[test]
    fn a_hit_ribbon_keeps_its_colour_for_the_hold_state() {
        // Gold comes from the shader's hold state, which also greys it when
        // the pitch drops — so a hit must not tint it gold itself.
        let colors = NoteColors::default();
        assert_eq!(ribbon_color(false, true, colors), {
            let c = colors.blow.to_srgba();
            Color::srgba(c.red, c.green, c.blue, 0.95).to_linear()
        });
    }

    #[test]
    fn a_missed_ribbon_dims_red() {
        let colors = NoteColors::default();
        let missed = ribbon_color(true, true, colors);
        assert_eq!(missed, Color::srgba(0.5, 0.13, 0.13, 0.7).to_linear());
        assert_eq!(missed, ribbon_color(true, false, colors));
    }

    #[test]
    fn blow_and_draw_ribbons_differ_and_follow_the_colorblind_palette() {
        let colors = NoteColors::default();
        assert_ne!(
            ribbon_color(false, true, colors),
            ribbon_color(false, false, colors)
        );
        let colorblind = harmonicon_platform::theme::COLORBLIND_NOTE_COLORS;
        assert_ne!(
            ribbon_color(false, true, colorblind),
            ribbon_color(false, true, colors)
        );
    }

    // ── harp_pitches / step_hole_glow ─────────────────────────────────────────

    fn pitch_info(midi: u8) -> harmonicon_audio::pitch_detect::PitchInfo {
        harmonicon_audio::pitch_detect::PitchInfo {
            midi,
            note: String::new(),
            octave: 0,
            frequency: 0.0,
        }
    }

    #[test]
    fn harp_pitches_keeps_only_sounding_pitches_the_harp_can_play() {
        let active = ActivePitches(vec![pitch_info(60), pitch_info(61)]);
        let valid = ValidHarpNotes(HashSet::from([60u8]));
        let mut sounding = HashSet::from([99u8]);
        harp_pitches(&active, &valid, &mut sounding);
        assert_eq!(sounding, HashSet::from([60u8]), "stale entries are cleared");
    }

    #[test]
    fn step_hole_glow_snaps_target_to_one_on_a_blow_hit() {
        let mut state = HoleState::default();
        let sounding = HashSet::from([60u8]);
        step_hole_glow(&mut state, Some(60), Some(64), None, &sounding, 1.0, 0.1);
        assert!(state.is_blow);
        assert!(
            (state.brightness - 1.0).abs() < 1e-6,
            "attack=1.0 should snap fully"
        );
    }

    #[test]
    fn step_hole_glow_prefers_an_actual_hit_over_a_hint() {
        let mut state = HoleState::default();
        let sounding = HashSet::from([64u8]);
        // Draw (64) is actually sounding; the hint says blow — the real hit wins.
        step_hole_glow(
            &mut state,
            Some(60),
            Some(64),
            Some(true),
            &sounding,
            1.0,
            0.1,
        );
        assert!(
            !state.is_blow,
            "the real draw hit should win over the blow hint"
        );
        assert!((state.brightness - 1.0).abs() < 1e-6);
    }

    #[test]
    fn step_hole_glow_uses_a_dim_floor_for_a_hint_with_nothing_sounding() {
        let mut state = HoleState::default();
        let sounding = HashSet::new();
        step_hole_glow(
            &mut state,
            Some(60),
            Some(64),
            Some(true),
            &sounding,
            1.0,
            0.1,
        );
        // A hint alone (no actual hit) only nudges brightness toward the dim
        // floor — `is_blow` is only ever written on a real hit, so it stays
        // at its prior value (the `Default` false) regardless of the hint.
        assert!(!state.is_blow);
        assert!((state.brightness - 0.18).abs() < 1e-6);
    }

    #[test]
    fn step_hole_glow_decays_toward_zero_with_nothing_sounding_or_hinted() {
        let mut state = HoleState {
            brightness: 1.0,
            is_blow: true,
        };
        let sounding = HashSet::new();
        step_hole_glow(&mut state, Some(60), Some(64), None, &sounding, 1.0, 0.5);
        // decay=0.5 halves the distance to the 0.0 target each step.
        assert!((state.brightness - 0.5).abs() < 1e-6);
        assert!(state.is_blow, "direction is only updated on an actual hit");
    }

    #[test]
    fn step_hole_glow_settles_exactly_on_its_target() {
        let mut state = HoleState {
            brightness: 1.0,
            is_blow: true,
        };
        let sounding = HashSet::new();
        for _ in 0..200 {
            step_hole_glow(&mut state, Some(60), Some(64), None, &sounding, 1.0, 0.1);
        }
        assert_eq!(state.brightness, 0.0, "an idle cell must stop changing");
    }
}

#[cfg(test)]
mod visibility_regression_tests {
    use super::*;

    #[test]
    fn expired_notes_stay_expired_until_the_clock_rewinds() {
        // The old spawn window remained open until t=3, even though the
        // despawner had already removed this short note by t=1.
        let mut note = super::super::tests::overlap_test_note(0.0);
        note.duration = 0.1;
        assert!(note_has_left_view(&note, 1.0));
        assert!(note_has_left_view(&note, 2.0));
        assert!(!note_has_left_view(&note, 0.0));
        note.duration = 4.0;
        assert!(!note_has_left_view(&note, 1.0));
    }
}
