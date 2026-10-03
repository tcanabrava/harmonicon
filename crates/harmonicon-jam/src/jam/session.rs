// SPDX-License-Identifier: MIT

//! Jam Session: the free-play screen (12-bar chart + metronome +
//! spectrogram, no falling notes), its transport (the finite-song loop
//! toggle, a generated jam's continuity and scheduled ending) and the
//! per-jam reset. The live hole map it spawns is `hole_map`'s.

use bevy::audio::{AudioPlayer, AudioSink, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_core::chart::Action;
use harmonicon_core::harmonica::{Position, detected_harp_key, progression_bars};

use harmonicon_app::app::{EffectiveHarmonica, JamProgression, JamScale, SelectedSong};
use harmonicon_audio::AudioSettings;
use harmonicon_gameplay::gameplay::{
    BackingStemPlayer, COUNTDOWN, GameplayClock, GameplayRoot, MusicPlayer, MusicStarted,
    resolve_item_time,
};
use harmonicon_platform::localization::{Localization, LocalizationExt};
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

use super::backing::{JamGenre, generate_ending_stems};
use super::hole_map::{build_hole_guide, spawn_detected_note, spawn_hole_map};
use super::midi_tracks::{JamStemMute, spawn_backing_stem_row};
use super::position_guide::spawn_position_compass;
use super::rhythm_guide::spawn_rhythm_guide;
use super::session_ui::*;
use harmonicon_app::app::GeneratedJamSession;

/// Free-play screen, two columns: left has everything but the harmonica
/// itself (title, loop toggle, 12-bar chart, metronome, spectrogram); right
/// is entirely the harmonica — the reference bend diagram and the
/// live-tinted hole map. The shared gameplay clock/music/pause systems run
/// for this mode too, so the chart tracks the song and the metronome clicks
/// — there are just no falling notes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct JamResetState<'w> {
    ending: ResMut<'w, JamEnding>,
    call_response: ResMut<'w, super::call_response::CallResponseState>,
    guides_visible: Res<'w, JamGuidesVisible>,
    call_density: Res<'w, super::call_response::JamCallDensity>,
    adaptive_band: Res<'w, super::band::AdaptiveBand>,
    band_listener: ResMut<'w, super::band::BandListener>,
    band_tracker: ResMut<'w, super::band::BandTracker>,
}

pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    mut stem_mute: ResMut<JamStemMute>,
    spectrogram_style: Res<SpectrogramStyle>,
    osc_material: Res<OscMaterial>,
    theme: Res<LoadedTheme>,
    jam_progression: Res<JamProgression>,
    jam_scale: Res<JamScale>,
    jam_genre: Res<JamGenre>,
    generated: Option<Res<GeneratedJamSession>>,
    effective: Res<EffectiveHarmonica>,
    loc: Res<Localization>,
    mut reset: JamResetState,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Jam Session");
        return;
    };
    clock.set_free(-COUNTDOWN);
    music_started.0 = false;
    *reset.ending = JamEnding::default();
    *reset.call_response = super::call_response::CallResponseState::fresh();
    *reset.band_listener = super::band::BandListener::default();
    *reset.band_tracker = super::band::BandTracker::fresh();
    let guides_visible = reset.guides_visible.0;
    let call_density = &reset.call_density;
    let adaptive_band = reset.adaptive_band.0;
    // Fresh, all-unmuted for this jam — sized to the song's own track
    // count (empty for an ordinary, non-MIDI-backed song, so the mute row
    // below simply doesn't spawn and the apply/UI systems have nothing to
    // iterate).
    stem_mute.0.resize(manifest.backing_stems.as_ref().map_or(0, Vec::len), false);
    stem_mute.0.fill(false);

    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let bpm = chart.song.tempo_bpm;
    let progression = jam_progression.0;
    let chords: Vec<String> =
        progression_bars(key, progression).into_iter().map(|(root, _)| root).collect();
    let title = format!("{} \u{2014} {}", chart.song.artist, chart.song.title);
    // From gameplay's one reading of a chart's meter (`bars::chart_meter`),
    // so the HUD's beat dots can't disagree with the click they animate.
    let beats_per_bar =
        usize::from(harmonicon_gameplay::gameplay::chart_meter(chart).numerator.max(1));

    // Per-hole note labels + the lookup the live feedback system uses to light
    // the hole(s) the player is currently sounding, coloured by scale fit and
    // — bar by bar — by whether the note is a tone of the chord currently
    // sounding (I, IV, or V), not just "somewhere in the scale". The chart's
    // own declared `Harmonica::scale()` wins when it sets one (a real song
    // authored for e.g. a major-pentatonic melody); otherwise `JamScale`
    // decides — `FirstPosition` (the blues hexatonic) unless "Generate Jam"
    // or a jam-based lesson picked something else.
    let scale = chart.harmonica.scale().unwrap_or(jam_scale.0);
    // The hole map, the overlay diagram and the "grab a C harp" hint all
    // describe the harp the player is *holding* — the harp-check page runs
    // before a jam too, and a substituted harp has other notes in the same
    // holes. The backing stays in the chart's key regardless: a jam has no
    // tab to transpose, only holes to name.
    let harp = effective.harp_for(chart);
    let (holes_info, guide) = build_hole_guide(harp, key, progression, scale);

    // Which physical harp to grab: a Richter harp's key is its hole-1 blow note.
    let harp_hint =
        harmonicon_gameplay::gameplay::harp_banner_text(harp, &effective.song_key_for(chart), &loc);
    // Same detection, bare (no banner sentence), plus whichever position the
    // chart itself declares — for the live position compass below.
    let harp_key = detected_harp_key(harp);
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
            root.spawn_empty().apply_scene(bsn! {
                Node {
                    position_type: {PositionType::Absolute},
                    width: {Val::Percent(100.0)},
                    height: {Val::Percent(100.0)},
                }
                BackgroundColor({Color::srgba(0.04, 0.04, 0.06, 0.70)})
            });

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
                    .spawn((
                        Node {
                            width: Val::Percent(if guides_visible { 50.0 } else { 100.0 }),
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
                        },
                        JamPrimaryPanel,
                    ))
                    .with_children(|left| {
                        left.spawn_empty().apply_scene(bsn! {
                            Text({title})
                            TextFont { font_size: {FontSize::Px(20.0)} }
                            TextColor({Color::WHITE})
                        });
                        left.spawn_empty().apply_scene(bsn! {
                            Text({harp_hint})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.95, 0.80, 0.35)})
                        });
                        if generated.is_some() {
                            left.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(8.0),
                                ..default()
                            })
                            .with_children(|row| {
                                row.spawn_empty().apply_scene(button::small(
                                    &loc.msg("jam-end-after-chorus-button"),
                                    |_: On<Activate>,
                                     absolute: Res<
                                        harmonicon_gameplay::gameplay::AbsoluteBar,
                                    >,
                                     mut ending: ResMut<JamEnding>| {
                                        if ending.stop_at_bar.is_none()
                                            && ending.ended_at_secs.is_none()
                                        {
                                            ending.stop_at_bar =
                                                Some(next_chorus_boundary(absolute.0));
                                        }
                                    },
                                ));
                                row.spawn_empty().apply_scene(bsn! {
                                    Text({String::from(loc.msg("jam-keep-playing"))})
                                    TextFont { font_size: {FontSize::Px(15.0)} }
                                    TextColor({Color::srgb(0.70, 0.70, 0.80)})
                                    JamEndingLabel
                                });
                            });
                        } else {
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
                                row.spawn_empty().apply_scene(bsn! {
                                    Text({String::from(loc.msg("jam-loop-off"))})
                                    TextFont { font_size: {FontSize::Px(15.0)} }
                                    TextColor({Color::srgb(0.70, 0.70, 0.80)})
                                    JamLoopLabel
                                });
                            });
                        }
                        left.spawn_empty().apply_scene(bsn! {
                            Text({String::from(loc.msg_args(
                                "jam-form-position",
                                &[("chorus", "1".into()), ("bar", "1".into())],
                            ))})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.95, 0.80, 0.35)})
                            JamFormPosition
                        });
                        left.spawn_empty().apply_scene(bsn! {
                            Text({format!("{}  →  {}", chords[0], chords[1])})
                            TextFont { font_size: {FontSize::Px(32.0)} }
                            TextColor({Color::WHITE})
                            JamChordPosition
                        });
                        spawn_form_strip(left, key, progression, theme.twelve_bar_colors());
                        super::call_response::spawn_call_response_banner(left);
                        spawn_detected_note(left, &loc);
                        left.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn_empty().apply_scene(button::small(
                                &loc.msg("jam-guides-button"),
                                |_: On<Activate>, mut guides: ResMut<JamGuidesVisible>| {
                                    guides.0 = !guides.0;
                                },
                            ));
                            row.spawn_empty().apply_scene(bsn! {
                                Text({String::from(if guides_visible {
                                    loc.msg("jam-guides-on")
                                } else {
                                    loc.msg("jam-guides-off")
                                })})
                                TextFont { font_size: {FontSize::Px(15.0)} }
                                TextColor({Color::srgb(0.70, 0.70, 0.80)})
                                JamGuidesLabel
                            });
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
                            row.spawn_empty().apply_scene(bsn! {
                                Text({String::from(loc.msg("jam-call-response-off"))})
                                TextFont { font_size: {FontSize::Px(15.0)} }
                                TextColor({Color::srgb(0.70, 0.70, 0.80)})
                                ~{super::call_response::CallResponseLabel}
                            });
                        });
                        // The one musical control over the calls: how busy
                        // they are. A cycle button, not a level picker.
                        left.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn_empty().apply_scene(button::small(
                                &loc.msg("jam-call-density-button"),
                                |_: On<Activate>,
                                 mut density: ResMut<super::call_response::JamCallDensity>| {
                                    density.0 = density.0.next();
                                },
                            ));
                            row.spawn_empty().apply_scene(bsn! {
                                Text({String::from(
                                    loc.msg(super::call_response::density_key(call_density.0)),
                                )})
                                TextFont { font_size: {FontSize::Px(15.0)} }
                                TextColor({Color::srgb(0.70, 0.70, 0.80)})
                                ~{super::call_response::CallDensityLabel}
                            });
                        });
                        // Only a generated jam has a band that can listen.
                        if generated.is_some() {
                            left.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(8.0),
                                ..default()
                            })
                            .with_children(|row| {
                                row.spawn_empty().apply_scene(button::small(
                                    &loc.msg("jam-adaptive-band-button"),
                                    |_: On<Activate>,
                                     mut adaptive: ResMut<super::band::AdaptiveBand>| {
                                        adaptive.0 = !adaptive.0;
                                    },
                                ));
                                row.spawn_empty().apply_scene(bsn! {
                                    Text({String::from(if adaptive_band {
                                        loc.msg("jam-adaptive-band-on")
                                    } else {
                                        loc.msg("jam-adaptive-band-off")
                                    })})
                                    TextFont { font_size: {FontSize::Px(15.0)} }
                                    TextColor({Color::srgb(0.70, 0.70, 0.80)})
                                    ~{super::band::AdaptiveBandLabel}
                                });
                            });
                        }
                        left.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                ..default()
                            },
                            JamGuidePanel,
                            if guides_visible { Visibility::Visible } else { Visibility::Hidden },
                        ))
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
                        left.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                row_gap: Val::Px(6.0),
                                ..default()
                            },
                            JamGuidePanel,
                            if guides_visible { Visibility::Visible } else { Visibility::Hidden },
                        ))
                        .with_children(|metro| {
                            spawn_metronome(metro, &loc, beats_per_bar, bpm);
                        });
                        // Only for a generated jam — a real song has no
                        // `Genre` concept attached to it (see
                        // `jam::rhythm_guide`'s own doc comment).
                        if generated.is_some() {
                            left.spawn((
                                Node::default(),
                                JamGuidePanel,
                                if guides_visible {
                                    Visibility::Visible
                                } else {
                                    Visibility::Hidden
                                },
                            ))
                            .with_children(|guide| spawn_rhythm_guide(guide, &loc, jam_genre.0));
                        }
                        left.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                            JamGuidePanel,
                            if guides_visible { Visibility::Visible } else { Visibility::Hidden },
                        ))
                        .with_children(|diagnostics| {
                            harmonicon_ui::spectrogram::spawn_style_toggle(
                                diagnostics,
                                *spectrogram_style,
                                &loc,
                            );
                            diagnostics
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_grow: 1.0,
                                    ..default()
                                })
                                .with_children(|spec| {
                                    spawn_spectrogram(spec, *spectrogram_style, &osc_material.0);
                                });
                        });
                    });

                // ── Right half: everything harmonica — the bend diagram and the
                // live-tinted hole map both name/track holes on the same
                // instrument, so they share this column rather than splitting
                // across both halves.
                columns
                    .spawn((
                        Node {
                            width: Val::Percent(50.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::top(Val::Px(BAR_HEIGHT)),
                            ..default()
                        },
                        JamGuidePanel,
                        if guides_visible { Visibility::Visible } else { Visibility::Hidden },
                    ))
                    .with_children(|right| {
                        spawn_harmonica_overlay(right, harp, &loc);
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
            if let Some(tracks) = &manifest.backing_stems {
                spawn_backing_stem_row(root, tracks, &loc);
            }
        });

    commands.insert_resource(guide);
    commands.insert_resource(JamChordSequence(chords));

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
        harp.hole_count(),
        &[],
        &[],
    );

    // Jam already shows the harp hint on the persistent left panel, so the
    // countdown doesn't repeat it.
    spawn_countdown(&mut commands, &loc, None, None);
}

// ── Music loop toggle ────────────────────────────────────────────────────────

/// Whether Jam Session should restart its background music from the top when
/// it reaches the end, instead of just letting it stop. Off by default; a
/// user preference that (intentionally) persists across songs within a jam.
#[derive(Resource, Default)]
pub struct JamLoop(pub bool);

/// The "Loop: on/off" readout, kept in step with [`JamLoop`].
#[derive(Component, Default, Clone)]
pub struct JamLoopLabel;

/// Generated-jam transport state. A scheduled ending names the first bar of
/// the next chorus; once reached, `ended_at_secs` pins the free-running jam
/// clock after the tonic punctuation has fired.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct JamEnding {
    stop_at_bar: Option<usize>,
    ended_at_secs: Option<f64>,
}

#[derive(Component, Default, Clone)]
pub struct JamEndingLabel;

#[derive(Component, Default, Clone)]
pub struct JamFormPosition;

fn next_chorus_boundary(absolute_bar: usize) -> usize {
    (absolute_bar / 12 + 1) * 12
}

/// Keeps the "Loop: ..." readout in step with the toggle.
pub fn update_jam_loop_label(
    jam_loop: Res<JamLoop>,
    loc: Res<Localization>,
    added: Query<(), Added<JamLoopLabel>>,
    mut labels: Query<&mut Text, With<JamLoopLabel>>,
) {
    // `JamLoop` persists across jams, so a freshly spawned label has to read
    // the current state rather than a default.
    if !jam_loop.is_changed() && added.is_empty() {
        return;
    }
    let want = loc.msg(if jam_loop.0 { "jam-loop-on" } else { "jam-loop-off" });
    for mut text in &mut labels {
        if text.0 != *want {
            text.0.clear();
            text.0.push_str(&want);
        }
    }
}

pub fn update_jam_status_labels(
    absolute: Res<harmonicon_gameplay::gameplay::AbsoluteBar>,
    ending: Res<JamEnding>,
    loc: Res<Localization>,
    mut positions: Query<&mut Text, (With<JamFormPosition>, Without<JamEndingLabel>)>,
    mut ending_labels: Query<&mut Text, (With<JamEndingLabel>, Without<JamFormPosition>)>,
) {
    if absolute.is_changed() || ending.is_changed() {
        for mut text in &mut positions {
            *text = Text::new(String::from(loc.msg_args(
                "jam-form-position",
                &[
                    ("chorus", (absolute.0 / 12 + 1).to_string()),
                    ("bar", (absolute.0 % 12 + 1).to_string()),
                ],
            )));
        }
    }
    if ending.is_changed() {
        let key = if ending.ended_at_secs.is_some() {
            "jam-ended"
        } else if ending.stop_at_bar.is_some() {
            "jam-ending-after-chorus"
        } else {
            "jam-keep-playing"
        };
        for mut text in &mut ending_labels {
            *text = Text::new(String::from(loc.msg(key)));
        }
    }
}

/// Whether the jam's music should be (re)spawned right now: the jam has
/// started, Loop is on, and no `MusicPlayer` entity is currently alive (i.e.
/// the previous playthrough already finished and despawned itself — see
/// `restart_finished_jam_music`). Split out as a pure predicate so the
/// decision is unit-testable without spinning up an `App`.
fn should_restart_jam_music(
    loop_on: bool,
    generated: bool,
    ending_active: bool,
    music_started: bool,
    music_player_alive: bool,
) -> bool {
    music_started && (loop_on || generated) && !ending_active && !music_player_alive
}

/// Restarts the jam's background music once the current playthrough has
/// *finished on its own* — the `MusicPlayer` entity despawns itself via
/// `PlaybackSettings::DESPAWN` — and either a picked song's Loop is on or a
/// generated jam has not been asked to end. This system never touches a live
/// sink, only ever spawning a *new* entity after the old one is gone — seeking
/// or restarting a still-playing sink is unreliable in `bevy_audio`.
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
    generated: Option<Res<GeneratedJamSession>>,
    ending: Res<JamEnding>,
    existing: Query<(), With<MusicPlayer>>,
    mut clock: ResMut<GameplayClock>,
    mut commands: Commands,
) {
    if !should_restart_jam_music(
        jam_loop.0,
        generated.is_some(),
        ending.stop_at_bar.is_some() || ending.ended_at_secs.is_some(),
        music_started.0,
        !existing.is_empty(),
    ) {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    // A song with neither `song/*.ogg`/`*.wav` nor `backing_stems` never had
    // a `MusicPlayer` to begin with (see `countdown_overlay::
    // update_countdown`) — nothing to loop.
    if manifest.music.is_none() && manifest.backing_stems.is_none() {
        return;
    }
    // A finite picked song really loops back to its own start. A generated
    // backing is four choruses cut into one buffer; respawning that buffer
    // continues the open jam at chorus nine, so its form clock must not jump.
    if generated.is_none() {
        clock.set_free(0.0);
    }
    if let Some(music) = manifest.music.clone() {
        commands.spawn((
            AudioPlayer::<AudioSource>(music),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
            MusicPlayer,
            GameplayRoot,
        ));
    } else if let Some(tracks) = &manifest.backing_stems {
        // Mute state (`JamStemMute`) isn't touched here — it's a resource
        // independent of any particular sink, so a track muted before the
        // loop stays muted after it without needing to be re-applied.
        for (index, track) in tracks.iter().enumerate() {
            commands.spawn((
                AudioPlayer::<AudioSource>(track.source.clone()),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
                MusicPlayer,
                BackingStemPlayer(index),
                GameplayRoot,
            ));
        }
    }
}

/// Stops a generated backing at the requested chorus boundary and resolves
/// its turnaround with a tonic rhythm-section hit. Afterwards the free-run
/// clock is held at that boundary so the form display does not wander on in
/// silence; Restart/Quit remain available through the normal pause control.
pub fn finish_generated_jam_at_chorus(
    absolute: Res<harmonicon_gameplay::gameplay::AbsoluteBar>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    audio: Res<AudioSettings>,
    progression: Res<JamProgression>,
    genre: Res<JamGenre>,
    mut sources: ResMut<Assets<AudioSource>>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
    mut ending: ResMut<JamEnding>,
    mut clock: ResMut<GameplayClock>,
    mut commands: Commands,
) {
    if let Some(ended) = ending.ended_at_secs {
        clock.set_free(ended);
        return;
    }
    let Some(stop_at) = ending.stop_at_bar else {
        return;
    };
    if absolute.0 < stop_at {
        return;
    }
    for sink in &sinks {
        sink.stop();
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let key = &manifest.chart.song.key;
    let quality = progression_bars(key, progression.0)[0].1;
    for (index, pcm) in generate_ending_stems(key, quality, manifest.chart.song.tempo_bpm, genre.0)
        .into_iter()
        .enumerate()
    {
        if pcm.is_empty() {
            continue;
        }
        let source = sources.add(AudioSource {
            bytes: harmonicon_core::wav::encode_wav(&pcm, super::backing::SAMPLE_RATE).into(),
        });
        commands.spawn((
            AudioPlayer::<AudioSource>(source),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
            MusicPlayer,
            BackingStemPlayer(index),
            GameplayRoot,
        ));
    }
    let ended = clock.get();
    ending.ended_at_secs = Some(ended);
    clock.set_free(ended);
}

#[cfg(test)]
mod tests;
