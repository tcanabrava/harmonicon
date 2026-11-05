// SPDX-License-Identifier: MIT

//! Falling piano notes aligned with a horizontal keyboard. The range
//! stays fixed at the full 88-key piano (A0–C8), independent of the song.
//! Current and upcoming keys light up and turn gold when heard.

use bevy::prelude::*;
use harmonicon_core::instrument::PlayPosition;
use harmonicon_ui::dialogs::piano_keys::{PianoKey, PianoKeyboard, paint_piano_keys};

use super::{GameplayClock, ScheduledNote, SongNotes};

/// A pending treble note's ribbon: warm amber, distinct from an unlit ivory key.
pub(super) const PIANO_COLOR: Color = Color::srgb(1.0, 0.65, 0.35);
/// A key under the hit line that is being heard.
const KEY_HELD: Color = Color::srgb(0.98, 0.80, 0.25);
/// A key about to be played.
const KEY_SOON: Color = Color::srgb(0.70, 0.80, 0.92);
/// Seconds ahead a key is shown as about to be played.
const SOON_SECS: f64 = 0.6;
/// The key a note plays, if it is a piano's.
fn key_of(note: &ScheduledNote) -> Option<u8> {
    match note.position {
        PlayPosition::Key(key) => Some(key),
        _ => None,
    }
}

/// How the keyboard shows `midi` at `now`: under the hit line (held or
/// not), about to be played, or plain.
pub(super) fn key_state(notes: &[ScheduledNote], midi: u8, now: f64) -> Option<Color> {
    let ours = notes.iter().filter(|n| n.playable && !n.missed && key_of(n) == Some(midi));
    let mut soon = false;
    for note in ours {
        if note.time <= now && now < note.time + note.duration {
            return Some(if note.hit { KEY_HELD } else { hand_color(midi) });
        }
        soon |= note.time > now && note.time - now <= SOON_SECS;
    }
    soon.then_some(KEY_SOON)
}

/// Lights the play highway's keyboard from the notes and the clock.
pub(super) fn update_piano_keyboard(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    keyboards: Query<Entity, With<PianoKeyboard>>,
    mut keys: Query<(&PianoKey, &mut BackgroundColor)>,
) {
    let now = clock.get();
    // Only near the clock is anything lit, so the search stays short.
    let window: Vec<ScheduledNote> = notes
        .notes
        .iter()
        .filter(|n| n.time <= now + SOON_SECS && n.time + n.duration >= now)
        .cloned()
        .collect();
    for keyboard in &keyboards {
        paint_piano_keys(keyboard, &mut keys, |midi| key_state(&window, midi, now));
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
            harmonicon_ui::dialogs::piano_keys::spawn_piano_keys(
                p,
                harmonicon_ui::dialogs::piano_keys::KeyboardSpec {
                    low,
                    high,
                    axis: harmonicon_ui::dialogs::piano_keys::KeyboardAxis::Horizontal,
                    label_cs: true,
                },
            );
        });
}

/// The teaching split used by the two-staff practice view: bass below
/// middle C, treble at and above it. Crossing hands needs authored voices.
pub(super) fn hand_color(midi: u8) -> Color {
    if midi < 60 { Color::srgb(0.40, 0.70, 1.0) } else { PIANO_COLOR }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_keyboard_and_highway_share_all_88_key_positions() {
        use harmonicon_core::instrument::LaneLayout;
        use harmonicon_ui::dialogs::piano_keys::{KeyboardAxis, KeyboardSpec, key_rect};
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
        assert_eq!(key_state(&notes, 60, 1.2), Some(hand_color(60)));
        notes[0].hit = true;
        assert_eq!(key_state(&notes, 60, 1.2), Some(KEY_HELD));
        assert_eq!(key_state(&notes, 64, 1.6), Some(KEY_SOON), "less than SOON_SECS away");
        assert_eq!(key_state(&notes, 64, 0.5), None, "too far off yet");
        notes[1].missed = true;
        assert_eq!(key_state(&notes, 64, 2.2), None, "a missed note lights nothing");
    }
}
