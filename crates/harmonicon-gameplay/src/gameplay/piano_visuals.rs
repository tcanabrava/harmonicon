// SPDX-License-Identifier: MIT

//! Falling piano notes aligned with a horizontal keyboard. The range
//! expands to full C–B octaves. Both the full-piano overview and the
//! playable keyboard light upcoming/current keys and turn gold when heard.

use bevy::prelude::*;
use harmonicon_core::instrument::PlayPosition;
#[cfg(test)]
use harmonicon_core::instrument::piano::is_black;
use harmonicon_ui::dialogs::piano_keys::{PianoKey, PianoKeyboard, paint_piano_keys};

use super::{GameplayClock, ScheduledNote, SongNotes};

/// A pending treble note's ribbon: warm amber, distinct from an unlit ivory key.
pub(super) const PIANO_COLOR: Color = Color::srgb(1.0, 0.65, 0.35);
/// A key under the hit line.

/// A key under the hit line that is being heard.
const KEY_HELD: Color = Color::srgb(0.98, 0.80, 0.25);
/// A key about to be played.
const KEY_SOON: Color = Color::srgb(0.70, 0.80, 0.92);
/// Seconds ahead a key is shown as about to be played.
const SOON_SECS: f64 = 0.6;
/// The fewest rows the roll has: an octave and a key, so a tune of a few
/// notes still reads as a keyboard.

/// The keys the roll spans for a song sounding `pitches`: its lowest to its
/// highest, expanded to complete C–B octaves including unused keys.
pub(super) fn roll_range(pitches: impl IntoIterator<Item = u8>) -> (u8, u8) {
    let values: Vec<_> = pitches.into_iter().collect();
    let low = values.iter().copied().min().unwrap_or(60);
    let high = values.iter().copied().max().unwrap_or(71);
    ((low / 12) * 12, ((high / 12) * 12 + 11).min(127))
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
    mut hands: Query<(&PianoHandRegion, &mut Node)>,
) {
    let now = clock.get();
    // Only near the clock is anything lit, so the search stays short.
    let window: Vec<ScheduledNote> = notes
        .notes
        .iter()
        .filter(|n| n.time <= now + SOON_SECS && n.time + n.duration >= now)
        .cloned()
        .collect();
    for (hand, mut node) in &mut hands {
        let pitches: Vec<_> = window
            .iter()
            .filter(|n| n.playable && !n.missed)
            .filter_map(key_of)
            .filter(|key| (*key < 60) == hand.0)
            .collect();
        node.display = if pitches.is_empty() { Display::None } else { Display::Flex };
        if let (Some(low), Some(high)) = (pitches.iter().min(), pitches.iter().max()) {
            use harmonicon_ui::dialogs::piano_keys::{KeyboardAxis, KeyboardSpec, key_rect};
            let spec = KeyboardSpec {
                low: 21,
                high: 108,
                axis: KeyboardAxis::Horizontal,
                label_cs: false,
            };
            let (left, _, _, _) = key_rect(&spec, *low);
            let (right, _, width, _) = key_rect(&spec, *high);
            node.left = Val::Percent(left);
            node.width = Val::Percent(right + width - left);
        }
    }
    for keyboard in &keyboards {
        paint_piano_keys(keyboard, &mut keys, |midi| key_state(&window, midi, now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miniature_piano_marks_both_hand_ranges() {
        let mut app = App::new();
        app.init_resource::<GameplayClock>()
            .init_resource::<SongNotes>()
            .add_systems(Update, update_piano_keyboard);
        app.world_mut().resource_mut::<GameplayClock>().set_free(1.1);
        app.world_mut().resource_mut::<SongNotes>().notes = vec![note(1.0, 48), note(1.0, 64)];
        app.world_mut().commands().spawn(Node::default()).with_children(|p| {
            spawn_keyboard(p, 21, 108, 42.0);
            spawn_keyboard(p, 48, 71, 96.0);
        });
        app.world_mut().flush();
        app.update();
        let world = app.world_mut();
        let regions: Vec<_> = world.query::<(&PianoHandRegion, &Node)>().iter(world).collect();
        assert_eq!(regions.len(), 2);
        assert!(regions.iter().all(|(_, node)| node.display == Display::Flex));
        assert!(
            regions
                .iter()
                .all(|(_, node)| matches!(node.width, Val::Percent(width) if width > 0.0))
        );
        assert!(regions.iter().any(|(hand, _)| hand.0));
        assert!(regions.iter().any(|(hand, _)| !hand.0));
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

    /// A tune from E4 to G4 gets an octave's rows around it, starting and
    /// ending on white keys; a wide one keeps its own span.
    #[test]
    fn the_roll_spans_the_song_widened_to_an_octave_on_white_keys() {
        let (low, high) = roll_range([64, 65, 67]);
        assert!(high - low + 1 >= 12);
        assert!(low <= 64 && high >= 67);
        assert!(!is_black(low) && !is_black(high));
        assert_eq!(roll_range([36, 96]), (36, 107));
        assert_eq!(roll_range([61, 85]), (60, 95), "out to the white keys either side");
        assert_eq!(roll_range(std::iter::empty()), (60, 71));
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
            if height < 50.0 {
                for bass in [true, false] {
                    let color = hand_color(if bass { 48 } else { 72 });
                    p.spawn((
                        PianoHandRegion(bass),
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(0.0),
                            height: Val::Percent(100.0),
                            display: Display::None,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(color.with_alpha(0.15)),
                        BorderColor::all(color),
                        bevy::picking::Pickable::IGNORE,
                    ));
                }
            }
        });
}

/// The teaching split used by the two-staff practice view: bass below
/// middle C, treble at and above it. Crossing hands needs authored voices.
pub(super) fn hand_color(midi: u8) -> Color {
    if midi < 60 { Color::srgb(0.40, 0.70, 1.0) } else { PIANO_COLOR }
}

#[derive(Component)]
pub(super) struct PianoHandRegion(bool);
