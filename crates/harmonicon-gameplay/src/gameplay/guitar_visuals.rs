// SPDX-License-Identifier: MIT

//! Guitar-only guidance. Positions guide the hands; scoring remains pitch-only.
use super::{GameplayClock, LOOKAHEAD, ScheduledNote, SongNotes};
use bevy::prelude::*;
use harmonicon_core::instrument::{
    PlayPosition,
    guitar::{FretAssignment, Guitar, HAND_SPAN_FRETS},
};
use harmonicon_platform::localization::LocalizationExt;

/// The colour string `string` is drawn in, string 1 first: the same on the
/// play highway, its fretboard strip and the Song Editor's tab view, so a
/// string reads as one thing everywhere.
pub fn string_color(string: u8) -> Color {
    const COLORS: [(f32, f32, f32); 6] = [
        (0.70, 0.45, 0.95),
        (0.25, 0.70, 1.0),
        (1.0, 0.65, 0.20),
        (0.95, 0.85, 0.25),
        (0.30, 0.85, 0.55),
        (0.95, 0.35, 0.40),
    ];
    let (r, g, b) = COLORS[usize::from(string.saturating_sub(1)) % COLORS.len()];
    Color::srgb(r, g, b)
}

/// Current/next attack group, including holds still crossing the hit line.
/// Computed from chart time so seeks, loops and adaptive rebuilds need no reset.
fn targets(notes: &[ScheduledNote], now: f64) -> Vec<FretAssignment> {
    let eligible = |n: &&ScheduledNote| {
        matches!(n.position, PlayPosition::Fret(_))
            && n.playable
            && !n.missed
            && n.time + n.duration > now
            && n.time <= now + LOOKAHEAD
    };
    let Some(first) = notes.iter().filter(eligible).min_by(|a, b| a.time.total_cmp(&b.time)) else {
        return Vec::new();
    };
    notes
        .iter()
        .filter(eligible)
        .filter(|n| n.time == first.time)
        .filter_map(|n| if let PlayPosition::Fret(p) = n.position { Some(p) } else { None })
        .collect()
}

fn anchor(targets: &[FretAssignment], top: u8) -> u8 {
    targets
        .iter()
        .filter(|p| p.fret > 0)
        .map(|p| p.fret)
        .min()
        .unwrap_or(1)
        .min(top.saturating_sub(HAND_SPAN_FRETS).max(1))
}

#[derive(Component)]
pub(super) struct FretCell {
    string: u8,
    column: u8,
    top: u8,
}

/// A compact movable neck window, `width` px wide: beside the highway's hit
/// zone, or the Note Trainer's diagram. Column zero always shows the open
/// string; other columns follow the hand.
pub(super) fn spawn_fretboard(parent: &mut ChildSpawnerCommands, guitar: &Guitar, width: f32) {
    parent
        .spawn(Node {
            width: Val::Px(width),
            height: Val::Percent(100.0),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|board| {
            for string in 1..=guitar.string_count() {
                board
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0 / f32::from(guitar.string_count())),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        for column in 0..=HAND_SPAN_FRETS + 1 {
                            row.spawn((
                                Text::new(""),
                                TextFont { font_size: FontSize::Px(16.0), ..default() },
                                TextColor(Color::WHITE),
                                TextLayout::justify(Justify::Center),
                                Node {
                                    width: Val::Percent(100.0 / f32::from(HAND_SPAN_FRETS + 2)),
                                    padding: UiRect::vertical(Val::Px(10.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BorderColor::all(string_color(string).with_alpha(0.5)),
                                BackgroundColor(Color::srgb(0.06, 0.07, 0.10)),
                                FretCell { string, column, top: guitar.top_fret() },
                            ));
                        }
                    });
            }
        });
}

pub(super) fn update_fretboard(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    mut cells: Query<(&FretCell, &mut Text, &mut BackgroundColor)>,
) {
    if cells.is_empty() {
        return;
    }
    let targets = targets(&notes.notes, clock.get());
    for (cell, mut text, mut color) in &mut cells {
        paint_cell(cell, &targets, &mut text, &mut color);
    }
}

/// One fretboard cell, for a hand window placed over `targets`: the fret it
/// shows, lit in its string's colour where a target sits, the rest of the
/// window faintly tinted. Shared by the highway and the Note Trainer.
pub(super) fn paint_cell(
    cell: &FretCell,
    targets: &[FretAssignment],
    text: &mut Text,
    color: &mut BackgroundColor,
) {
    let fret = if cell.column == 0 { 0 } else { anchor(targets, cell.top) + cell.column - 1 };
    let label = if fret <= cell.top { fret.to_string() } else { String::new() };
    if text.0 != label {
        text.0 = label;
    }
    let paint = if targets.iter().any(|p| p.string == cell.string && p.fret == fret) {
        string_color(cell.string)
    } else if cell.column > 0 && fret <= cell.top && !targets.is_empty() {
        string_color(cell.string).with_alpha(0.13)
    } else {
        Color::srgb(0.06, 0.07, 0.10)
    };
    if color.0 != paint {
        color.0 = paint;
    }
}

/// Recognise exact pitch-class sets only; ambiguous clusters keep their fret labels.
fn chord_name(pitches: &[u8]) -> Option<String> {
    let bass = *pitches.iter().min()? % 12;
    let mut roots: Vec<_> = pitches.iter().map(|p| p % 12).collect();
    roots.sort();
    roots.dedup();
    roots.sort_by_key(|root| (*root != bass, *root));
    for root in roots {
        let mut intervals: Vec<_> = pitches.iter().map(|p| (p % 12 + 12 - root) % 12).collect();
        intervals.sort();
        intervals.dedup();
        for (shape, suffix) in [
            (&[0, 4, 7][..], ""),
            (&[0, 3, 7][..], "m"),
            (&[0, 7][..], "5"),
            (&[0, 4, 7, 10][..], "7"),
            (&[0, 3, 7, 10][..], "m7"),
            (&[0, 4, 7, 11][..], "maj7"),
        ] {
            if intervals == shape {
                let name = harmonicon_core::midi::NOTE_NAMES[usize::from(root)];
                return Some(if bass == root {
                    format!("{name}{suffix}")
                } else {
                    format!(
                        "{name}{suffix}/{}",
                        harmonicon_core::midi::NOTE_NAMES[usize::from(bass)]
                    )
                });
            }
        }
    }
    None
}

/// Only the top string owns the connecting stem; individual ribbons still
/// carry each string's own duration, technique and judgment feedback.
pub(super) fn chord_span(note: &ScheduledNote, notes: &[ScheduledNote]) -> Option<(u8, String)> {
    let PlayPosition::Fret(at) = note.position else { return None };
    if note.chord_pitches.len() < 2 {
        return None;
    }
    let strings: Vec<_> = notes
        .iter()
        .filter(|n| n.time == note.time && n.chord_pitches == note.chord_pitches)
        .filter_map(|n| if let PlayPosition::Fret(p) = n.position { Some(p.string) } else { None })
        .collect();
    if strings.iter().min().copied()? != at.string {
        return None;
    }
    let span = strings.iter().max().copied()? - at.string;
    (span > 0).then(|| (span, chord_name(&note.chord_pitches).unwrap_or_default()))
}

#[derive(Component)]
pub(super) struct ChordStem(pub f32);

pub(super) fn align_chord_stems(
    mut stems: Query<(&ChildOf, &ChordStem, &mut Node)>,
    parents: Query<&Node, Without<ChordStem>>,
) {
    for (parent, span, mut node) in &mut stems {
        if let Ok(parent) = parents.get(parent.parent())
            && let Val::Percent(height) = parent.height
            && height > 0.0
        {
            node.height = Val::Percent(span.0 * 100.0 / height);
        }
    }
}

pub(super) fn spawn_chord_stem(
    parent: &mut ChildSpawnerCommands,
    span: u8,
    lane_pct: f32,
    name: &str,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: Val::Percent(50.0),
            width: Val::Px(3.0),
            height: Val::Percent(0.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.9, 0.9, 1.0, 0.65)),
        ChordStem(f32::from(span) * lane_pct),
    ));
    if !name.is_empty() {
        parent.spawn((
            Text::new(name),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Percent(100.0),
                ..default()
            },
        ));
    }
}

/// The line under the highway naming where the hand sits. Fret 0 is the
/// open string, or the capo on a capoed guitar, and the line says which.
#[derive(Component)]
pub(super) struct FretboardHint {
    pub top_fret: u8,
    pub capo: u8,
}

pub(super) fn update_hint(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    loc: Res<harmonicon_platform::localization::Localization>,
    mut labels: Query<(&FretboardHint, &mut Text)>,
) {
    let now = clock.get();
    let current = targets(&notes.notes, now);
    let next_time = notes
        .notes
        .iter()
        .filter(|n| n.playable && !n.missed && n.time > now && n.time <= now + LOOKAHEAD)
        .map(|n| n.time)
        .min_by(f64::total_cmp);
    let next: Vec<_> = notes
        .notes
        .iter()
        .filter(|n| Some(n.time) == next_time && n.playable && !n.missed)
        .filter_map(|n| if let PlayPosition::Fret(p) = n.position { Some(p) } else { None })
        .collect();
    for (hint, mut text) in &mut labels {
        let next =
            if next.is_empty() { "—".into() } else { anchor(&next, hint.top_fret).to_string() };
        let mut args = vec![("fret", anchor(&current, hint.top_fret).to_string()), ("next", next)];
        let key = if hint.capo == 0 {
            "gameplay-guitar-window"
        } else {
            args.push(("capo", hint.capo.to_string()));
            "gameplay-guitar-window-capo"
        };
        text.0 = loc.msg_args(key, &args).to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::instrument::guitar::StringTechnique;
    fn note(time: f64, string: u8, fret: u8) -> ScheduledNote {
        ScheduledNote {
            time,
            duration: 1.0,
            position: PlayPosition::Fret(FretAssignment {
                string,
                fret,
                technique: StringTechnique::Fretted,
            }),
            ..default()
        }
    }
    #[test]
    fn window_previews_shifts_keeps_holds_and_rewinds() {
        let notes = vec![note(1.0, 6, 0), note(1.0, 5, 2), note(3.0, 2, 12)];
        assert_eq!(targets(&notes, -3.0).len(), 0);
        assert_eq!(anchor(&targets(&notes, 0.0), 22), 2);
        assert_eq!(targets(&notes, 1.5).len(), 2);
        assert_eq!(anchor(&targets(&notes, 2.0), 22), 12);
        assert!(targets(&notes, 4.0).is_empty());
        assert_eq!(anchor(&targets(&notes, 0.0), 22), 2);
        assert_eq!(
            anchor(
                &[FretAssignment { string: 1, fret: 22, technique: StringTechnique::Fretted }],
                22
            ),
            18
        );
    }
    #[test]
    fn missed_and_unplayable_notes_do_not_guide_the_hand() {
        let mut notes = vec![note(1.0, 1, 2), note(2.0, 2, 10), note(3.0, 3, 5)];
        notes[0].missed = true;
        notes[1].playable = false;
        assert_eq!(targets(&notes, 1.0)[0].fret, 5);
    }
    #[test]
    fn chords_have_one_stem_and_name_without_merging_judgments() {
        let mut notes = vec![note(1.0, 6, 0), note(1.0, 5, 2), note(1.0, 4, 2)];
        for note in &mut notes {
            note.chord_pitches = vec![40, 47, 52];
        }
        assert!(chord_span(&notes[0], &notes).is_none());
        assert_eq!(chord_span(&notes[2], &notes), Some((2, "E5".into())));
        assert_eq!(chord_name(&[64, 67, 72]), Some("C/E".into()));
        assert!(chord_name(&[60, 61, 66]).is_none());
    }
    #[test]
    fn fretboard_uses_the_configured_string_count_and_capo() {
        let mut app = App::new();
        let mut guitar =
            Guitar::standard(harmonicon_core::instrument::guitar::GuitarBody::Electric);
        guitar.strings.pop();
        guitar.capo = 2;
        app.world_mut()
            .commands()
            .spawn(Node::default())
            .with_children(|p| spawn_fretboard(p, &guitar, 180.0));
        app.world_mut().flush();
        let world = app.world_mut();
        let cells: Vec<_> = world.query::<&FretCell>().iter(world).collect();
        assert_eq!(cells.len(), 5 * usize::from(HAND_SPAN_FRETS + 2));
        assert!(cells.iter().all(|cell| cell.string <= 5 && cell.top == 20));
    }
}
