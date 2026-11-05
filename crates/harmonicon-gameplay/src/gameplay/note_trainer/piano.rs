// SPDX-License-Identifier: MIT
//! Full keyboard and conservative, explicitly suggested five-finger positions.
use super::*;
use harmonicon_core::instrument::piano::{Piano, is_black};
use harmonicon_ui::dialogs::piano_keys::{KeyboardAxis, KeyboardSpec, key_rect, spawn_piano_keys};

#[derive(Component)]
pub(crate) struct HandRegion(bool);
#[derive(Default)]
pub(crate) struct Hands {
    left: Option<u8>,
    right: Option<u8>,
}

fn spec(piano: &Piano) -> KeyboardSpec {
    KeyboardSpec {
        low: piano.lowest(),
        high: piano.highest(),
        axis: KeyboardAxis::Horizontal,
        label_cs: true,
    }
}

/// Keep a comfortable position until the target lies outside it, then centre
/// five white keys around the target. This suggests a span, never a fingering.
fn span(piano: &Piano, previous: Option<u8>, target: u8) -> (u8, u8) {
    let whites: Vec<u8> = (piano.lowest()..=piano.highest()).filter(|&k| !is_black(k)).collect();
    if let Some(low) = previous
        && let Some(index) = whites.iter().position(|&k| k == low)
    {
        let high = whites[(index + 4).min(whites.len() - 1)];
        if (low..=high).contains(&target) {
            return (low, high);
        }
    }
    let at = whites.partition_point(|&k| k <= target).saturating_sub(1);
    let start = at.saturating_sub(2).min(whites.len().saturating_sub(5));
    (whites[start], whites[(start + 4).min(whites.len() - 1)])
}

pub(super) fn spawn(parent: &mut ChildSpawnerCommands, piano: &Piano, loc: &Localization) {
    parent.spawn((
        Text::new(loc.msg("note-trainer-piano-hands-help").to_string()),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::WHITE),
    ));
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(180.0),
            flex_shrink: 0.0,
            margin: UiRect::top(Val::Px(34.0)),
            ..default()
        })
        .with_children(|board| {
            spawn_piano_keys(board, spec(piano));
            for (right, key) in
                [(false, "note-trainer-left-hand"), (true, "note-trainer-right-hand")]
            {
                board
                    .spawn((
                        HandRegion(right),
                        Node {
                            position_type: PositionType::Absolute,
                            height: Val::Percent(100.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        ZIndex(3),
                        bevy::picking::Pickable::IGNORE,
                        BackgroundColor(Color::NONE),
                        BorderColor::all(Color::WHITE),
                    ))
                    .with_children(|region| {
                        region.spawn((
                            Text::new(loc.msg(key).to_string()),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                            TextColor(Color::WHITE),
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(-28.0),
                                ..default()
                            },
                            bevy::picking::Pickable::IGNORE,
                        ));
                    });
            }
        });
}

pub(crate) fn update_hands(
    trainer: Res<NoteTrainer>,
    mut hands: Local<Hands>,
    mut regions: Query<(&HandRegion, &mut Node, &mut BorderColor, &mut BackgroundColor)>,
) {
    let Some(piano) = trainer.piano() else { return };
    let Some(target) = trainer.practice.drill().map(|d| d.target.round() as u8) else { return };
    let right_active = target >= 60;
    let left = span(
        piano,
        hands.left.or(Some(48)),
        if right_active { hands.left.unwrap_or(48) } else { target },
    );
    let right = span(
        piano,
        hands.right.or(Some(60)),
        if right_active { target } else { hands.right.unwrap_or(60) },
    );
    hands.left = Some(left.0);
    hands.right = Some(right.0);
    for (hand, mut node, mut border, mut fill) in &mut regions {
        let (low, high) = if hand.0 { right } else { left };
        let (x, _, _, _) = key_rect(&spec(piano), low);
        let (end, _, width, _) = key_rect(&spec(piano), high);
        node.left = Val::Percent(x);
        node.width = Val::Percent(end + width - x);
        let active = hand.0 == right_active;
        let color = if hand.0 { Color::srgb(1.0, 0.65, 0.3) } else { Color::srgb(0.35, 0.7, 1.0) };
        *border = BorderColor::all(color.with_alpha(if active { 1.0 } else { 0.5 }));
        fill.0 = color.with_alpha(if active { 0.10 } else { 0.03 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggested_span_contains_each_drill_key_and_stays_inside_the_full_keyboard() {
        let piano = Piano::grand();
        let mut previous = Some(48);
        for target in 48..=84 {
            let (low, high) = span(&piano, previous, target);
            assert!((piano.lowest()..=piano.highest()).contains(&low));
            assert!((low..=high).contains(&target));
            assert_eq!((low..=high).filter(|&k| !is_black(k)).count(), 5);
            previous = Some(low);
        }
        assert_eq!(spec(&piano).low, 21);
        assert_eq!(spec(&piano).high, 108);
    }
}
