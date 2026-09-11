// SPDX-License-Identifier: MIT

//! Jam Session: free-play screen (12-bar chart + metronome + spectrogram,
//! no falling notes) plus the live harmonica hole-map feedback and the
//! background-music loop toggle.

use std::collections::{HashMap, HashSet};

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_fluent::Localization;

use harmonicon_core::chart::{Action, Scale};
use harmonicon_core::harmonica::{
    ChordQuality, Harmonica, Position, Progression, chord_intervals, detected_harp_key,
    harp_banner, progression_bars, semitone,
};

use harmonicon_app::app::{JamProgression, JamScale, SelectedSong};
use harmonicon_audio::AudioSettings;
use harmonicon_gameplay::gameplay::{
    ActivePitches, COUNTDOWN, CurrentBar, GameplayClock, GameplayRoot, MidiTrackPlayer,
    MusicPlayer, MusicStarted, resolve_item_time,
};
use harmonicon_platform::localization::LocalizationExt;
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::button;

use harmonicon_gameplay::gameplay::countdown_overlay::spawn_countdown;
use harmonicon_gameplay::gameplay::harmonica_overlay::spawn_harmonica_overlay;
use harmonicon_gameplay::gameplay::metronome_overlay::spawn_metronome;
use harmonicon_gameplay::gameplay::song_progress_overlay::{
    BAR_HEIGHT, NoteMarker, spawn_song_progress,
};
use harmonicon_ui::dialogs::twelve_bar_grid::{GridConfig, spawn_12_bar_grid};
use harmonicon_ui::spectrogram::{OscMaterial, SpectrogramStyle, spawn_spectrogram};

use super::backing::JamGenre;
use super::improv::classify_note_fit;
use super::midi_tracks::{JamMidiMute, spawn_midi_track_row};
use super::position_guide::spawn_position_compass;
use super::rhythm_guide::spawn_rhythm_guide;
use harmonicon_app::app::GeneratedJamSession;

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
    mut clock: ResMut<GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut midi_mute: ResMut<JamMidiMute>,
    spectrogram_style: Res<SpectrogramStyle>,
    osc_material: Res<OscMaterial>,
    theme: Res<LoadedTheme>,
    jam_progression: Res<JamProgression>,
    jam_scale: Res<JamScale>,
    jam_genre: Res<JamGenre>,
    generated: Option<Res<GeneratedJamSession>>,
    loc: Res<Localization>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Jam Session");
        return;
    };
    clock.set_free(-COUNTDOWN);
    music_started.0 = false;
    // Fresh, all-unmuted for this jam — sized to the song's own track
    // count (empty for an ordinary, non-MIDI-backed song, so the mute row
    // below simply doesn't spawn and the apply/UI systems have nothing to
    // iterate).
    midi_mute.0 = vec![false; manifest.midi_tracks.as_ref().map_or(0, Vec::len)];

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
    // the hole(s) the player is currently sounding, coloured by scale fit and
    // — bar by bar — by whether the note is a tone of the chord currently
    // sounding (I, IV, or V), not just "somewhere in the scale". The chart's
    // own declared `Harmonica::scale()` wins when it sets one (a real song
    // authored for e.g. a major-pentatonic melody); otherwise `JamScale`
    // decides — `FirstPosition` (the blues hexatonic) unless "Generate Jam"
    // or a jam-based lesson picked something else.
    let scale = chart.harmonica.scale().unwrap_or(jam_scale.0);
    let (holes_info, guide) = build_hole_guide(&chart.harmonica, key, progression, scale);

    // Which physical harp to grab: a Richter harp's key is its hole-1 blow note.
    let harp_hint = harp_banner(&chart.harmonica, key);
    // Same detection, bare (no banner sentence), plus whichever position the
    // chart itself declares — for the live position compass below.
    let harp_key = detected_harp_key(&chart.harmonica);
    let position = chart.harmonica.position().and_then(Position::from_label);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                // Column, not Row: the 12-bar/harmonica columns below sit in
                // their own Row-direction wrapper (so they still sit side by
                // side), leaving room for the MIDI-track mute row — present
                // only for a MIDI-backed song — as a full-width sibling
                // underneath both of them.
                flex_direction: FlexDirection::Column,
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

            // The two side-by-side columns below, wrapped in their own
            // Row-direction, flex-growing container so the MIDI-track mute
            // row (added after this closes) can sit below both of them as a
            // full-width sibling instead of a third column.
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|columns| {
                // ── Left half: 12-bar chart + metronome, vertical ────────────────
                columns
                    .spawn(Node {
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
                                &loc.msg("jam-loop-button"),
                                |_: On<Activate>, mut jam_loop: ResMut<JamLoop>| {
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
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn_empty().apply_scene(
                                button::small(
                                    &loc.msg("jam-call-response-button"),
                                    |_: On<Activate>,
                                     mut enabled: ResMut<
                                        super::call_response::CallResponseEnabled,
                                    >| {
                                        enabled.0 = !enabled.0;
                                    },
                                ),
                            );
                            row.spawn((
                                Text::new(String::from(loc.msg("jam-call-response-off"))),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.70, 0.70, 0.80)),
                                super::call_response::CallResponseLabel,
                            ));
                        });
                        left.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|grid| {
                            let _ = spawn_12_bar_grid(
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
                            spawn_metronome(metro, &loc, beats_per_bar, bpm);
                        });
                        // Only for a generated jam — a real song has no
                        // `Genre` concept attached to it (see
                        // `jam::rhythm_guide`'s own doc comment).
                        if generated.is_some() {
                            spawn_rhythm_guide(left, &loc, jam_genre.0);
                        }
                        harmonicon_ui::spectrogram::spawn_style_toggle(
                            left,
                            *spectrogram_style,
                            &loc,
                        );
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
                columns
                    .spawn(Node {
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
                        spawn_position_compass(
                            right,
                            &loc,
                            harp_key.as_deref(),
                            position,
                            theme.circle_of_fifths_colors(),
                        );
                    });
            });

            // MIDI-backed song only — an ordinary music/silent song has
            // nothing to mute, so nothing spawns here for it.
            if let Some(tracks) = &manifest.midi_tracks {
                spawn_midi_track_row(root, tracks, &loc);
            }
        });

    commands.insert_resource(guide);

    // Song-progress bar, pinned across the top like the scored modes — Jam
    // Session has no `SongNotes` (nothing is scored), so note markers are
    // built directly from the chart's own track events instead — one
    // marker per event (not per item), matching the scored modes' own
    // per-event `ScheduledNote` granularity, so a chord/split item's
    // notes each get their own correctly-tinted marker.
    let note_markers: Vec<NoteMarker> = chart
        .track
        .iter()
        .flat_map(|item| {
            let time = resolve_item_time(item, &chart.timing);
            item.events.iter().map(move |ev| NoteMarker {
                time,
                duration: item.duration,
                hole: ev.hole,
                is_blow: matches!(ev.action, Action::Blow),
            })
        })
        .collect();
    // No phrase sections either — adaptive difficulty is a scored-mode
    // concept, so Jam Session's bar just shows no phrase strip rectangles.
    spawn_song_progress(
        &mut commands,
        &manifest.waveform,
        manifest.music_duration_secs,
        &note_markers,
        chart.harmonica.hole_count(),
        &[],
        &[],
    );

    // Jam already shows the harp hint on the persistent left panel, so the
    // countdown doesn't repeat it.
    spawn_countdown(&mut commands, &loc, None);

    super::call_response::spawn_call_response_banner(&mut commands);
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
/// `PlaybackSettings::DESPAWN` — and Loop is on at that moment. This system
/// never touches a live sink, only ever spawning a *new* entity after the
/// old one is gone — seeking or restarting a still-playing sink is
/// unreliable in `bevy_audio` (see `TODO.md`).
///
/// Also resets `GameplayClock` back to 0 — Jam Session's clock free-runs on
/// frame deltas rather than anchoring to the sink (see `should_anchor_to_
/// sink`), so nothing else would bring it back down once it ran past the
/// song's length; otherwise the song-progress playhead would stay pinned
/// at the right edge even though the music genuinely restarted.
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
    // A song with neither `song/*.ogg`/`*.wav` nor `midi_tracks` never had
    // a `MusicPlayer` to begin with (see `countdown_overlay::
    // update_countdown`) — nothing to loop.
    if manifest.music.is_none() && manifest.midi_tracks.is_none() {
        return;
    }
    clock.set_free(0.0);
    if let Some(music) = manifest.music.clone() {
        commands.spawn((
            AudioPlayer::<AudioSource>(music),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
            MusicPlayer,
            GameplayRoot,
        ));
    } else if let Some(tracks) = &manifest.midi_tracks {
        // Mute state (`JamMidiMute`) isn't touched here — it's a resource
        // independent of any particular sink, so a track muted before the
        // loop stays muted after it without needing to be re-applied.
        for (index, track) in tracks.iter().enumerate() {
            commands.spawn((
                AudioPlayer::<AudioSource>(track.source.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
                MusicPlayer,
                MidiTrackPlayer(index),
                GameplayRoot,
            ));
        }
    }
}

// ── Live harmonica hole map ─────────────────────────────────────────────────────

/// Lookup driving the live hole feedback, rebuilt for each jam: which holes
/// can sound a given `note+octave`; which note classes are in the jam's
/// active [`Scale`] generally (blues by default, but configurable — see
/// `JamScale`); and, per bar of the 12-bar cycle, which note classes are
/// tones of *that bar's* chord (I, IV, or V) — chord-tone awareness is a
/// distinct, more advanced skill than just staying in scale.
///
/// Fields are `pub(crate)`: `jam::improv::accumulate_improv_stats` reads
/// them directly, the same lookup `update_hole_map`'s tint uses, so the
/// two can't disagree.
#[derive(Resource)]
pub struct JamHoleGuide {
    /// MIDI note number → the holes that can sound it (may be more than one
    /// — e.g. draw-2 and blow-3 are both G4 on a C harp).
    pub(crate) note_to_holes: HashMap<u8, Vec<u8>>,
    pub(crate) scale_classes: HashSet<String>,
    pub(crate) chord_tones_by_bar: [HashSet<String>; 12],
}

/// One hole cell in the map; its background is tinted each frame by play state.
#[derive(Component)]
pub struct JamHoleCell {
    hole: u8,
}

/// Static rendering data for one hole: its blow/draw notes and whether each sits
/// in the blues scale (for the green "safe note" hint).
pub(crate) struct HoleInfo {
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
/// A hole used by the current call-and-response lick, shown while nothing's
/// actually sounding it right now — a visual memory aid for the echo, not a
/// graded outcome (see `call_response`'s module doc comment).
const PLAY_GHOST_LICK: Color = Color::srgba(0.45, 0.40, 0.85, 0.85);

/// The note class (drop the trailing octave digit) of e.g. `"D#5"` → `"D#"`.
/// `pub(super)` so `jam::call_response`'s lick generator can classify a
/// MIDI pitch's note class the same way the hole map's own tint does.
pub(super) fn note_class(note: &str) -> &str {
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
/// generated one), its `scale` (see `song::chart::Scale` — the caller
/// resolves the chart-vs-`JamScale` precedence; this function just applies
/// whichever one it's given), and its tempo (needed to track which bar —
/// and thus which chord — is currently sounding).
pub(crate) fn build_hole_guide(
    harp: &Harmonica,
    key: &str,
    progression: Progression,
    scale: Scale,
) -> (Vec<HoleInfo>, JamHoleGuide) {
    let dash = "\u{2014}";
    let scale_classes = scale.classes(key);
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
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
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
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(if h.blow_in_scale {
                                LABEL_IN_SCALE
                            } else {
                                LABEL_OUT_SCALE
                            }),
                        ));
                        cell.spawn((
                            Text::new(h.hole.to_string()),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                        cell.spawn((
                            Text::new(note_class(&h.draw).to_string()),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(if h.draw_in_scale {
                                LABEL_IN_SCALE
                            } else {
                                LABEL_OUT_SCALE
                            }),
                        ));
                    });
                }
            });
        });
}

/// Tint each hole cell from the live mic pitches, three tiers: gold if the
/// sounding note is a tone of the chord currently sounding (the most targeted
/// choice — chord-tone awareness, not just scale membership), green if it's
/// elsewhere in the blues scale, amber if outside the scale; a hole that's
/// part of the current call-and-response lick (`call_response::
/// CallResponseState`) but not currently sounding gets the ghost tint
/// instead of the plain default, so the player has a visual reference for
/// what to echo. Reuses the same `ActivePitches` the scored modes detect.
pub fn update_hole_map(
    active: Res<ActivePitches>,
    guide: Option<Res<JamHoleGuide>>,
    current: Res<CurrentBar>,
    call_response: Option<Res<super::call_response::CallResponseState>>,
    mut cells: Query<(&JamHoleCell, &mut BackgroundColor)>,
) {
    let Some(guide) = guide else {
        return;
    };
    let chord_tones = &guide.chord_tones_by_bar[current.0];

    // Map each currently-lit hole to the best fit among all notes sounding it.
    let mut lit: HashMap<u8, super::improv::NoteFit> = HashMap::new();
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

    let ghost_holes: &[u8] = call_response
        .as_deref()
        .map(|s| s.lick_holes.as_slice())
        .unwrap_or(&[]);

    for (cell, mut bg) in &mut cells {
        bg.0 = match lit.get(&cell.hole) {
            Some(super::improv::NoteFit::ChordTone) => PLAY_CHORD_TONE,
            Some(super::improv::NoteFit::InScale) => PLAY_IN_SCALE,
            Some(super::improv::NoteFit::OutOfScale) => PLAY_OUT_SCALE,
            None if ghost_holes.contains(&cell.hole) => PLAY_GHOST_LICK,
            None => HOLE_DEFAULT,
        };
    }
}

#[cfg(test)]
mod tests;
