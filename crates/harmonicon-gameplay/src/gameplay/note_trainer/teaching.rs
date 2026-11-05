// SPDX-License-Identifier: MIT
//! The trainer's notation and instrument guides, driven by its current drill.
use super::*;
use harmonicon_ui::music_score::{BravuraFont, MusicScoreNotes, NotationNote};

pub(super) fn notation(drill: Option<&Drill>) -> Vec<NotationNote> {
    let Some(drill) = drill else { return Vec::new() };
    pitch_notation(drill.start, drill.target)
}

pub(crate) fn pitch_notation(start: Option<f32>, target: f32) -> Vec<NotationNote> {
    let note = |midi: f32, beat, highlighted| NotationNote {
        start_beat: beat,
        duration_beats: 1.0,
        midi: midi.round() as u8,
        tied_from_previous: false,
        highlighted,
    };
    let mut notes = Vec::new();
    if let Some(start) = start {
        notes.push(note(start, 0.0, false));
    }
    notes.push(note(target, if start.is_some() { 1.0 } else { 0.0 }, true));
    notes
}

pub(crate) fn spawn_staff(
    parent: &mut ChildSpawnerCommands,
    font: &BravuraFont,
    piano: bool,
    loc: &Localization,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(760.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(24.0), Val::Px(8.0)),
            ..default()
        })
        .with_children(|score| {
            score.spawn((
                Text::new(loc.msg("note-trainer-score-help").to_string()),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.78, 0.82, 0.88)),
            ));
            score
                .spawn(Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(if piano { 270.0 } else { 150.0 }),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    padding: UiRect::bottom(Val::Px(30.0)),
                    ..default()
                })
                .with_children(|staff| {
                    if piano {
                        harmonicon_ui::music_score::spawn_piano_score(staff, font);
                    } else {
                        harmonicon_ui::music_score::spawn_music_score(staff, font);
                    }
                });
        });
}

pub(crate) fn sync_score(trainer: Res<NoteTrainer>, mut notes: ResMut<MusicScoreNotes>) {
    let next = notation(trainer.practice.drill());
    if notes.0 != next {
        notes.0 = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn octave_notation_distinguishes_the_start_from_the_target() {
        let instrument = Instrument::Flute(Flute::default());
        let drills = drills(&instrument, TechniqueKind::Register);
        let drill = drills.first().unwrap();
        let notes = notation(Some(drill));
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].midi, drill.start.unwrap() as u8);
        assert!(!notes[0].highlighted);
        assert_eq!(notes[1].midi, drill.target as u8);
        assert!(notes[1].highlighted);
        assert!(notes[1].start_beat > notes[0].start_beat);
    }
}
