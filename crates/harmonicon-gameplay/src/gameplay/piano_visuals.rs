// SPDX-License-Identifier: MIT

//! The piano in Play 2D: a piano roll, one row per semitone across the
//! song's range (`LaneLayout::keyboard_rows`, [`roll_range`]), travelled
//! right to left, with the keyboard drawn at the rows' start
//! (`harmonicon_ui::dialogs::piano_keys`, the same widget the Song Editor
//! and the Note Trainer draw), each key level with its row. The keyboard
//! lights the keys of the notes at the hit line, gold once heard, and the
//! keys of the notes about to arrive faintly, so the hand is ready.

use bevy::prelude::*;
use harmonicon_core::instrument::PlayPosition;
use harmonicon_core::instrument::piano::is_black;
use harmonicon_ui::dialogs::piano_keys::{PianoKey, PianoKeyboard, paint_piano_keys};

use super::{GameplayClock, ScheduledNote, SongNotes};

/// A pending piano note's ribbon: ivory, the colour of the keys.
pub(super) const PIANO_COLOR: Color = Color::srgb(0.93, 0.90, 0.80);
/// A key under the hit line.
const KEY_NOW: Color = Color::srgb(0.55, 0.78, 1.0);
/// A key under the hit line that is being heard.
const KEY_HELD: Color = Color::srgb(0.98, 0.80, 0.25);
/// A key about to be played.
const KEY_SOON: Color = Color::srgb(0.70, 0.80, 0.92);
/// Seconds ahead a key is shown as about to be played.
const SOON_SECS: f64 = 0.6;
/// The fewest rows the roll has: an octave and a key, so a tune of a few
/// notes still reads as a keyboard.
const MIN_ROWS: u8 = 13;

/// The keys the roll spans for a song sounding `pitches`: its lowest to its
/// highest, widened to at least [`MIN_ROWS`] around their middle, and out
/// to a white key at either end, as a keyboard drawing starts and ends.
pub(super) fn roll_range(pitches: impl IntoIterator<Item = u8>) -> (u8, u8) {
    let mut pitches = pitches.into_iter().peekable();
    let Some(&first) = pitches.peek() else { return (60, 72) };
    let (mut low, mut high) = pitches.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
    while high - low + 1 < MIN_ROWS {
        if (high - low) % 2 == 0 {
            high = high.saturating_add(1)
        } else {
            low = low.saturating_sub(1)
        }
    }
    while is_black(low) {
        low -= 1;
    }
    while is_black(high) {
        high += 1;
    }
    (low, high)
}

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
            return Some(if note.hit { KEY_HELD } else { KEY_NOW });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn note(time: f64, key: u8) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 0.5,
            playable: true,
            position: PlayPosition::Key(key),
            ..default()
        }
    }

    /// A tune from E4 to G4 gets an octave's rows around it, starting and
    /// ending on white keys; a wide one keeps its own span.
    #[test]
    fn the_roll_spans_the_song_widened_to_an_octave_on_white_keys() {
        let (low, high) = roll_range([64, 65, 67]);
        assert!(high - low + 1 >= MIN_ROWS);
        assert!(low <= 64 && high >= 67);
        assert!(!is_black(low) && !is_black(high));
        assert_eq!(roll_range([36, 96]), (36, 96));
        assert_eq!(roll_range([61, 85]), (60, 86), "out to the white keys either side");
        assert_eq!(roll_range(std::iter::empty()), (60, 72));
    }

    #[test]
    fn a_key_lights_at_the_line_gold_once_heard_and_faintly_before() {
        let mut notes = vec![note(1.0, 60), note(2.0, 64)];
        assert_eq!(key_state(&notes, 60, 1.2), Some(KEY_NOW));
        notes[0].hit = true;
        assert_eq!(key_state(&notes, 60, 1.2), Some(KEY_HELD));
        assert_eq!(key_state(&notes, 64, 1.6), Some(KEY_SOON), "less than SOON_SECS away");
        assert_eq!(key_state(&notes, 64, 0.5), None, "too far off yet");
        notes[1].missed = true;
        assert_eq!(key_state(&notes, 64, 2.2), None, "a missed note lights nothing");
    }
}
