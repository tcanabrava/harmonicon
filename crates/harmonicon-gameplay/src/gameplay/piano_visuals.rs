// SPDX-License-Identifier: MIT

//! Falling piano notes aligned with a horizontal keyboard. The range
//! stays fixed at the full 88-key piano (A0–C8), independent of the song.
//! Current and upcoming keys light up and turn gold when heard.

use bevy::prelude::*;
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::instrument::piano::is_black;
use harmonicon_ui::dialogs::piano_keys::{
    KeyboardAxis, KeyboardSpec, PianoKey, key_rect, paint_piano_keys, spawn_piano_keys,
};

use super::{GameplayClock, ScheduledNote, SongNotes, piano_fingering::MIDDLE_C};

/// A pending treble note's ribbon: warm amber, distinct from an unlit ivory key.
pub(super) const PIANO_COLOR: Color = Color::srgb(1.0, 0.65, 0.35);
/// A key under the hit line that is being heard.
const KEY_HELD: Color = Color::srgb(0.98, 0.80, 0.25);
/// A key about to be played.
const KEY_SOON: Color = Color::srgb(0.70, 0.80, 0.92);
const MIDDLE_C_TINT: Color = Color::srgb_u8(173, 220, 204);
const MIDDLE_C_ACCENT: Color = Color::srgb_u8(7, 101, 84);

#[derive(Component)]
pub(super) struct PlayPianoKeyboard;

#[derive(Component)]
pub(super) struct PianoFingerLabel(u8);

#[derive(Clone, Copy)]
struct KeyCue {
    color: Color,
    finger: Option<u8>,
}
/// Seconds ahead a key is shown as about to be played.
const SOON_SECS: f64 = 0.6;
/// The key a note plays, if it is a piano's.
fn key_of(note: &ScheduledNote) -> Option<u8> {
    match note.position {
        PlayPosition::Key(key) => Some(key),
        _ => None,
    }
}

fn key_cues(notes: &[ScheduledNote], now: f64) -> [Option<KeyCue>; 128] {
    let mut cues = [None; 128];
    let mut next_attack = [None; 2];
    for note in notes.iter().take_while(|note| note.time <= now + SOON_SECS) {
        if !note.playable || note.missed || note.time + note.duration <= now {
            continue;
        }
        let Some(midi) = key_of(note) else { continue };
        let Some(cue) = cues.get_mut(usize::from(midi)) else { continue };
        let current = note.time <= now;
        if current {
            *cue = Some(KeyCue {
                color: if note.hit { KEY_HELD } else { hand_color(midi) },
                finger: note.suggested_piano_finger,
            });
        } else {
            let upcoming = next_attack[usize::from(midi >= MIDDLE_C)].get_or_insert(note.time);
            if note.time == *upcoming && cue.is_none() {
                *cue = Some(KeyCue { color: KEY_SOON, finger: note.suggested_piano_finger });
            }
        }
    }
    cues
}

/// Lights the play keyboard and labels the sounding and next attack of each hand.
pub(super) fn update_piano_keyboard(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    keyboards: Query<Entity, With<PlayPianoKeyboard>>,
    mut keys: Query<(&PianoKey, &mut BackgroundColor)>,
    mut labels: Query<(&PianoFingerLabel, &mut Text, &mut Visibility)>,
) {
    let Ok(keyboard) = keyboards.single() else { return };
    let cues = key_cues(&notes.notes, clock.get());
    paint_piano_keys(keyboard, &mut keys, |midi| {
        cues.get(usize::from(midi))
            .and_then(|cue| cue.map(|cue| cue.color))
            .or((midi == MIDDLE_C).then_some(MIDDLE_C_TINT))
    });
    for (label, mut text, mut visibility) in &mut labels {
        let finger = cues.get(usize::from(label.0)).and_then(|cue| (*cue)?.finger);
        let desired = if finger.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        visibility.set_if_neq(desired);
        if let Some(finger) = finger {
            const DIGITS: [&str; 5] = ["1", "2", "3", "4", "5"];
            let digit = DIGITS[usize::from(finger - 1)];
            if text.0 != digit {
                digit.clone_into(&mut text.0);
            }
        }
    }
}

pub(super) fn spawn_keyboard(parent: &mut ChildSpawnerCommands, low: u8, high: u8, height: f32) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|p| {
            let spec = KeyboardSpec { low, high, axis: KeyboardAxis::Horizontal, label_cs: true };
            let keyboard = spawn_piano_keys(p, spec);
            p.commands().entity(keyboard).insert(PlayPianoKeyboard).with_children(|overlay| {
                if (low..=high).contains(&MIDDLE_C) {
                    let (left, _, width, _) = key_rect(&spec, MIDDLE_C);
                    overlay.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(left),
                            width: Val::Percent(width),
                            bottom: Val::Px(0.0),
                            height: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(MIDDLE_C_ACCENT),
                        bevy::picking::Pickable::IGNORE,
                    ));
                }
                for midi in low..=high {
                    let (left, _, width, depth) = key_rect(&spec, midi);
                    overlay
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(left),
                                width: Val::Percent(width),
                                top: Val::Percent(depth - if is_black(midi) { 24.0 } else { 34.0 }),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            bevy::picking::Pickable::IGNORE,
                        ))
                        .with_child((
                            PianoFingerLabel(midi),
                            Visibility::Hidden,
                            Node {
                                padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(hand_color(midi)),
                            Text::new(""),
                            TextFont { font_size: FontSize::Px(14.0), ..default() },
                            TextColor(Color::srgb_u8(17, 28, 36)),
                            TextLayout::no_wrap(),
                            bevy::picking::Pickable::IGNORE,
                        ));
                }
            });
        });
}

/// The teaching split used by the two-staff practice view: bass below
/// middle C, treble at and above it. Crossing hands needs authored voices.
pub(super) fn hand_color(midi: u8) -> Color {
    if midi < MIDDLE_C { Color::srgb(0.40, 0.70, 1.0) } else { PIANO_COLOR }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_keyboard_and_highway_share_all_88_key_positions() {
        use harmonicon_core::instrument::LaneLayout;
        use harmonicon_ui::dialogs::piano_keys::PianoKeyboard;
        let layout = LaneLayout::keyboard_rows(21, 108);
        let spec =
            KeyboardSpec { low: 21, high: 108, axis: KeyboardAxis::Horizontal, label_cs: true };
        for midi in 21..=108 {
            let lane = layout.lane_of(&PlayPosition::Key(midi)).unwrap();
            let (left, width) = layout.span(lane);
            let (key_left, _, key_width, _) = key_rect(&spec, midi);
            assert!((left - key_left).abs() < 0.001, "key {midi} left");
            assert!((width - key_width).abs() < 0.001, "key {midi} width");
        }
        let mut app = App::new();
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|p| spawn_keyboard(p, 21, 108, 96.0));
        app.world_mut().flush();
        let world = app.world_mut();
        assert_eq!(world.query::<&PianoKeyboard>().iter(world).count(), 1);
        assert_eq!(world.query::<&PianoKey>().iter(world).count(), 88);
    }

    fn note(time: f64, key: u8) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 0.5,
            playable: true,
            position: PlayPosition::Key(key),
            ..default()
        }
    }

    #[test]
    fn a_key_lights_at_the_line_gold_once_heard_and_faintly_before() {
        let mut notes = vec![note(1.0, 60), note(2.0, 64)];
        let lit = |notes: &[ScheduledNote], midi: usize, now| {
            key_cues(notes, now)[midi].map(|cue| cue.color)
        };
        assert_eq!(lit(&notes, 60, 1.2), Some(hand_color(60)));
        notes[0].hit = true;
        assert_eq!(lit(&notes, 60, 1.2), Some(KEY_HELD));
        assert_eq!(lit(&notes, 64, 1.6), Some(KEY_SOON));
        assert_eq!(lit(&notes, 64, 0.5), None);
        notes[1].missed = true;
        assert_eq!(lit(&notes, 64, 2.2), None);
    }
}
