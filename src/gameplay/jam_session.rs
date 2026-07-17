// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::{
    dialogs::button,
    localization::LocalizationExt,
    app::{JamProgression, SelectedSong},
    settings::AudioSettings,
    song::SongManifest,
    song::chart::Action,
    song::harmonica::{
        ChordQuality, Harmonica, Progression, blues_scale_classes, chord_intervals, harp_banner,
        progression_bars, semitone,
    },
    theme::LoadedTheme,
};

use crate::spectrogram::{OscMaterial, SpectrogramStyle, spawn_spectrogram};

use super::countdown_overlay::spawn_countdown;
use super::harmonica_overlay::spawn_harmonica_overlay;
use super::metronome_overlay::spawn_metronome;
use super::song_progress_overlay::{BAR_HEIGHT, spawn_song_progress};
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{
    AbsoluteBar, ActivePitches, COUNTDOWN, CurrentBar, GameplayClock, GameplayRoot, MusicPlayer,
    MusicStarted,
};

/// Free-play screen, two columns: left has everything but the harmonica
/// itself (title, loop toggle, 12-bar chart, metronome, spectrogram); right
/// is entirely the harmonica — the reference bend diagram and the
/// live-tinted hole map. The shared gameplay clock/music/pause systems run
/// for this mode too, so the chart tracks the song and the metronome clicks
/// — there are just no falling notes.
pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    spectrogram_style: Res<SpectrogramStyle>,
    osc_material: Res<OscMaterial>,
    theme: Res<LoadedTheme>,
    jam_progression: Res<JamProgression>,
    loc: Res<Localization>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Jam Session");
        return;
    };
    clock.set_free(-COUNTDOWN);
    music_started.0 = false;

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let bpm = chart.song.tempo_bpm;
    let progression = jam_progression.0;
    let chords: Vec<String> = progression_bars(key, progression)
        .into_iter()
        .map(|(root, _)| root)
        .collect();
    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    let beats_per_bar = {
        let ts = chart.song.time_signature.as_deref().unwrap_or("4/4");
        ts.split('/')
            .next()
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(4)
    };

    // Per-hole note labels + the lookup the live feedback system uses to light
    // the hole(s) the player is currently sounding, coloured by blues-scale fit
    // and — bar by bar — by whether the note is a tone of the chord currently
    // sounding (I, IV, or V), not just "somewhere in the blues scale".
    let (holes_info, guide) = build_hole_guide(&chart.harmonica, key, progression);

    // Which physical harp to grab: a Richter harp's key is its hole-1 blow note.
    let harp_hint = harp_banner(&chart.harmonica, key);

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
            // `BAR_HEIGHT` of top space so it doesn't cover their content.
            GlobalZIndex(1),
            GameplayRoot,
        ))
        .with_children(|root| {
            // Dark overlay for legibility.
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.70)),
            ));

            // ── Left half: 12-bar chart + metronome, vertical ────────────────
            root.spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(24.0),
                padding: UiRect {
                    top: Val::Px(16.0 + BAR_HEIGHT),
                    ..UiRect::all(Val::Px(16.0))
                },
                ..default()
            })
            .with_children(|left| {
                left.spawn((
                    Text::new(title),
                    TextFont {
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                left.spawn((
                    Text::new(harp_hint),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.80, 0.35)),
                ));
                left.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn_empty().apply_scene(button::small(
                        "\u{21BB} Loop",
                        |_: On<Pointer<Click>>, mut jam_loop: ResMut<JamLoop>| {
                            jam_loop.0 = !jam_loop.0;
                        },
                    ));
                    row.spawn((
                        Text::new(String::from(loc.msg("jam-loop-off"))),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.70, 0.70, 0.80)),
                        JamLoopLabel,
                    ));
                });
                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|grid| {
                    spawn_12_bar_grid(
                        grid,
                        &chords,
                        key,
                        progression,
                        &GridConfig::for_2d(),
                        theme.twelve_bar_colors(),
                    );
                });
                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|metro| {
                    spawn_metronome(metro, beats_per_bar, bpm);
                });
                left.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        ..default()
                    })
                    .with_children(|spec| {
                        spawn_spectrogram(spec, *spectrogram_style, &osc_material.0);
                    });
            });

            // ── Right half: everything harmonica — the bend diagram and the
            // live-tinted hole map both name/track holes on the same
            // instrument, so they share this column rather than splitting
            // across both halves.
            root.spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(Val::Px(BAR_HEIGHT)),
                ..default()
            })
            .with_children(|right| {
                spawn_harmonica_overlay(right, &chart.harmonica, &loc);
                spawn_hole_map(right, &holes_info, &loc);
            });
        });

    commands.insert_resource(guide);

    // Song-progress bar, pinned across the top like the scored modes — Jam
    // Session has no `SongNotes` (nothing is scored), so note markers are
    // taken directly from the chart's track items instead.
    let note_times: Vec<f64> = chart
        .track
        .iter()
        .map(|item| super::resolve_item_time(item, &chart.timing))
        .collect();
    // No phrase sections either — adaptive difficulty is a scored-mode
    // concept, so Jam Session's bar just shows no phrase strip rectangles.
    spawn_song_progress(
        &mut commands,
        &manifest.waveform,
        manifest.music_duration_secs,
        &note_times,
        &[],
        &[],
    );

    // Jam already shows the harp hint on the persistent left panel, so the
    // countdown doesn't repeat it.
    spawn_countdown(&mut commands, &loc, None);
}

// ── Music loop toggle ────────────────────────────────────────────────────────

/// Whether Jam Session should restart its background music from the top when
/// it reaches the end, instead of just letting it stop. Off by default; a
/// user preference that (intentionally) persists across songs within a jam.
#[derive(Resource, Default)]
pub struct JamLoop(pub bool);

/// The "Loop: on/off" readout, kept in step with [`JamLoop`].
#[derive(Component)]
pub struct JamLoopLabel;

/// Keeps the "Loop: ..." readout in step with the toggle.
pub fn update_jam_loop_label(
    jam_loop: Res<JamLoop>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<JamLoopLabel>>,
) {
    if !jam_loop.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(String::from(if jam_loop.0 {
            loc.msg("jam-loop-on")
        } else {
            loc.msg("jam-loop-off")
        }));
    }
}

/// Whether the jam's music should be (re)spawned right now: the jam has
/// started, Loop is on, and no `MusicPlayer` entity is currently alive (i.e.
/// the previous playthrough already finished and despawned itself — see
/// `restart_finished_jam_music`). Split out as a pure predicate so the
/// decision is unit-testable without spinning up an `App`.
fn should_restart_jam_music(loop_on: bool, music_started: bool, music_player_alive: bool) -> bool {
    music_started && loop_on && !music_player_alive
}

/// Restarts the jam's background music once the current playthrough has
/// *finished on its own* — the `MusicPlayer` entity despawns itself via
/// `PlaybackSettings::DESPAWN` (`countdown_overlay::update_countdown` spawns
/// it that way for Jam Session) — and Loop is on at that moment. Toggling
/// Loop itself does nothing here beyond flipping the resource
/// (`update_jam_loop_label` is the only other reader); this system never
/// touches a live sink, only ever spawning a *new* entity after the old one
/// is already gone — seeking or restarting a still-playing sink is
/// unreliable in `bevy_audio` (see `TODO.md`), so this sidesteps that
/// entirely rather than working around it.
///
/// Also resets `GameplayClock` back to 0 — Jam Session's clock free-runs on
/// frame deltas rather than anchoring to the sink (see `should_anchor_to_
/// sink`), so nothing else would ever bring it back down once it ran past
/// the song's length; left alone, the song-progress playhead would stay
/// pinned at the right edge forever even though the music genuinely
/// restarted.
pub fn restart_finished_jam_music(
    jam_loop: Res<JamLoop>,
    music_started: Res<MusicStarted>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    audio: Res<AudioSettings>,
    existing: Query<(), With<MusicPlayer>>,
    mut clock: ResMut<GameplayClock>,
    mut commands: Commands,
) {
    if !should_restart_jam_music(jam_loop.0, music_started.0, !existing.is_empty()) {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    // A song with no `song/*.ogg` never had a `MusicPlayer` to begin with
    // (see `countdown_overlay::update_countdown`) — nothing to loop.
    let Some(music) = manifest.music.clone() else {
        return;
    };
    clock.set_free(0.0);
    commands.spawn((
        AudioPlayer::<AudioSource>(music),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
        MusicPlayer,
        GameplayRoot,
    ));
}

// ── Live harmonica hole map ─────────────────────────────────────────────────────

/// Lookup driving the live hole feedback, rebuilt for each jam: which holes can
/// sound a given `note+octave`; which note classes are in the song's blues
/// scale generally; and, per bar of the 12-bar cycle, which note classes are
/// tones of *that bar's* chord (I, IV, or V) — chord-tone awareness is a
/// distinct, more advanced skill than just staying in the scale.
#[derive(Resource)]
pub struct JamHoleGuide {
    /// MIDI note number → the holes that can sound it (may be more than one
    /// — e.g. draw-2 and blow-3 are both G4 on a C harp).
    note_to_holes: HashMap<u8, Vec<u8>>,
    scale_classes: HashSet<String>,
    chord_tones_by_bar: [HashSet<String>; 12],
}

/// One hole cell in the map; its background is tinted each frame by play state.
#[derive(Component)]
pub struct JamHoleCell {
    hole: u8,
}

/// Static rendering data for one hole: its blow/draw notes and whether each sits
/// in the blues scale (for the green "safe note" hint).
struct HoleInfo {
    hole: u8,
    blow: String,
    draw: String,
    blow_in_scale: bool,
    draw_in_scale: bool,
}

const HOLE_DEFAULT: Color = Color::srgba(0.12, 0.12, 0.16, 0.9);
/// A chord tone of the bar currently sounding — the strongest, most targeted
/// choice right now (not just "in the scale somewhere").
const PLAY_CHORD_TONE: Color = Color::srgb(0.95, 0.85, 0.25);
const PLAY_IN_SCALE: Color = Color::srgb(0.20, 0.80, 0.35);
const PLAY_OUT_SCALE: Color = Color::srgb(0.90, 0.55, 0.15);
const LABEL_IN_SCALE: Color = Color::srgb(0.45, 0.85, 0.50);
const LABEL_OUT_SCALE: Color = Color::srgb(0.50, 0.50, 0.55);

/// The note class (drop the trailing octave digit) of e.g. `"D#5"` → `"D#"`.
fn note_class(note: &str) -> &str {
    note.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// The four note classes of `quality`'s chord rooted on `chord_root` (root,
/// 3rd, 5th, 7th — see `song::harmonica::chord_intervals`).
fn chord_tone_classes(chord_root: &str, quality: ChordQuality) -> HashSet<String> {
    chord_intervals(quality)
        .iter()
        .map(|&n| semitone(chord_root, n))
        .collect()
}

/// Build the per-hole render data and the live-feedback lookup from the harp
/// layout, the song key, its `progression` (see `song::harmonica::
/// Progression` — `Standard` for a real-song jam, player-selected for a
/// generated one), and its tempo (needed to track which bar — and thus
/// which chord — is currently sounding).
fn build_hole_guide(harp: &Harmonica, key: &str, progression: Progression) -> (Vec<HoleInfo>, JamHoleGuide) {
    let dash = "\u{2014}";
    let scale_classes = blues_scale_classes(key);
    let chord_tones_by_bar: [HashSet<String>; 12] = {
        let bars = progression_bars(key, progression);
        std::array::from_fn(|i| {
            let (root, quality) = &bars[i];
            chord_tone_classes(root, *quality)
        })
    };
    let mut note_to_holes: HashMap<u8, Vec<u8>> = HashMap::new();
    let mut holes = Vec::new();

    for hole in 1..=harp.hole_count() {
        let blow = harp.wind_direction_label(hole, &Action::Blow);
        let draw = harp.wind_direction_label(hole, &Action::Draw);
        if blow == dash && draw == dash {
            continue;
        }
        if let Some(m) = harp.wind_direction_midi(hole, &Action::Blow) {
            note_to_holes.entry(m).or_default().push(hole);
        }
        if let Some(m) = harp.wind_direction_midi(hole, &Action::Draw) {
            note_to_holes.entry(m).or_default().push(hole);
        }
        holes.push(HoleInfo {
            hole,
            blow_in_scale: scale_classes.contains(note_class(&blow)),
            draw_in_scale: scale_classes.contains(note_class(&draw)),
            blow,
            draw,
        });
    }

    (
        holes,
        JamHoleGuide {
            note_to_holes,
            scale_classes,
            chord_tones_by_bar,
        },
    )
}

/// Spawn the bottom-strip hole map: a row of cells (blow note, hole number, draw
/// note), with in-scale notes tinted green as a static guide.
fn spawn_hole_map(parent: &mut ChildSpawnerCommands, holes: &[HoleInfo], loc: &Localization) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(String::from(loc.msg("jam-hole-map-hint"))),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(Color::srgb(0.70, 0.70, 0.80)),
            ));
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for h in holes {
                    row.spawn((
                        Node {
                            width: Val::Px(50.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Px(2.0),
                            padding: UiRect::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(HOLE_DEFAULT),
                        JamHoleCell { hole: h.hole },
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            Text::new(note_class(&h.blow).to_string()),
                            TextFont { font_size: FontSize::Px(15.0), ..default() },
                            TextColor(if h.blow_in_scale { LABEL_IN_SCALE } else { LABEL_OUT_SCALE }),
                        ));
                        cell.spawn((
                            Text::new(h.hole.to_string()),
                            TextFont { font_size: FontSize::Px(16.0), ..default() },
                            TextColor(Color::WHITE),
                        ));
                        cell.spawn((
                            Text::new(note_class(&h.draw).to_string()),
                            TextFont { font_size: FontSize::Px(15.0), ..default() },
                            TextColor(if h.draw_in_scale { LABEL_IN_SCALE } else { LABEL_OUT_SCALE }),
                        ));
                    });
                }
            });
        });
}

/// How "targeted" a sounding note is, worst to best.
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub(super) enum NoteFit {
    OutOfScale,
    InScale,
    ChordTone,
}

/// Classifies one played note class (e.g. `"G"`, no octave — see
/// `PitchInfo::note`) by how well it fits the harmonic context right now:
/// a tone of the bar's current chord is the most targeted choice, elsewhere
/// in the blues scale is still "safe," anything else is out. Shared by the
/// live hole-map tint (`update_hole_map`) and the improv-lesson accumulator
/// (`accumulate_improv_stats`) so the two can never silently disagree.
pub(super) fn classify_note_fit(
    note: &str,
    chord_tones: &HashSet<String>,
    scale_classes: &HashSet<String>,
) -> NoteFit {
    if chord_tones.contains(note) {
        NoteFit::ChordTone
    } else if scale_classes.contains(note) {
        NoteFit::InScale
    } else {
        NoteFit::OutOfScale
    }
}

/// Tint each hole cell from the live mic pitches, three tiers: gold if the
/// sounding note is a tone of the chord currently sounding (the most targeted
/// choice — chord-tone awareness, not just scale membership), green if it's
/// elsewhere in the blues scale, amber if outside the scale, default when
/// silent. Reuses the same `ActivePitches` the scored modes detect.
pub fn update_hole_map(
    active: Res<ActivePitches>,
    guide: Option<Res<JamHoleGuide>>,
    current: Res<CurrentBar>,
    mut cells: Query<(&JamHoleCell, &mut BackgroundColor)>,
) {
    let Some(guide) = guide else {
        return;
    };
    let chord_tones = &guide.chord_tones_by_bar[current.0];

    // Map each currently-lit hole to the best fit among all notes sounding it.
    let mut lit: HashMap<u8, NoteFit> = HashMap::new();
    for p in &active.0 {
        if let Some(holes) = guide.note_to_holes.get(&p.midi) {
            let fit = classify_note_fit(&p.note, chord_tones, &guide.scale_classes);
            for &h in holes {
                lit.entry(h)
                    .and_modify(|v| {
                        if fit > *v {
                            *v = fit
                        }
                    })
                    .or_insert(fit);
            }
        }
    }

    for (cell, mut bg) in &mut cells {
        bg.0 = match lit.get(&cell.hole) {
            Some(NoteFit::ChordTone) => PLAY_CHORD_TONE,
            Some(NoteFit::InScale) => PLAY_IN_SCALE,
            Some(NoteFit::OutOfScale) => PLAY_OUT_SCALE,
            None => HOLE_DEFAULT,
        };
    }
}

// ── Improv-lesson scale-adherence accumulator ───────────────────────────────

/// Enforces a fresh attack per pitch for [`accumulate_improv_stats`] — the
/// same fresh-attack idea `gameplay::PitchGate` uses for scored modes
/// (`crate::scoring::AttackGate`), so holding one note doesn't tally it
/// again every frame it stays sounding.
#[derive(Resource, Default)]
pub struct ImprovGate(crate::scoring::AttackGate<u8>);

/// Running tally of every fresh note attack played during an open Jam
/// Session, classified by [`NoteFit`] against the bar it landed on. Reset
/// at the start of every `Playing` session (`gameplay::reset_score`) — not
/// jam-only, so it's always in a known state, but only [`accumulate_improv_
/// stats`] (jam-only) ever writes to it. The improv lesson's pass criterion
/// (`lessons::PassCriteria::ScaleAdherence`) reads [`adherence`](Self::
/// adherence) when the player ends the session.
#[derive(Resource, Default, Clone, Copy)]
pub struct ImprovStats {
    pub chord_tone: u32,
    pub in_scale: u32,
    pub out_of_scale: u32,
    /// Fresh attacks that landed inside a "rest" window of the phrase-
    /// discipline pattern (see [`in_rest_window`]) — tallied regardless of
    /// pitch/chord-tone classification, since phrase discipline judges
    /// *when* you played, not *what*.
    pub rest_violations: u32,
}

impl ImprovStats {
    pub fn total(&self) -> u32 {
        self.chord_tone + self.in_scale + self.out_of_scale
    }

    /// Fraction of attacks that were at least in-scale (a chord tone is the
    /// strictly better case within "in scale", so it counts too) — `None`
    /// with nothing played yet, same "nothing to report" convention as
    /// `gameplay::TechniqueStats::accuracy`.
    pub fn adherence(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some((self.chord_tone + self.in_scale) as f32 / total as f32)
        }
    }

    /// Fraction of attacks that were specifically chord tones — stricter
    /// than [`adherence`](Self::adherence), which also accepts merely-in-
    /// scale notes. The `chord-tone-improv` lesson's criterion.
    pub fn chord_tone_adherence(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some(self.chord_tone as f32 / total as f32)
        }
    }

    /// Fraction of attacks that landed *outside* a rest window — "did you
    /// leave space", not what was played. The `question-answer` lesson's
    /// criterion.
    pub fn phrase_discipline(&self) -> Option<f32> {
        let total = self.total();
        if total == 0 {
            None
        } else {
            Some(1.0 - (self.rest_violations as f32 / total as f32))
        }
    }
}

/// Whether `bar_index` (an absolute, non-wrapped bar count — see
/// `gameplay::AbsoluteBar`) falls inside a "rest" window of a repeating
/// play/rest pattern: `play_bars` bars of playing, then `rest_bars` bars of
/// rest, repeating. The phrase-discipline lesson's "leave space" primitive —
/// pure so it's directly unit-testable. A zero-length cycle (both zero)
/// never counts as rest, since there's no pattern to violate.
pub(super) fn in_rest_window(bar_index: usize, play_bars: usize, rest_bars: usize) -> bool {
    let cycle = play_bars + rest_bars;
    if cycle == 0 {
        return false;
    }
    bar_index % cycle >= play_bars
}

/// The phrase-discipline pattern every jam session measures against: 2 bars
/// of playing, then 2 bars of rest — the "question and answer" phrasing
/// discipline the lesson teaches (see `docs/lessons_plan.md`, engine item
/// 3). Always-on, like every other `ImprovStats` tally, not gated on a
/// lesson being in flight.
const PHRASE_PLAY_BARS: usize = 2;
const PHRASE_REST_BARS: usize = 2;

/// Tallies each fresh note attack into [`ImprovStats`], classified by
/// [`classify_note_fit`] against the bar it landed on — the live twin of
/// `update_hole_map`'s per-frame tint, but counting discrete attacks once
/// each instead of repainting every frame a pitch stays held.
pub fn accumulate_improv_stats(
    active: Res<ActivePitches>,
    guide: Option<Res<JamHoleGuide>>,
    current: Res<CurrentBar>,
    absolute: Res<AbsoluteBar>,
    mut gate: ResMut<ImprovGate>,
    mut stats: ResMut<ImprovStats>,
) {
    let Some(guide) = guide else {
        return;
    };
    let sounding: HashSet<u8> = active
        .0
        .iter()
        .filter(|p| guide.note_to_holes.contains_key(&p.midi))
        .map(|p| p.midi)
        .collect();
    gate.0.release_absent(|m| sounding.contains(&m));

    let chord_tones = &guide.chord_tones_by_bar[current.0];
    let resting = in_rest_window(absolute.0, PHRASE_PLAY_BARS, PHRASE_REST_BARS);
    for p in &active.0 {
        if !guide.note_to_holes.contains_key(&p.midi) || !gate.0.is_fresh(p.midi, true) {
            continue;
        }
        gate.0.consume(p.midi);
        match classify_note_fit(&p.note, chord_tones, &guide.scale_classes) {
            NoteFit::ChordTone => stats.chord_tone += 1,
            NoteFit::InScale => stats.in_scale += 1,
            NoteFit::OutOfScale => stats.out_of_scale += 1,
        }
        if resting {
            stats.rest_violations += 1;
        }
    }
}

#[cfg(test)]
mod tests;
