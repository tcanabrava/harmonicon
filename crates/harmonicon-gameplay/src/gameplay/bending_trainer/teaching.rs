// SPDX-License-Identifier: MIT
//! A focused reading of the selected hole: breath, natural pitch and target.
use super::*;
use harmonicon_ui::music_score::{
    BravuraFont, MusicScoreKeySignature, MusicScoreNotes, NotationNote, project_score,
};

#[derive(Component)]
pub(crate) struct ScoreSlot;

pub(super) fn breath(target: TrainerTarget) -> &'static str {
    match target.technique {
        Technique::Blow => "bending-breath-out",
        Technique::Draw => "bending-breath-in",
        Technique::Bend1 | Technique::Bend2 | Technique::Bend3 if target.hole <= 6 => {
            "bending-breath-in"
        }
        Technique::Bend1 | Technique::Bend2 | Technique::Bend3 => "bending-breath-out",
        Technique::Over if target.hole <= 6 => "bending-breath-out",
        Technique::Over => "bending-breath-in",
    }
}

fn notation(harp: &Harmonica, target: TrainerTarget) -> Vec<NotationNote> {
    let Some(pitch) = target_note(harp, target).and_then(|n| note_to_midi(&n)) else {
        return Vec::new();
    };
    let start = natural_note_for_target(harp, target)
        .and_then(|n| note_to_midi(&n))
        .filter(|&natural| natural != pitch)
        .map(|n| n as f32);
    super::super::note_trainer::teaching::pitch_notation(start, pitch as f32)
}

pub(crate) fn sync_teaching_score(
    mut commands: Commands,
    key: Res<TrainerKey>,
    target: Res<TrainerTarget>,
    loc: Res<Localization>,
    font: Option<Res<BravuraFont>>,
    slots: Query<(Entity, Option<&Children>), With<ScoreSlot>>,
    mut notes: ResMut<MusicScoreNotes>,
    signature: Option<ResMut<MusicScoreKeySignature>>,
) {
    let mut next = notation(key.harp(), *target);
    let written_key = project_score(&mut next, Default::default(), "C");
    if let Some(mut signature) = signature {
        signature.set_if_neq(written_key);
    }
    if notes.0 != next {
        notes.0 = next;
    }
    if let Some(font) = font {
        for (slot, children) in &slots {
            if children.is_none() {
                commands.entity(slot).with_children(|p| {
                    super::super::note_trainer::teaching::spawn_staff(p, &font, false, None, &loc);
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn breath_changes_for_high_blow_bends_and_overdraws() {
        assert_eq!(
            breath(TrainerTarget { hole: 2, technique: Technique::Bend1 }),
            "bending-breath-in"
        );
        assert_eq!(
            breath(TrainerTarget { hole: 8, technique: Technique::Bend1 }),
            "bending-breath-out"
        );
        assert_eq!(
            breath(TrainerTarget { hole: 6, technique: Technique::Over }),
            "bending-breath-out"
        );
        assert_eq!(
            breath(TrainerTarget { hole: 7, technique: Technique::Over }),
            "bending-breath-in"
        );
    }
    #[test]
    fn notation_resolves_actual_pitches_and_changes_with_the_harp_key() {
        let target = TrainerTarget { hole: 2, technique: Technique::Bend1 };
        let c = notation(&richter_harp("C"), target);
        let d = notation(&richter_harp("D"), target);
        assert_eq!(c.len(), 2);
        assert!(!c[0].highlighted);
        assert!(c[1].highlighted);
        assert!(c[0].midi > c[1].midi, "a bend lowers the natural reed pitch");
        assert_eq!(d[1].midi, c[1].midi + 2);
        assert_eq!(
            notation(&richter_harp("C"), TrainerTarget { hole: 4, technique: Technique::Blow })
                .len(),
            1
        );
        assert!(
            notation(&richter_harp("C"), TrainerTarget { hole: 5, technique: Technique::Bend1 })
                .is_empty()
        );
    }
}
