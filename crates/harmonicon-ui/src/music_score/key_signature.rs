// SPDX-License-Identifier: MIT
//! A fixed key-signature strip, shared by live scores and the paused full score.
use super::*;

#[derive(Component)]
pub(super) struct KeySignatureSlot { root: Entity, lower: bool, last: Option<(harmonicon_core::notation::KeySignature, Clef)> }

pub(super) fn spawn(parent: &mut ChildSpawnerCommands, root: Entity, lower: bool) {
    parent.spawn((
        KeySignatureSlot { root, lower, last: None },
        Node { position_type: PositionType::Absolute, left: Val::Px(34.0), top: Val::Px(if lower { PANEL_HEIGHT } else { 0.0 }), width: Val::Px(64.0), height: Val::Px(PANEL_HEIGHT), ..default() },
        ZIndex(2), bevy::picking::Pickable::IGNORE,
    ));
}
pub(super) fn rebuild(
    mut commands: Commands,
    font: Option<Res<BravuraFont>>,
    signature: Res<MusicScoreKeySignature>,
    notes: Res<MusicScoreNotes>,
    roots: Query<(Option<&InstrumentScoreClef>, Has<PianoGrandStaff>)>,
    mut slots: Query<(Entity, &mut KeySignatureSlot)>,
) {
    let Some(font) = font else { return };
    for (entity, mut slot) in &mut slots {
        let (forced, grand) = roots.get(slot.root).unwrap_or((None, false));
        let clef = if slot.lower { Clef::Bass } else if grand { Clef::Treble } else { notation::resolve_clef(&notes.0, forced.map(|forced| forced.0)) };
        if slot.last == Some((signature.0, clef)) { continue; }
        slot.last = Some((signature.0, clef));
        commands.entity(entity).despawn_children();
        let flat = signature.0.fifths() < 0;
        let steps = signature_steps(clef, flat);
        commands.entity(entity).with_children(|parent| {
            for (index, &step) in steps.iter().take(usize::from(signature.0.fifths().unsigned_abs())).enumerate() {
                parent.spawn(glyph(&font, if flat { notation::glyph::ACCIDENTAL_FLAT } else { notation::glyph::ACCIDENTAL_SHARP }, index as f32 * 9.0, y_for_step(step), Color::WHITE));
            }
        });
    }
}

/// Conventional accidental order and octave on each clef, as staff steps.
fn signature_steps(clef: Clef, flat: bool) -> [i32; 7] {
    match (clef, flat) {
        (Clef::Alto, false) => [7, 4, 8, 5, 2, 6, 3],
        (Clef::Alto, true) => [3, 6, 2, 5, 1, 4, 0],
        (Clef::Bass | Clef::Bass8vb, false) => [6, 3, 7, 4, 1, 5, 2],
        (Clef::Bass | Clef::Bass8vb, true) => [2, 5, 1, 4, 0, 3, 6],
        (_, false) => [8, 5, 9, 6, 3, 7, 4],
        (_, true) => [4, 7, 3, 6, 2, 5, 1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alto_key_signature_has_conventional_letters_and_octaves() {
        let sharps = [65, 60, 67, 62, 57, 64, 59].map(|midi| notation::staff_step(midi, Clef::Alto));
        let flats = [59, 64, 57, 62, 55, 60, 53].map(|midi| notation::staff_step(midi, Clef::Alto));
        assert_eq!(signature_steps(Clef::Alto, false), sharps);
        assert_eq!(signature_steps(Clef::Alto, true), flats);
    }
}
