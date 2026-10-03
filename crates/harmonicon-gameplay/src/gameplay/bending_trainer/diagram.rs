// SPDX-License-Identifier: MIT

//! The diagram as a target picker you don't need a mouse for, and as a
//! progress map you don't need colour vision for.
//!
//! **Keyboard.** The diagram is one Tab stop (`TabIndex` on its
//! [`OverlayHost`]); once it has focus, the arrow keys move the selected
//! target across the grid as drawn, and Enter/Space does what a click does —
//! which only differs from moving under the Custom scope, where it toggles
//! the cell in or out of the pool (`choose_cell`). This is WAI-ARIA's grid
//! pattern: fifty cells as fifty Tab stops would make everything after the
//! diagram unreachable in practice.
//!
//! Movement follows the *drawing*, not the technique list: Right from hole 6's
//! ½-step draw bend looks for another cell in that same row, and holes 7–10
//! have none there (their bends are blow bends, drawn above the blow row), so
//! the selection stays put rather than jumping to a cell that isn't to the
//! right of it on screen.
//!
//! **Progress without colour.** The tint (`drill::progress_tint`) runs from
//! red to green, which is exactly the pair a red-green colour-blind player
//! can't separate. Each cell therefore also carries a bar along its bottom
//! edge whose *length* is the same accuracy — absent for a target never
//! attempted, so "untried" and "tried and always missed" stay distinct in
//! shape as well as colour. The selected target's record is also spelled out
//! as text in the target card (`layout::progress_text`).

use bevy::input_focus::InputFocus;
use bevy::picking::Pickable;

use super::super::harmonica_overlay::diatonic_row_order;
use super::*;

/// The diagram row `target` is drawn in. The inverse of `row_to_technique`
/// for a given hole: bends and overbends sit on different wings depending on
/// which side of the harp the hole is, so the technique alone isn't enough.
pub(super) fn target_row(target: TrainerTarget) -> Row {
    let draw_side = target.hole <= 6;
    match target.technique {
        Technique::Blow => Row::Blow,
        Technique::Draw => Row::Draw,
        Technique::Bend1 if draw_side => Row::DrawBend(0),
        Technique::Bend2 if draw_side => Row::DrawBend(1),
        Technique::Bend3 if draw_side => Row::DrawBend(2),
        Technique::Bend1 => Row::BlowBend(0),
        Technique::Bend2 => Row::BlowBend(1),
        Technique::Bend3 => Row::BlowBend(2),
        Technique::Over if draw_side => Row::Overblow,
        Technique::Over => Row::Overdraw,
    }
}

/// The cell at (`hole`, `row`), if the diagram draws one there for `harp`.
fn cell_at(harp: &Harmonica, hole: u8, row: Row) -> Option<TrainerTarget> {
    let technique = row_to_technique(row)?;
    let candidate = TrainerTarget { hole, technique };
    (target_note(harp, candidate).is_some() && target_row(candidate) == row).then_some(candidate)
}

/// One arrow-key step from `from`: `dx` along the holes, `dy` down the rows
/// (positive = down the screen). Skips over gaps in the grid to the next
/// drawn cell in that direction, and stays put at an edge — a key press that
/// moves nothing is a clearer answer than one that wraps somewhere
/// unexpected.
pub(super) fn step_target(harp: &Harmonica, from: TrainerTarget, dx: i8, dy: i8) -> TrainerTarget {
    let rows = diatonic_row_order();
    let row = target_row(from);
    if dx != 0 {
        let mut hole = i16::from(from.hole);
        loop {
            hole += i16::from(dx.signum());
            if !(1..=10).contains(&hole) {
                return from;
            }
            if let Some(next) = cell_at(harp, hole as u8, row) {
                return next;
            }
        }
    }
    if dy != 0 {
        let Some(mut index) = rows.iter().position(|r| *r == row).map(|i| i as i16) else {
            return from;
        };
        loop {
            index += i16::from(dy.signum());
            let Some(&next_row) = usize::try_from(index).ok().and_then(|i| rows.get(i)) else {
                return from;
            };
            if let Some(next) = cell_at(harp, from.hole, next_row) {
                return next;
            }
        }
    }
    from
}

/// Arrow keys and Enter/Space on the focused diagram.
pub fn navigate_diagram(
    keyboard: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    hosts: Query<(), With<OverlayHost>>,
    key: Res<TrainerKey>,
    mut target: ResMut<TrainerTarget>,
    mut drill: ResMut<DrillState>,
) {
    let Some(focused) = focus.get() else {
        return;
    };
    if !hosts.contains(focused) {
        return;
    }
    let (mut dx, mut dy) = (0, 0);
    if keyboard.just_pressed(KeyCode::ArrowLeft) {
        dx = -1;
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        dx = 1;
    } else if keyboard.just_pressed(KeyCode::ArrowUp) {
        dy = -1;
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        dy = 1;
    }
    if dx != 0 || dy != 0 {
        let next = step_target(key.harp(), *target, dx, dy);
        if next != *target {
            *target = next;
        }
        return;
    }
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space) {
        let current = *target;
        choose_cell(&mut drill, &mut target, current);
    }
}

/// Marks a diagram cell that already has its progress bar, so
/// [`attach_progress_bars`] adds exactly one — including to the fresh cells
/// `rebuild_overlay` spawns on every key change.
#[derive(Component)]
pub struct HasProgressBar;

/// The bar along a cell's bottom edge whose length is that target's
/// accuracy.
#[derive(Component)]
pub struct CellProgressBar(pub TrainerTarget);

/// Gives every selectable cell its progress bar. The shared diagram
/// spawner (`harmonica_overlay`) knows nothing about drill progress and
/// shouldn't — it also draws the plain, non-trainer diagram — so the trainer
/// decorates its cells after the fact. Scheduled in `PostUpdate` — see the
/// comment at its registration for the despawn race that rules out `Update`.
pub fn attach_progress_bars(
    cells: Query<(Entity, &DiagramCellTarget), Without<HasProgressBar>>,
    mut commands: Commands,
) {
    for (entity, cell) in &cells {
        let Some(technique) = row_to_technique(cell.row) else {
            continue;
        };
        commands.entity(entity).insert(HasProgressBar).with_children(|cell_node| {
            cell_node.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    height: Val::Px(3.0),
                    width: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.92, 0.94, 1.0, 0.85)),
                Visibility::Hidden,
                // A decoration, not a target: without this it would take
                // the click meant for the cell it sits in.
                Pickable::IGNORE,
                CellProgressBar(TrainerTarget { hole: cell.hole, technique }),
            ));
        });
    }
}

/// Sets each bar's length from the same accuracy the tint uses, and hides
/// it for a target never attempted. Runs for every bar when the drill's
/// stats change, and for a freshly spawned bar (a key change rebuilds the
/// diagram) otherwise.
pub fn update_cell_progress_bars(
    drill: Res<DrillState>,
    mut bars: Query<(Ref<CellProgressBar>, &mut Node, &mut Visibility)>,
) {
    let refresh_all = drill.is_changed();
    for (bar, mut node, mut visibility) in &mut bars {
        if !refresh_all && !bar.is_added() {
            continue;
        }
        match drill_accuracy(drill.stats.get(&(bar.0.hole, bar.0.technique))) {
            Some(accuracy) => {
                let width = Val::Percent(accuracy.clamp(0.0, 1.0) * 100.0);
                if node.width != width {
                    node.width = width;
                }
                if *visibility != Visibility::Inherited {
                    *visibility = Visibility::Inherited;
                }
            }
            None if *visibility != Visibility::Hidden => *visibility = Visibility::Hidden,
            None => {}
        }
    }
}
