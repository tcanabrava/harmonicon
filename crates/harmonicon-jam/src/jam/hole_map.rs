// SPDX-License-Identifier: MIT

//! Jam Session's live harmonica hole map: the per-jam lookup of which holes
//! sound which notes and how each note sits against the scale and the
//! bar's chord ([`JamHoleGuide`]), the bottom-strip hole cells it tints from
//! the mic every frame ([`update_hole_map`]), and the note-class helpers
//! `improv`, `position_guide` and `call_response` share so no two of them
//! can classify a note differently.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use harmonicon_audio::pitch_detect::PitchInfo;
use harmonicon_core::chart::{Action, Scale};
use harmonicon_core::harmonica::{
    ChordQuality, Harmonica, Progression, chord_intervals, progression_bars, semitone,
};
use harmonicon_gameplay::gameplay::{ActivePitches, CurrentBar};
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::call_response::phrase::{PlayableNote, playable_notes};
use super::improv::classify_note_fit;

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
    /// The same blow/draw vocabulary as `note_to_holes`, kept per (hole,
    /// direction) for `call_response::phrase`, whose calls need to know
    /// which hole and breath a pitch takes to keep consecutive notes
    /// reachable.
    pub(crate) playable: Vec<PlayableNote>,
    pub(crate) scale_classes: HashSet<String>,
    pub(crate) chord_tones_by_bar: [HashSet<String>; 12],
}

/// One hole cell in the map; its background is tinted each frame by play state.
#[derive(Component, Default, Clone)]
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
/// `pub(crate)` so `jam::call_response`'s phrase generator classifies a
/// MIDI pitch's note class the same way the hole map's own tint does.
pub(crate) fn note_class(note: &str) -> &str {
    note.trim_end_matches(|c: char| c.is_ascii_digit())
}

/// The four note classes of `quality`'s chord rooted on `chord_root` (root,
/// 3rd, 5th, 7th — see `song::harmonica::chord_intervals`).
fn chord_tone_classes(chord_root: &str, quality: ChordQuality) -> HashSet<String> {
    chord_intervals(quality).iter().map(|&n| semitone(chord_root, n)).collect()
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
    let mut holes = Vec::with_capacity(harp.hole_count() as usize);

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
            playable: playable_notes(harp),
            scale_classes,
            chord_tones_by_bar,
        },
    )
}

/// Spawn the bottom-strip hole map: a row of cells (blow note, hole number, draw
/// note), with in-scale notes tinted green as a static guide.
pub(crate) fn spawn_hole_map(
    parent: &mut ChildSpawnerCommands,
    holes: &[HoleInfo],
    loc: &Localization,
) {
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
            col.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg("jam-hole-map-hint"))})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.70, 0.70, 0.80)})
            });
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for h in holes {
                    row.spawn_empty()
                        .apply_scene(bsn! {
                            Node {
                                width: {Val::Px(50.0)},
                                flex_direction: {FlexDirection::Column},
                                align_items: {AlignItems::Center},
                                justify_content: {JustifyContent::Center},
                                row_gap: {Val::Px(2.0)},
                                padding: {UiRect::all(Val::Px(4.0))},
                            }
                            BackgroundColor({HOLE_DEFAULT})
                            JamHoleCell { hole: {h.hole} }
                        })
                        .with_children(|cell| {
                            cell.spawn_empty().apply_scene(bsn! {
                                Text({note_class(&h.blow).to_string()})
                                TextFont { font_size: {FontSize::Px(15.0)} }
                                TextColor({if h.blow_in_scale {
                                    LABEL_IN_SCALE
                                } else {
                                    LABEL_OUT_SCALE
                                }})
                            });
                            cell.spawn_empty().apply_scene(bsn! {
                                Text({h.hole.to_string()})
                                TextFont { font_size: {FontSize::Px(16.0)} }
                                TextColor({Color::WHITE})
                            });
                            cell.spawn_empty().apply_scene(bsn! {
                                Text({note_class(&h.draw).to_string()})
                                TextFont { font_size: {FontSize::Px(15.0)} }
                                TextColor({if h.draw_in_scale {
                                    LABEL_IN_SCALE
                                } else {
                                    LABEL_OUT_SCALE
                                }})
                            });
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
    let mut lit = [None; 256];
    for p in &active.0 {
        if let Some(holes) = guide.note_to_holes.get(&p.midi) {
            let fit = classify_note_fit(&p.note, chord_tones, &guide.scale_classes);
            for &h in holes {
                let slot = &mut lit[h as usize];
                if slot.is_none_or(|previous| fit > previous) {
                    *slot = Some(fit);
                }
            }
        }
    }

    let ghost_holes: &[u8] =
        call_response.as_deref().map(|s| s.lick_holes.as_slice()).unwrap_or(&[]);

    for (cell, mut bg) in &mut cells {
        let color = match lit[cell.hole as usize] {
            Some(super::improv::NoteFit::ChordTone) => PLAY_CHORD_TONE,
            Some(super::improv::NoteFit::InScale) => PLAY_IN_SCALE,
            Some(super::improv::NoteFit::OutOfScale) => PLAY_OUT_SCALE,
            None if ghost_holes.contains(&cell.hole) => PLAY_GHOST_LICK,
            None => HOLE_DEFAULT,
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

// ── Compact detected-note indicator ─────────────────────────────────────────

/// The default stage's one line of live feedback: which hole and breath the
/// mic hears, tinted by the same fit the hole map uses. Restrained on
/// purpose — no history, no counts, and a rest reads as a plain dash.
#[derive(Component, Default, Clone)]
pub struct JamDetectedNote;

/// Colour for a rest — nothing sounding.
const DETECTED_NONE: Color = Color::srgb(0.45, 0.45, 0.55);

/// Which sounding pitch the indicator names: the lowest one the harp can
/// actually sound (a chord's root; a single note itself), so a
/// tongue-blocked chord doesn't flicker between its holes frame to frame.
/// `None` when nothing playable is sounding.
pub(crate) fn lowest_sounding<'a>(
    active: &'a [PitchInfo],
    playable: &[PlayableNote],
) -> Option<(&'a PitchInfo, PlayableNote)> {
    active
        .iter()
        .filter_map(|p| playable.iter().find(|n| n.midi == p.midi).map(|n| (p, *n)))
        .min_by_key(|(_, n)| (n.midi, n.hole))
}

/// Spawns the indicator, resting.
pub(crate) fn spawn_detected_note(parent: &mut ChildSpawnerCommands, loc: &Localization) {
    parent.spawn_empty().apply_scene(bsn! {
        Text({String::from(loc.msg("jam-detected-none"))})
        TextFont { font_size: {FontSize::Px(20.0)} }
        TextColor({DETECTED_NONE})
        JamDetectedNote
    });
}

/// Names the hole/breath the mic hears and tints it by fit — chord tone,
/// in scale, outside — writing only when the reading changes.
pub fn update_detected_note(
    active: Res<ActivePitches>,
    guide: Option<Res<JamHoleGuide>>,
    current: Res<CurrentBar>,
    loc: Res<Localization>,
    mut labels: Query<(&mut Text, &mut TextColor), With<JamDetectedNote>>,
) {
    let Some(guide) = guide else {
        return;
    };
    let (reading, color) = match lowest_sounding(&active.0, &guide.playable) {
        Some((pitch, note)) => {
            let key = if note.blow { "jam-detected-blow" } else { "jam-detected-draw" };
            let fit = classify_note_fit(
                &pitch.note,
                &guide.chord_tones_by_bar[current.0],
                &guide.scale_classes,
            );
            (
                loc.msg_args(key, &[("hole", note.hole.to_string())]),
                match fit {
                    super::improv::NoteFit::ChordTone => PLAY_CHORD_TONE,
                    super::improv::NoteFit::InScale => PLAY_IN_SCALE,
                    super::improv::NoteFit::OutOfScale => PLAY_OUT_SCALE,
                },
            )
        }
        None => (loc.msg("jam-detected-none"), DETECTED_NONE),
    };
    for (mut text, mut tint) in &mut labels {
        if text.0 != *reading {
            text.0 = reading.to_string();
        }
        if tint.0 != color {
            tint.0 = color;
        }
    }
}

#[cfg(test)]
mod tests;
