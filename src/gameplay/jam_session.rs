// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::{
    assets_management::GlobalFonts,
    menu::SelectedSong,
    song::SongManifest,
    song::chart::Action,
    song::harmonica::{Harmonica, harp_banner, semitone, twelve_bar},
};

use crate::spectrogram::{OscMaterial, SpectrogramStyle, spawn_spectrogram};

use super::countdown_overlay::spawn_countdown;
use super::harmonica_overlay::spawn_harmonica_overlay;
use super::metronome_overlay::spawn_metronome;
use super::twelve_bar_blues_overlay::{GridConfig, spawn_12_bar_grid};
use super::{ActivePitches, COUNTDOWN, GameplayRoot, MusicStarted};

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
    fonts: Res<GlobalFonts>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        error!("SongManifest not ready when entering Jam Session");
        return;
    };
    clock.0 = -COUNTDOWN;
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
    // the hole(s) the player is currently sounding, coloured by blues-scale fit.
    let (holes_info, guide) = build_hole_guide(&chart.harmonica, key);

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
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|grid| {
                    spawn_12_bar_grid(grid, &chords, key, &fonts.gameplay, &GridConfig::for_2d());
                    spawn_hole_map(grid, &holes_info, &fonts.gameplay);
                });
                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|metro| {
                    spawn_metronome(metro, beats_per_bar, bpm, &fonts.gameplay);
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
                spawn_harmonica_overlay(right, &chart.harmonica, &fonts.gameplay);
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
    spawn_countdown(&mut commands, &fonts.gameplay, None);
}

// ── Live harmonica hole map ─────────────────────────────────────────────────────

/// Lookup driving the live hole feedback, rebuilt for each jam: which holes can
/// sound a given `note+octave`, and which note classes are in the song's blues
/// scale (so a sounding note can be coloured "in scale" vs "outside").
#[derive(Resource)]
pub struct JamHoleGuide {
    note_to_holes: HashMap<String, Vec<u8>>,
    scale_classes: HashSet<String>,
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
const PLAY_IN_SCALE: Color = Color::srgb(0.20, 0.80, 0.35);
const PLAY_OUT_SCALE: Color = Color::srgb(0.90, 0.55, 0.15);
const LABEL_IN_SCALE: Color = Color::srgb(0.45, 0.85, 0.50);
const LABEL_OUT_SCALE: Color = Color::srgb(0.50, 0.50, 0.55);

/// The note class (drop the trailing octave digit) of e.g. `"D#5"` → `"D#"`.
fn note_class(note: &str) -> &str {
    note.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// The six note classes of the blues scale rooted on `key` (1, b3, 4, b5, 5, b7).
fn blues_scale_classes(key: &str) -> HashSet<String> {
    [0, 3, 5, 6, 7, 10].iter().map(|&n| semitone(key, n)).collect()
}

/// Build the per-hole render data and the live-feedback lookup from the harp
/// layout and the song key.
fn build_hole_guide(harp: &Harmonica, key: &str) -> (Vec<HoleInfo>, JamHoleGuide) {
    let dash = "\u{2014}";
    let scale_classes = blues_scale_classes(key);
    let mut note_to_holes: HashMap<String, Vec<u8>> = HashMap::new();
    let mut holes = Vec::new();

    for hole in 1..=10u8 {
        let blow = harp.wind_direction_label(hole, &Action::Blow);
        let draw = harp.wind_direction_label(hole, &Action::Draw);
        if blow == dash && draw == dash {
            continue;
        }
        if blow != dash {
            note_to_holes.entry(blow.clone()).or_default().push(hole);
        }
        if draw != dash {
            note_to_holes.entry(draw.clone()).or_default().push(hole);
        }
        holes.push(HoleInfo {
            hole,
            blow_in_scale: scale_classes.contains(note_class(&blow)),
            draw_in_scale: scale_classes.contains(note_class(&draw)),
            blow,
            draw,
        });
    }

    (holes, JamHoleGuide { note_to_holes, scale_classes })
}

/// Spawn the bottom-strip hole map: a row of cells (blow note, hole number, draw
/// note), with in-scale notes tinted green as a static guide.
fn spawn_hole_map(parent: &mut ChildSpawnerCommands, holes: &[HoleInfo], font: &FontSource) {
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
                Text::new("Your harmonica  \u{00B7}  green = blues-scale note  \u{00B7}  top blow / bottom draw"),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
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
                            width: Val::Px(40.0),
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
                            TextFont { font_size: FontSize::Px(10.0), ..default() },
                            TextColor(if h.blow_in_scale { LABEL_IN_SCALE } else { LABEL_OUT_SCALE }),
                        ));
                        cell.spawn((
                            Text::new(h.hole.to_string()),
                            TextFont { font_size: FontSize::Px(16.0), ..default() },
                            TextColor(Color::WHITE),
                        ));
                        cell.spawn((
                            Text::new(note_class(&h.draw).to_string()),
                            TextFont { font_size: FontSize::Px(10.0), ..default() },
                            TextColor(if h.draw_in_scale { LABEL_IN_SCALE } else { LABEL_OUT_SCALE }),
                        ));
                    });
                }
            });
        });
}

/// Tint each hole cell from the live mic pitches: bright green if the sounding
/// note is in the blues scale, amber if outside, default when silent. Reuses the
/// same `ActivePitches` the scored modes detect.
pub fn update_hole_map(
    active: Res<ActivePitches>,
    guide: Option<Res<JamHoleGuide>>,
    mut cells: Query<(&JamHoleCell, &mut BackgroundColor)>,
) {
    let Some(guide) = guide else {
        return;
    };

    // Map each currently-lit hole to whether the note sounding it is in scale.
    let mut lit: HashMap<u8, bool> = HashMap::new();
    for p in &active.0 {
        let note = format!("{}{}", p.note, p.octave);
        if let Some(holes) = guide.note_to_holes.get(&note) {
            let in_scale = guide.scale_classes.contains(&p.note);
            for &h in holes {
                lit.entry(h).and_modify(|v| *v |= in_scale).or_insert(in_scale);
            }
        }
    }

    for (cell, mut bg) in &mut cells {
        bg.0 = match lit.get(&cell.hole) {
            Some(true) => PLAY_IN_SCALE,
            Some(false) => PLAY_OUT_SCALE,
            None => HOLE_DEFAULT,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn blues_scale_is_the_six_classes() {
        // C blues: C, Eb(=D#), F, Gb(=F#), G, Bb(=A#).
        let s = blues_scale_classes("C");
        for c in ["C", "D#", "F", "F#", "G", "A#"] {
            assert!(s.contains(c), "missing {c}");
        }
        assert_eq!(s.len(), 6);
        assert!(!s.contains("D"), "major 2nd is not in the blues scale");
        assert!(!s.contains("E"), "major 3rd is not in the blues scale");
    }

    #[test]
    fn guide_maps_a_shared_note_to_every_hole_that_sounds_it() {
        // On a C harp, G4 is both draw-2 and blow-3 — both holes should light.
        let (_, guide) = build_hole_guide(&c_harp(), "C");
        let mut holes = guide.note_to_holes.get("G4").cloned().unwrap_or_default();
        holes.sort_unstable();
        assert_eq!(holes, vec![2, 3]);
    }

    #[test]
    fn guide_marks_scale_membership_per_direction() {
        let (holes, _) = build_hole_guide(&c_harp(), "C");
        let hole1 = holes.iter().find(|h| h.hole == 1).unwrap();
        assert!(hole1.blow_in_scale, "blow C4 is the root → in scale");
        assert!(!hole1.draw_in_scale, "draw D4 (major 2nd) → outside");
        let hole2 = holes.iter().find(|h| h.hole == 2).unwrap();
        assert!(hole2.draw_in_scale, "draw G4 (the 5th) → in scale");
    }

    #[test]
    fn guide_covers_all_ten_holes() {
        let (holes, _) = build_hole_guide(&c_harp(), "C");
        assert_eq!(holes.len(), 10);
    }

    #[test]
    fn banner_derives_harp_key_from_hole_1_blow() {
        // c_harp() has no position field → the "no position" wording.
        assert_eq!(harp_banner(&c_harp(), "G"), "Use a C harmonica  \u{00B7}  key of G");
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
