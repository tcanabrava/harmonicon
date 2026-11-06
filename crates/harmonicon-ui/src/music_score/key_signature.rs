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
        let bass = matches!(clef, Clef::Bass | Clef::Bass8vb);
        let steps = match (bass, flat) {
            (false, false) => [8,5,9,6,3,7,4], (false, true) => [4,7,3,6,2,5,1],
            (true, false) => [6,3,7,4,1,5,2], (true, true) => [2,5,1,4,0,3,6],
        };
        commands.entity(entity).with_children(|parent| {
            for (index, &step) in steps.iter().take(usize::from(signature.0.fifths().unsigned_abs())).enumerate() {
                parent.spawn(glyph(&font, if flat { notation::glyph::ACCIDENTAL_FLAT } else { notation::glyph::ACCIDENTAL_SHARP }, index as f32 * 9.0, y_for_step(step), Color::WHITE));
            }
        });
    }
}
