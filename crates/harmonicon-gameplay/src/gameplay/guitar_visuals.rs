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

/// The Note Trainer's compact movable neck window. Column zero shows the
/// open string; other columns follow the hand. Play uses the full overview.
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
    mut diagrams: Query<(&ChordDiagram, &mut Text, &mut Node), Without<FretCell>>,
) {
    let targets = targets(&notes.notes, clock.get());
    for (diagram, mut text, mut node) in &mut diagrams {
        let first = targets.iter().filter(|p| p.fret > 0).map(|p| p.fret).min().unwrap_or(1);
        if diagram.0 == 255 {
            text.0 = group_name(&attack_group(&notes.notes, clock.get(), None));
            continue;
        }
        if diagram.0 == 0 {
            text.0 = if targets.is_empty() { String::new() } else { first.to_string() };
            continue;
        }
        let target = targets.iter().find(|p| p.string == diagram.0);
        text.0 = if targets.is_empty() {
            String::new()
        } else {
            match target {
                Some(p) if p.fret == 0 => "○",
                Some(_) => "●",
                None => "×",
            }
            .into()
        };
        let fret = target.map_or(0, |p| p.fret);
        node.top = Val::Px(if fret == 0 { 0.0 } else { 24.0 + f32::from(fret - first) * 24.0 });
    }
    for (cell, mut text, mut color) in &mut cells {
        paint_cell(cell, &targets, &mut text, &mut color);
    }
}

/// One fretboard cell, for a hand window placed over `targets`: the fret it
/// shows, lit in its string's colour where a target sits, the rest of the
/// window faintly tinted in the Note Trainer.
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
    fn overview_keeps_current_and_next_attacks_distinct_before_the_first_note() {
        let notes = vec![note(1.0, 6, 0), note(1.0, 5, 2), note(2.0, 2, 12)];
        let current = attack_group(&notes, 0.0, None);
        let next = attack_group(&notes, 0.0, current.first().map(|n| n.time));
        assert_eq!(current.len(), 2);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].time, 2.0);
        assert!(attack_group(&notes, 4.0, None).is_empty());
        assert_eq!(attack_group(&notes, 0.0, None).len(), 2, "seek resets guidance");
        assert_eq!(
            hand_region(
                &[FretAssignment { string: 6, fret: 0, technique: StringTechnique::Fretted }],
                22
            ),
            None,
            "open strings have no hand region"
        );
        assert_eq!(
            hand_region(
                &[FretAssignment { string: 1, fret: 22, technique: StringTechnique::Fretted }],
                22
            ),
            Some((18, 22))
        );
    }

    #[test]
    fn overview_updates_without_a_highway_fret_grid() {
        let mut app = App::new();
        app.init_resource::<GameplayClock>()
            .init_resource::<SongNotes>()
            .init_resource::<harmonicon_platform::localization::Localization>()
            .add_systems(Update, (update_fretboard, update_neck_overview));
        let mut guitar =
            Guitar::standard(harmonicon_core::instrument::guitar::GuitarBody::Electric);
        guitar.capo = 2;
        guitar.strings.pop();
        let loc = harmonicon_platform::localization::Localization::default();
        app.world_mut().commands().spawn(Node::default()).with_children(|p| {
            spawn_neck_overview(p, &guitar, &loc);
            spawn_chord_diagram(p, &guitar);
        });
        app.world_mut().flush();
        let mut notes = app.world_mut().resource_mut::<SongNotes>();
        notes.notes = vec![note(1.0, 1, 3), note(2.0, 5, 8)];
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&FretCell>().iter(world).count(), 0);
        let visible: Vec<_> = world
            .query::<(&NeckDot, &Node)>()
            .iter(world)
            .filter(|(_, node)| node.display != Display::None)
            .collect();
        assert_eq!(visible.len(), 2, "one current dot and one next ring");
        let current = visible.iter().find(|(dot, _)| !dot.next).unwrap();
        assert_eq!(
            current.1.left,
            Val::Percent(5.5 * 100.0 / (f32::from(guitar.frets) + 1.0)),
            "relative fret 3 is physical fret 5 with capo 2"
        );
        assert!(visible.iter().all(|(dot, _)| dot.string <= 5));
        let labels: Vec<_> = world.query::<(&ChordDiagram, &Text)>().iter(world).collect();
        assert!(
            labels.iter().any(|(marker, text)| marker.0 == 1 && text.0 == "●"),
            "right-panel diagram updates without FretCell entities"
        );
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

/// Printed-chart convention: bass strings at left, nut at top. Dots are
/// positions from the chart, never guessed finger numbers.
#[derive(Component)]
pub(super) struct ChordDiagram(u8);

pub(super) fn spawn_chord_diagram(parent: &mut ChildSpawnerCommands, guitar: &Guitar) {
    parent
        .spawn((
            Node {
                width: Val::Px(150.0),
                height: Val::Px(152.0),
                flex_shrink: 0.0,
                margin: UiRect { top: Val::Px(28.0), ..UiRect::horizontal(Val::Px(12.0)) },
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.10, 0.14)),
        ))
        .with_children(|chart| {
            chart.spawn((
                ChordDiagram(255),
                Text::new(""),
                TextFont { font_size: FontSize::Px(20.0), ..default() },
                TextColor(Color::WHITE),
                Node { position_type: PositionType::Absolute, top: Val::Px(-28.0), ..default() },
            ));
            chart.spawn((
                ChordDiagram(0),
                Text::new(""),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::WHITE),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-10.0),
                    top: Val::Px(28.0),
                    ..default()
                },
            ));
            let count = guitar.string_count();
            for fret in 0..=5 {
                chart.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(10.0),
                        right: Val::Px(10.0),
                        top: Val::Px(24.0 + fret as f32 * 24.0),
                        height: Val::Px(if fret == 0 { 3.0 } else { 1.0 }),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.55, 0.60, 0.67)),
                ));
            }
            for string in 1..=count {
                let x = 10.0
                    + f32::from(count - string) * 130.0 / f32::from(count.saturating_sub(1).max(1));
                chart.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(24.0),
                        width: Val::Px(1.0),
                        height: Val::Px(120.0),
                        ..default()
                    },
                    BackgroundColor(string_color(string).with_alpha(0.7)),
                ));
                chart.spawn((
                    ChordDiagram(string),
                    Text::new(""),
                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                    TextColor(Color::srgb(0.98, 0.85, 0.47)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x - 8.0),
                        top: Val::Px(0.0),
                        ..default()
                    },
                ));
            }
        });
}

/// The first eligible attack, or the attack strictly after a given one.
/// Holds remain eligible after being heard, and seeks need no cached cursor.
fn attack_group(notes: &[ScheduledNote], now: f64, after: Option<f64>) -> Vec<&ScheduledNote> {
    let eligible = |n: &&ScheduledNote| {
        matches!(n.position, PlayPosition::Fret(_))
            && n.playable
            && !n.missed
            && n.time + n.duration > now
            && n.time <= now + LOOKAHEAD
            && after.is_none_or(|previous| n.time > previous)
    };
    let Some(first) = notes.iter().filter(eligible).min_by(|a, b| a.time.total_cmp(&b.time)) else {
        return Vec::new();
    };
    notes.iter().filter(eligible).filter(|n| n.time == first.time).collect()
}

fn group_name(group: &[&ScheduledNote]) -> String {
    let pitches = group
        .iter()
        .find(|n| n.chord_pitches.len() > 1)
        .map(|n| n.chord_pitches.clone())
        .unwrap_or_else(|| group.iter().filter_map(|n| n.expected_pitch).collect());
    chord_name(&pitches).unwrap_or_default()
}

fn hand_region(positions: &[FretAssignment], top: u8) -> Option<(u8, u8)> {
    let highest = positions.iter().filter(|p| p.fret > 0).map(|p| p.fret).max()?;
    let first = anchor(positions, top);
    Some((first, (first + HAND_SPAN_FRETS).max(highest).min(top)))
}

#[derive(Component)]
pub(super) struct NeckOverview(Guitar);
#[derive(Component)]
pub(super) struct NeckRegion(bool);
#[derive(Component)]
pub(super) struct NeckDot {
    next: bool,
    string: u8,
}
#[derive(Component)]
pub(super) struct NeckLabel(bool);
#[derive(Component)]
pub(super) struct NeckDash;

const NECK_NOW: Color = Color::srgb(0.98, 0.82, 0.35);
const NECK_NEXT: Color = Color::srgb(0.48, 0.77, 1.0);

/// A schematic full neck: physical frets left to right, string 1 on top.
/// Two outlined windows guide the hand; dots alone say which strings to play.
pub(super) fn spawn_neck_overview(
    parent: &mut ChildSpawnerCommands,
    guitar: &Guitar,
    loc: &harmonicon_platform::localization::Localization,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(198.0),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|panel| {
            let caption = if guitar.capo == 0 {
                loc.msg("gameplay-guitar-neck").to_string()
            } else {
                loc.msg_args("gameplay-guitar-neck-capo", &[("capo", guitar.capo.to_string())])
                    .to_string()
            };
            panel.spawn((
                Text::new(caption),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.7, 0.75, 0.82)),
            ));
            panel
                .spawn((
                    NeckOverview(guitar.clone()),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(92.0),
                        margin: UiRect { top: Val::Px(25.0), bottom: Val::Px(46.0), ..default() },
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.09, 0.08)),
                ))
                .with_children(|board| {
                    let columns = f32::from(guitar.frets) + 1.0;
                    for fret in 0..=guitar.frets {
                        let x = f32::from(fret) * 100.0 / columns;
                        board.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(x),
                                width: Val::Px(if fret == 1 || fret == guitar.capo + 1 {
                                    3.0
                                } else {
                                    1.0
                                }),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.45, 0.44, 0.40)),
                        ));
                        if fret == 0 || matches!(fret, 3 | 5 | 7 | 9 | 12 | 15 | 17 | 19 | 21 | 24)
                        {
                            board.spawn((
                                Text::new(fret.to_string()),
                                TextFont { font_size: FontSize::Px(11.0), ..default() },
                                TextColor(Color::srgb(0.67, 0.7, 0.75)),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Percent((f32::from(fret) + 0.25) * 100.0 / columns),
                                    bottom: Val::Px(-16.0),
                                    ..default()
                                },
                            ));
                        }
                    }
                    for string in 1..=guitar.string_count() {
                        let y =
                            (f32::from(string) - 0.5) * 100.0 / f32::from(guitar.string_count());
                        board.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Percent(y),
                                width: Val::Percent(100.0),
                                height: Val::Px(1.0),
                                ..default()
                            },
                            BackgroundColor(string_color(string).with_alpha(0.5)),
                        ));
                    }
                    for next in [false, true] {
                        let color = if next { NECK_NEXT } else { NECK_NOW };
                        board
                            .spawn((
                                NeckRegion(next),
                                Node {
                                    position_type: PositionType::Absolute,
                                    height: Val::Percent(100.0),
                                    border: UiRect::all(Val::Px(if next { 0.0 } else { 2.0 })),
                                    ..default()
                                },
                                BorderColor::all(color),
                                BackgroundColor(color.with_alpha(if next { 0.05 } else { 0.14 })),
                            ))
                            .with_children(|region| {
                                if next {
                                    // Segmented edges remain distinguishable without colour.
                                    for segment in 0..8 {
                                        for bottom in [false, true] {
                                            region.spawn((
                                                NeckDash,
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    left: Val::Percent(segment as f32 * 12.5),
                                                    top: if bottom {
                                                        Val::Auto
                                                    } else {
                                                        Val::Px(0.0)
                                                    },
                                                    bottom: if bottom {
                                                        Val::Px(0.0)
                                                    } else {
                                                        Val::Auto
                                                    },
                                                    width: Val::Percent(7.0),
                                                    height: Val::Px(2.0),
                                                    ..default()
                                                },
                                                BackgroundColor(color),
                                            ));
                                        }
                                    }
                                    for segment in 0..5 {
                                        for right in [false, true] {
                                            region.spawn((
                                                NeckDash,
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    left: if right {
                                                        Val::Auto
                                                    } else {
                                                        Val::Px(0.0)
                                                    },
                                                    right: if right {
                                                        Val::Px(0.0)
                                                    } else {
                                                        Val::Auto
                                                    },
                                                    top: Val::Percent(segment as f32 * 20.0),
                                                    width: Val::Px(2.0),
                                                    height: Val::Percent(12.0),
                                                    ..default()
                                                },
                                                BackgroundColor(color),
                                            ));
                                        }
                                    }
                                }
                                region.spawn((
                                    NeckLabel(next),
                                    Text::new(""),
                                    TextLayout::no_wrap(),
                                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                                    TextColor(color),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(if next { 112.0 } else { -22.0 }),
                                        ..default()
                                    },
                                ));
                            });
                        // Next rings are drawn first, with solid current dots on top.
                    }
                    for next in [true, false] {
                        for string in 1..=guitar.string_count() {
                            board.spawn((
                                NeckDot { next, string },
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: Val::Px(if next { 18.0 } else { 10.0 }),
                                    height: Val::Px(if next { 18.0 } else { 10.0 }),
                                    border_radius: BorderRadius::MAX,
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                UiTransform::from_translation(Val2::all(Val::Percent(-50.0))),
                                BorderColor::all(if next { NECK_NEXT } else { NECK_NOW }),
                                BackgroundColor(if next { Color::NONE } else { NECK_NOW }),
                            ));
                        }
                    }
                });
        });
}

pub(super) fn update_neck_overview(
    clock: Res<GameplayClock>,
    notes: Res<SongNotes>,
    loc: Res<harmonicon_platform::localization::Localization>,
    boards: Query<&NeckOverview>,
    mut regions: Query<
        (&NeckRegion, &mut Node, &mut BackgroundColor, &mut BorderColor),
        (Without<NeckDot>, Without<NeckLabel>),
    >,
    mut dots: Query<(&NeckDot, &mut Node), (Without<NeckRegion>, Without<NeckLabel>)>,
    mut labels: Query<(&NeckLabel, &mut Text, &mut Node), (Without<NeckRegion>, Without<NeckDot>)>,
    mut dashes: Query<&mut BackgroundColor, (With<NeckDash>, Without<NeckRegion>)>,
) {
    let Ok(NeckOverview(guitar)) = boards.single() else { return };
    let current = attack_group(&notes.notes, clock.get(), None);
    let next = attack_group(&notes.notes, clock.get(), current.first().map(|n| n.time));
    let positions = |group: &[&ScheduledNote]| {
        group
            .iter()
            .filter_map(|n| if let PlayPosition::Fret(p) = n.position { Some(p) } else { None })
            .collect::<Vec<_>>()
    };
    let now_positions = positions(&current);
    let next_positions = positions(&next);
    let columns = f32::from(guitar.frets) + 1.0;
    let now_span = hand_region(&now_positions, guitar.top_fret());
    let next_span = hand_region(&next_positions, guitar.top_fret());
    let shared = now_span.is_some() && now_span == next_span;
    for mut dash in &mut dashes {
        dash.0 = if shared || next_span.is_none() { Color::NONE } else { NECK_NEXT };
    }
    for (region, mut node, mut fill, mut border) in &mut regions {
        let group = if region.0 { &next_positions } else { &now_positions };
        let span = hand_region(group, guitar.top_fret());
        let color = if region.0 { NECK_NEXT } else { NECK_NOW };
        fill.0 = if span.is_none() || (region.0 && shared) {
            Color::NONE
        } else {
            color.with_alpha(if region.0 { 0.05 } else { 0.14 })
        };
        *border = BorderColor::all(if span.is_none() { Color::NONE } else { color });
        if let Some((first, last)) = span {
            node.display = Display::Flex;
            node.left = Val::Percent(f32::from(first + guitar.capo) * 100.0 / columns);
            node.width = Val::Percent(f32::from(last - first + 1) * 100.0 / columns);
        } else {
            // All-open groups still need their action label, at the nut/capo.
            node.display = if group.is_empty() { Display::None } else { Display::Flex };
            node.left = Val::Percent(f32::from(guitar.capo) * 100.0 / columns);
            node.width = Val::Percent(100.0 / columns);
        }
    }
    for (dot, mut node) in &mut dots {
        let group = if dot.next { &next_positions } else { &now_positions };
        if let Some(position) = group.iter().find(|p| p.string == dot.string) {
            node.display = Display::Flex;
            node.left =
                Val::Percent((f32::from(position.fret + guitar.capo) + 0.5) * 100.0 / columns);
            node.top = Val::Percent(
                (f32::from(dot.string) - 0.5) * 100.0 / f32::from(guitar.string_count()),
            );
        } else {
            node.display = Display::None;
        }
    }
    for (label, mut text, mut node) in &mut labels {
        let span = if label.0 { next_span } else { now_span };
        let near_end =
            span.is_some_and(|(_, last)| last + guitar.capo >= guitar.frets.saturating_sub(3));
        node.left = if near_end { Val::Auto } else { Val::Px(0.0) };
        node.right = if near_end { Val::Px(0.0) } else { Val::Auto };
        let name = group_name(if label.0 { &next } else { &current });
        let key = if label.0 { "gameplay-guitar-neck-next" } else { "gameplay-guitar-neck-now" };
        text.0 = loc.msg_args(key, &[("chord", name)]).to_string();
    }
}
