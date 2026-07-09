// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::{
    dialogs::button,
    menu::SelectedSong,
    settings::AudioSettings,
    song::SongManifest,
    song::chart::Action,
    song::harmonica::{Harmonica, blues_scale_classes, harp_banner, semitone, twelve_bar},
    theme::LoadedTheme,
};

use crate::spectrogram::{OscMaterial, SpectrogramStyle, spawn_spectrogram};

use super::countdown_overlay::spawn_countdown;
use super::harmonica_overlay::spawn_harmonica_overlay;
use super::metronome_overlay::spawn_metronome;
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{
    ActivePitches, COUNTDOWN, GameplayClock, GameplayRoot, MusicPlayer, MusicStarted,
    current_bar_index, secs_per_bar,
};

/// Free-play screen: left half shows the 12-bar chart and the metronome stacked
/// vertically; the right half is reserved for a future jam feature. The shared
/// gameplay clock/music/pause systems run for this mode too, so the chart tracks
/// the song and the metronome clicks — there are just no falling notes.
pub fn setup(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut clock: ResMut<super::GameplayClock>,
    mut music_started: ResMut<MusicStarted>,
    spectrogram_style: Res<SpectrogramStyle>,
    osc_material: Res<OscMaterial>,
    theme: Res<LoadedTheme>,
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
    let chords = twelve_bar(key);
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
    let (holes_info, guide) = build_hole_guide(&chart.harmonica, key, bpm, beats_per_bar);

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
                padding: UiRect::all(Val::Px(16.0)),
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
                        Text::new("Loop: off"),
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
                        &GridConfig::for_2d(),
                        theme.twelve_bar_colors(),
                    );
                    spawn_hole_map(grid, &holes_info);
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
            });

            // ── Right half: harmonica bend diagram (top) + live spectrogram ──
            root.spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|right| {
                spawn_harmonica_overlay(right, &chart.harmonica);
                right
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

    commands.insert_resource(guide);
    // Jam already shows the harp hint on the persistent left panel, so the
    // countdown doesn't repeat it.
    spawn_countdown(&mut commands, None);
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
    mut labels: Query<&mut Text, With<JamLoopLabel>>,
) {
    if !jam_loop.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(if jam_loop.0 { "Loop: on" } else { "Loop: off" });
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
pub fn restart_finished_jam_music(
    jam_loop: Res<JamLoop>,
    music_started: Res<MusicStarted>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    audio: Res<AudioSettings>,
    existing: Query<(), With<MusicPlayer>>,
    mut commands: Commands,
) {
    if !should_restart_jam_music(jam_loop.0, music_started.0, !existing.is_empty()) {
        return;
    }
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    commands.spawn((
        AudioPlayer::<AudioSource>(manifest.music.clone()),
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
    bpm: f32,
    beats_per_bar: usize,
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

/// The four note classes of the dominant-7th chord rooted on `chord_root`
/// (root, major 3rd, perfect 5th, minor 7th) — every chord in a standard
/// 12-bar blues (I7, IV7, V7) is dominant 7th.
fn chord_tone_classes(chord_root: &str) -> HashSet<String> {
    [0, 4, 7, 10]
        .iter()
        .map(|&n| semitone(chord_root, n))
        .collect()
}

/// Build the per-hole render data and the live-feedback lookup from the harp
/// layout, the song key, and its tempo (needed to track which bar — and thus
/// which chord — is currently sounding).
fn build_hole_guide(
    harp: &Harmonica,
    key: &str,
    bpm: f32,
    beats_per_bar: usize,
) -> (Vec<HoleInfo>, JamHoleGuide) {
    let dash = "\u{2014}";
    let scale_classes = blues_scale_classes(key);
    let chord_tones_by_bar: [HashSet<String>; 12] = {
        let chords = twelve_bar(key);
        std::array::from_fn(|i| chord_tone_classes(&chords[i]))
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
            bpm,
            beats_per_bar,
        },
    )
}

/// Spawn the bottom-strip hole map: a row of cells (blow note, hole number, draw
/// note), with in-scale notes tinted green as a static guide.
fn spawn_hole_map(parent: &mut ChildSpawnerCommands, holes: &[HoleInfo]) {
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
                Text::new("Your harmonica  \u{00B7}  gold = chord tone right now  \u{00B7}  green = blues-scale note  \u{00B7}  top blow / bottom draw"),
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
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum NoteFit {
    OutOfScale,
    InScale,
    ChordTone,
}

/// Tint each hole cell from the live mic pitches, three tiers: gold if the
/// sounding note is a tone of the chord currently sounding (the most targeted
/// choice — chord-tone awareness, not just scale membership), green if it's
/// elsewhere in the blues scale, amber if outside the scale, default when
/// silent. Reuses the same `ActivePitches` the scored modes detect.
pub fn update_hole_map(
    active: Res<ActivePitches>,
    guide: Option<Res<JamHoleGuide>>,
    clock: Res<GameplayClock>,
    mut cells: Query<(&JamHoleCell, &mut BackgroundColor)>,
) {
    let Some(guide) = guide else {
        return;
    };
    let spb = secs_per_bar(guide.bpm as f64, guide.beats_per_bar as f64);
    let bar = current_bar_index(clock.get(), spb);
    let chord_tones = &guide.chord_tones_by_bar[bar];

    // Map each currently-lit hole to the best fit among all notes sounding it.
    let mut lit: HashMap<u8, NoteFit> = HashMap::new();
    for p in &active.0 {
        if let Some(holes) = guide.note_to_holes.get(&p.midi) {
            let fit = if chord_tones.contains(&p.note) {
                NoteFit::ChordTone
            } else if guide.scale_classes.contains(&p.note) {
                NoteFit::InScale
            } else {
                NoteFit::OutOfScale
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── should_restart_jam_music ─────────────────────────────────────────────

    #[test]
    fn restarts_once_finished_when_loop_is_on() {
        assert!(should_restart_jam_music(true, true, false));
    }

    #[test]
    fn does_not_restart_while_still_playing() {
        assert!(!should_restart_jam_music(true, true, true));
    }

    #[test]
    fn does_not_restart_when_loop_is_off() {
        assert!(!should_restart_jam_music(false, true, false));
    }

    #[test]
    fn does_not_restart_before_the_jam_has_started() {
        assert!(!should_restart_jam_music(true, false, false));
    }

    /// Standard Richter C diatonic, matching `harmonica.rs`'s test layout.
    fn c_harp() -> Harmonica {
        serde_json::from_str(
            r#"{"type":"diatonic","holes":10,"bending_profile":"richter_standard",
                "layout":{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                          "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}"#,
        )
        .unwrap()
    }

    #[test]
    fn note_class_drops_octave() {
        assert_eq!(note_class("C4"), "C");
        assert_eq!(note_class("D#5"), "D#");
        assert_eq!(note_class("A6"), "A");
    }

    #[test]
    fn guide_maps_a_shared_note_to_every_hole_that_sounds_it() {
        // On a C harp, G4 is both draw-2 and blow-3 — both holes should light.
        let (_, guide) = build_hole_guide(&c_harp(), "C", 120.0, 4);
        let mut holes = guide.note_to_holes.get(&67u8).cloned().unwrap_or_default(); // G4
        holes.sort_unstable();
        assert_eq!(holes, vec![2, 3]);
    }

    #[test]
    fn guide_marks_scale_membership_per_direction() {
        let (holes, _) = build_hole_guide(&c_harp(), "C", 120.0, 4);
        let hole1 = holes.iter().find(|h| h.hole == 1).unwrap();
        assert!(hole1.blow_in_scale, "blow C4 is the root → in scale");
        assert!(!hole1.draw_in_scale, "draw D4 (major 2nd) → outside");
        let hole2 = holes.iter().find(|h| h.hole == 2).unwrap();
        assert!(hole2.draw_in_scale, "draw G4 (the 5th) → in scale");
    }

    #[test]
    fn guide_covers_all_ten_holes() {
        let (holes, _) = build_hole_guide(&c_harp(), "C", 120.0, 4);
        assert_eq!(holes.len(), 10);
    }

    /// 12-hole chromatic, matching the fixture in `harmonica.rs`'s tests.
    fn c_chromatic_harp() -> Harmonica {
        serde_json::from_str(
            r#"{"type":"chromatic","holes":12,
                "layout":{"blow":["C4","D4","E4","F4","G4","A4","B4","C5","D5","E5","F5","G5"],
                          "draw":["D4","E4","F#4","G4","A4","B4","C#5","D5","E5","F#5","G5","A5"],
                          "blow_slide":["C#4","D#4","F4","F#4","G#4","A#4","C5","C#5","D#5","F5","F#5","G#5"],
                          "draw_slide":["D#4","F4","G4","G#4","A#4","C5","D5","D#5","F5","G5","G#5","A#5"]}}"#,
        )
        .unwrap()
    }

    #[test]
    fn guide_covers_all_twelve_holes_for_a_chromatic_harp() {
        let (holes, _) = build_hole_guide(&c_chromatic_harp(), "C", 120.0, 4);
        assert_eq!(holes.len(), 12);
    }

    #[test]
    fn chord_tone_classes_are_the_dominant_seventh() {
        // C7: C, E, G, Bb(=A#).
        let s = chord_tone_classes("C");
        assert_eq!(s.len(), 4);
        for c in ["C", "E", "G", "A#"] {
            assert!(s.contains(c), "missing {c}");
        }
        assert!(!s.contains("D"), "major 2nd is not a chord tone");
    }

    #[test]
    fn guide_indexes_chord_tones_per_bar_of_the_twelve_bar_cycle() {
        // C 12-bar: bars are [I,I,I,I,IV,IV,I,I,V,IV,I,V] (0-indexed) — see
        // `twelve_bar`. Bar 4 is IV (F7); bar 8 is V (G7).
        let (_, guide) = build_hole_guide(&c_harp(), "C", 120.0, 4);
        assert!(guide.chord_tones_by_bar[0].contains("C"), "bar 0 is I (C7)");
        assert!(
            guide.chord_tones_by_bar[4].contains("F"),
            "bar 4 is IV (F7)"
        );
        assert!(guide.chord_tones_by_bar[8].contains("G"), "bar 8 is V (G7)");
        assert!(
            !guide.chord_tones_by_bar[0].contains("F"),
            "F is not a tone of the I chord"
        );
    }

    #[test]
    fn note_fit_orders_chord_tone_above_scale_above_out_of_scale() {
        assert!(NoteFit::ChordTone > NoteFit::InScale);
        assert!(NoteFit::InScale > NoteFit::OutOfScale);
    }

    #[test]
    fn banner_derives_harp_key_from_hole_1_blow() {
        // c_harp() has no position field → the "no position" wording.
        assert_eq!(
            harp_banner(&c_harp(), "G"),
            "Use a C harmonica  \u{00B7}  key of G"
        );
    }

    #[test]
    fn banner_includes_position_when_present() {
        let harp: Harmonica = serde_json::from_str(
            r#"{"type":"diatonic","holes":10,"bending_profile":"richter_standard","position":"2nd",
                "layout":{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                          "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}}"#,
        )
        .unwrap();
        // C harp, 2nd position → you play in G: the canonical cross-harp setup.
        assert_eq!(
            harp_banner(&harp, "G"),
            "Use a C harmonica  \u{00B7}  2nd position  \u{00B7}  key of G"
        );
    }
}
