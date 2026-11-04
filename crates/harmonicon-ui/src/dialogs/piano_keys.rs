// SPDX-License-Identifier: MIT

//! A drawn piano keyboard, shared by every view of one: the play highway's
//! piano roll, the Song Editor's, the Note Trainer. It knows nothing about
//! songs or notes, only keys: a caller says which range to draw and which
//! way it lies, and later which keys to light.
//!
//! - [`KeyboardAxis::Vertical`] is a piano roll's keyboard: pitch-aligned keys
//!   with white keys extending beneath adjacent accidentals, the highest on top, so each key is level with the roll's
//!   row for its pitch. The black keys are shorter, darker bars from the
//!   keyboard's back edge (the left), as on the instrument.
//! - [`KeyboardAxis::Horizontal`] is a keyboard as a player faces it: the
//!   white keys side by side, low to high, and the black ones narrower and
//!   shorter, between them.
//!
//! Everything is placed in percent of the keyboard's own node, so it
//! scales to whatever box the caller gives it, and the geometry is pure
//! ([`key_rect`]) so it can be tested without a window. Each key carries
//! the keyboard it belongs to ([`PianoKey::keyboard`]), so several
//! keyboards can be on screen at once and lit independently
//! ([`paint_piano_keys`]).

use bevy::prelude::*;
use harmonicon_core::instrument::piano::is_black;

/// Which way a keyboard lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardAxis {
    /// Keys stacked top to bottom, highest first, beside a piano roll.
    Vertical,
    /// Keys left to right, lowest first, as a player sees them.
    Horizontal,
}

/// What to draw: the keys from `low` to `high` (MIDI numbers, both
/// included), lying along `axis`, with each C named when `label_cs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardSpec {
    pub low: u8,
    pub high: u8,
    pub axis: KeyboardAxis,
    pub label_cs: bool,
}

/// A drawn keyboard's root.
#[derive(Component, Clone, Copy, Debug)]
pub struct PianoKeyboard(pub KeyboardSpec);

/// One drawn key: its pitch and the keyboard it belongs to.
#[derive(Component, Clone, Copy, Debug)]
pub struct PianoKey {
    pub midi: u8,
    pub keyboard: Entity,
}

const WHITE: Color = Color::srgb(0.93, 0.92, 0.88);
const BLACK: Color = Color::srgb(0.10, 0.10, 0.12);
const EDGE: Color = Color::srgba(0.0, 0.0, 0.0, 0.35);
/// A black key's length across the keyboard, as a share of a white key's.
const BLACK_LENGTH: f32 = 0.62;
/// A black key's width, as a share of a white key's (horizontal only).
const BLACK_WIDTH: f32 = 0.6;

/// A key's unlit colour.
pub fn key_colour(midi: u8) -> Color {
    if is_black(midi) { BLACK } else { WHITE }
}

/// How many white keys lie from `low` to `high`.
pub fn white_keys(low: u8, high: u8) -> usize {
    (low..=high).filter(|&m| !is_black(m)).count()
}

/// Where `midi` sits in a keyboard `spec` draws, as `(left, top, width,
/// height)` in percent of the keyboard's node.
pub fn key_rect(spec: &KeyboardSpec, midi: u8) -> (f32, f32, f32, f32) {
    let (low, high) = (spec.low.min(spec.high), spec.high.max(spec.low));
    match spec.axis {
        KeyboardAxis::Vertical => {
            let rows = f32::from(high - low + 1);
            let center = (f32::from(high.saturating_sub(midi)) + 0.5) / rows * 100.0;
            if is_black(midi) {
                return (0.0, center - 30.0 / rows, BLACK_LENGTH * 100.0, 60.0 / rows);
            }
            // White keys meet halfway through the neighbouring accidental's
            // lane. Their centres remain aligned with the piano-roll pitches.
            let above = if midi < high && is_black(midi + 1) { 100.0 / rows } else { 50.0 / rows };
            let below = if midi > low && is_black(midi - 1) { 100.0 / rows } else { 50.0 / rows };
            (0.0, center - above, 100.0, above + below)
        }
        KeyboardAxis::Horizontal => {
            let whites = white_keys(low, high).max(1) as f32;
            let white_w = 100.0 / whites;
            // The white keys below this one: a black key sits on the line
            // between the white key below it and the next.
            let below = white_keys(low, midi).saturating_sub(usize::from(!is_black(midi))) as f32;
            if is_black(midi) {
                let w = white_w * BLACK_WIDTH;
                (below * white_w - w / 2.0, 0.0, w, BLACK_LENGTH * 100.0)
            } else {
                (below * white_w, 0.0, white_w, 100.0)
            }
        }
    }
}

/// Draws a keyboard under `parent`, filling it, and returns its root.
/// White keys are spawned first so the black ones paint over them.
pub fn spawn_piano_keys(parent: &mut ChildSpawnerCommands, spec: KeyboardSpec) -> Entity {
    let (low, high) = (spec.low.min(spec.high), spec.high.max(spec.low));
    let mut root = parent.spawn((
        PianoKeyboard(spec),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(BLACK),
    ));
    let keyboard = root.id();
    root.with_children(|keys| {
        let whites = (low..=high).filter(|&m| !is_black(m));
        let blacks = (low..=high).filter(|&m| is_black(m));
        for midi in whites.chain(blacks) {
            let (left, top, width, height) = key_rect(&spec, midi);
            let edge = match spec.axis {
                KeyboardAxis::Vertical => UiRect::bottom(Val::Px(1.0)),
                KeyboardAxis::Horizontal => UiRect::right(Val::Px(1.0)),
            };
            let mut key = keys.spawn((
                PianoKey { midi, keyboard },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left),
                    top: Val::Percent(top),
                    width: Val::Percent(width),
                    height: Val::Percent(height),
                    border: if is_black(midi) { UiRect::ZERO } else { edge },
                    justify_content: JustifyContent::FlexEnd,
                    align_items: match spec.axis {
                        KeyboardAxis::Vertical => AlignItems::Center,
                        KeyboardAxis::Horizontal => AlignItems::FlexEnd,
                    },
                    padding: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(key_colour(midi)),
                BorderColor::all(EDGE),
                bevy::picking::Pickable::IGNORE,
            ));
            if spec.label_cs && midi % 12 == 0 {
                key.with_children(|label| {
                    label.spawn((
                        Text::new(harmonicon_core::midi::midi_to_note(i32::from(midi))),
                        TextFont { font_size: FontSize::Px(10.0), ..default() },
                        TextColor(Color::srgb(0.25, 0.25, 0.30)),
                        bevy::picking::Pickable::IGNORE,
                    ));
                });
            }
        }
    });
    keyboard
}

/// Lights `keyboard`'s keys: each in the colour `lit` gives it, or its own
/// when `lit` gives none. Keys of other keyboards are left alone, so each
/// view of a keyboard can show something different.
pub fn paint_piano_keys(
    keyboard: Entity,
    keys: &mut Query<(&PianoKey, &mut BackgroundColor)>,
    lit: impl Fn(u8) -> Option<Color>,
) {
    for (key, mut fill) in keys.iter_mut().filter(|(key, _)| key.keyboard == keyboard) {
        let want = lit(key.midi).unwrap_or_else(|| key_colour(key.midi));
        if fill.0 != want {
            fill.0 = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(axis: KeyboardAxis) -> KeyboardSpec {
        KeyboardSpec { low: 60, high: 72, axis, label_cs: false }
    }

    /// A piano roll's keyboard: thirteen equal bands, C5 on top, C4 at the
    /// bottom, and the black keys shorter.
    #[test]
    fn a_vertical_keyboard_places_accidentals_between_continuous_white_keys() {
        let roll = spec(KeyboardAxis::Vertical);
        let (_, top, width, height) = key_rect(&roll, 72);
        assert_eq!((top, width), (0.0, 100.0));
        assert!((height - 150.0 / 13.0).abs() < 1e-4);
        let (_, top, _, _) = key_rect(&roll, 60);
        assert!((top - 1100.0 / 13.0).abs() < 1e-3, "C extends beneath C#");
        let (_, c_top, _, _) = key_rect(&roll, 60);
        let (_, d_top, _, d_height) = key_rect(&roll, 62);
        assert!((c_top - d_top - d_height).abs() < 1e-4, "white keys meet");
        let (_, cs_top, _, cs_height) = key_rect(&roll, 61);
        assert!((cs_top + cs_height / 2.0 - c_top).abs() < 1e-4);
        assert!(key_rect(&roll, 61).2 < 100.0, "a black key is shorter");
    }

    /// As a player faces it: eight white keys side by side from C4, and a
    /// black key straddling the line between the two whites beside it.
    #[test]
    fn a_horizontal_keyboard_lays_white_keys_side_by_side_and_black_between() {
        let keys = spec(KeyboardAxis::Horizontal);
        assert_eq!(white_keys(60, 72), 8);
        let (c_left, _, c_w, _) = key_rect(&keys, 60);
        let (d_left, _, _, _) = key_rect(&keys, 62);
        assert_eq!(c_left, 0.0);
        assert!((d_left - c_w).abs() < 1e-4, "D is the second white key");
        let (cs_left, _, cs_w, cs_h) = key_rect(&keys, 61);
        assert!((cs_left + cs_w / 2.0 - c_w).abs() < 1e-4, "C# centred on the C-D line");
        assert!(cs_w < c_w && cs_h < 100.0);
    }

    #[test]
    fn an_unlit_key_is_ivory_or_ebony() {
        assert_eq!(key_colour(60), WHITE);
        assert_eq!(key_colour(61), BLACK);
    }
}
