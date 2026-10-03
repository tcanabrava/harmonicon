// SPDX-License-Identifier: MIT

//! A live circle-of-fifths "position compass" for Jam Session: shows the
//! equipped harp's own key and which `song::harmonica::Position` its
//! currently-active `Scale` corresponds to (see `dialogs::circle_of_fifths`
//! for the diagram itself). Two uses share this one widget:
//!
//! - **A passive helper**, always spawned by `jam::session::setup`: whatever
//!   position the loaded chart declares stays highlighted for the whole
//!   session, purely as a reference.
//! - **A playable lesson mechanic**, opted into per-lesson via
//!   `LessonManifest::position_cycle`: [`cycle_position`] periodically walks
//!   `JamScale` through `First`/`Second`/`Third` position (the three
//!   `song::chart::Scale` variants with a matching `Position` — blues-
//!   hexatonic pitch classes rooted at the jam key +0/+7/+2 semitones, i.e.
//!   the practical, *stay-on-one-harp* meaning of "switching position"), and
//!   [`on_position_called`] both re-highlights the compass and patches
//!   `JamHoleGuide::scale_classes` so the existing `ScaleAdherence` lesson
//!   criterion (`jam::improv::ImprovStats`) judges played notes against
//!   whichever position was actually called at the time — no new scoring
//!   machinery, just a moving target for the one that already exists.

use bevy::prelude::*;

use harmonicon_app::app::{JamPositionCycle, JamScale, SelectedSong};
use harmonicon_core::chart::Scale;
use harmonicon_core::harmonica::Position;
use harmonicon_gameplay::gameplay::AbsoluteBar;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::{CircleOfFifthsColors, LoadedTheme};
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::circle_of_fifths::spawn_circle_of_fifths;

use super::hole_map::JamHoleGuide;

// ── Pure timing/mapping ─────────────────────────────────────────────────────

/// How many bars the compass holds one called position before advancing —
/// 4 bars per call, 3 calls per 12-bar chorus.
pub(crate) const POSITION_CYCLE_BARS: usize = 4;

/// The positions [`cycle_position`] walks through, in order — the only three
/// `Scale` variants with a matching `Position` (see this module's own doc
/// comment for why the others — `Major`/`MinorPentatonic`/`Country` — don't
/// apply here).
pub(crate) const POSITION_CYCLE: [Scale; 3] =
    [Scale::FirstPosition, Scale::SecondPosition, Scale::ThirdPosition];

/// Which `Scale` should be active at `absolute_bar` (an open-ended, non-
/// wrapped bar count — see `gameplay::AbsoluteBar`), cycling through
/// [`POSITION_CYCLE`] every [`POSITION_CYCLE_BARS`] bars. Pure so the
/// cadence is directly unit-testable.
pub(crate) fn called_scale(absolute_bar: usize) -> Scale {
    POSITION_CYCLE[(absolute_bar / POSITION_CYCLE_BARS) % POSITION_CYCLE.len()]
}

/// The `Position` a `Scale` corresponds to, for the compass's own highlight —
/// `None` for a `Scale` shape that isn't rooted at a circle-of-fifths step at
/// all (`Major`/`MinorPentatonic`/`Country`, always rooted on the harp's own
/// key regardless of position).
pub(crate) fn scale_as_position(scale: Scale) -> Option<Position> {
    match scale {
        Scale::FirstPosition => Some(Position::First),
        Scale::SecondPosition => Some(Position::Second),
        Scale::ThirdPosition => Some(Position::Third),
        Scale::Major | Scale::MinorPentatonic | Scale::Country => None,
    }
}

// ── Live updates ─────────────────────────────────────────────────────────────

/// Fired by [`cycle_position`] when the called position actually changes —
/// not every frame.
#[derive(Message, Clone, Copy)]
pub(crate) struct PositionCalled(pub Scale);

/// Tags the compass's own container with the harp key it was built for, so
/// [`on_position_called`] can rebuild it without re-deriving that key.
#[derive(Component)]
pub(crate) struct PositionCompassSlot(pub String);

/// Walks `JamScale` through [`POSITION_CYCLE`] while `JamPositionCycle` is
/// on (a jam-based lesson that opted in via its manifest's `position_cycle`
/// field) — a no-op for an ordinary Jam Session, where `JamScale` stays
/// whatever it was set to at session start.
pub(crate) fn cycle_position(
    absolute: Res<AbsoluteBar>,
    cycle: Res<JamPositionCycle>,
    mut jam_scale: ResMut<JamScale>,
    mut writer: MessageWriter<PositionCalled>,
) {
    if !cycle.0 {
        return;
    }
    let wanted = called_scale(absolute.0);
    if wanted != jam_scale.0 {
        jam_scale.0 = wanted;
        writer.write(PositionCalled(wanted));
    }
}

/// Reacts to a newly called position: patches `JamHoleGuide::scale_classes`
/// in place (it's only ever computed once at `session::setup` otherwise, so
/// a mid-session `JamScale` change would silently do nothing to live
/// scoring/tint without this) using the same chart-vs-`JamScale` precedence
/// `session::setup` itself uses, then rebuilds the compass diagram to
/// highlight the new position.
pub(crate) fn on_position_called(
    mut events: MessageReader<PositionCalled>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut guide: ResMut<JamHoleGuide>,
    loc: Res<Localization>,
    theme: Res<LoadedTheme>,
    slots: Query<(Entity, &PositionCompassSlot)>,
    mut commands: Commands,
) {
    let Some(&PositionCalled(called)) = events.read().last() else {
        return;
    };
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let key = chart.song.key.as_str();
    let scale = chart.harmonica.scale().unwrap_or(called);
    guide.scale_classes = scale.classes(key);

    let Ok((slot_entity, slot)) = slots.single() else {
        return;
    };
    let position = scale_as_position(scale).unwrap_or(Position::First);
    commands.entity(slot_entity).despawn_related::<Children>();
    commands.entity(slot_entity).with_children(|col| {
        spawn_position_caption(col, &loc, &slot.0, position, theme.circle_of_fifths_colors());
    });
}

// ── UI ────────────────────────────────────────────────────────────────────────

/// The diagram + "Position: Nth" caption, shared by the initial spawn and
/// every rebuild in [`on_position_called`].
fn spawn_position_caption(
    parent: &mut ChildSpawnerCommands,
    loc: &Localization,
    harp_key: &str,
    position: Position,
    colors: CircleOfFifthsColors,
) {
    spawn_circle_of_fifths(
        parent,
        harp_key,
        &[position],
        colors,
        &loc.msg("circle-of-fifths-harp-label"),
    );
    parent.spawn_empty().apply_scene(bsn! {
        Text({String::from(loc.msg_args(
            "jam-position-label",
            &[(
                "position",
                harmonicon_gameplay::gameplay::position_label(position.label(), loc),
            )],
        ))})
        TextFont { font_size: {FontSize::Px(13.0)} }
        TextColor({Color::srgb(0.55, 0.55, 0.60)})
    });
}

/// Spawns the live position compass in Jam Session, as a child of `parent`.
/// A no-op when `harp_key` can't be determined (nothing meaningful to show —
/// same fallback spirit as `song::harmonica::harp_banner`). When `position`
/// is `None` (a hand-authored chart that never declared one), the container
/// still spawns — tagged so a later `position_cycle` lesson can fill it in —
/// just with nothing inside it yet.
pub fn spawn_position_compass(
    parent: &mut ChildSpawnerCommands,
    loc: &Localization,
    harp_key: Option<&str>,
    position: Option<Position>,
    colors: CircleOfFifthsColors,
) {
    let Some(harp_key) = harp_key else {
        return;
    };
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            PositionCompassSlot(harp_key.to_string()),
        ))
        .with_children(|col| {
            if let Some(position) = position {
                spawn_position_caption(col, loc, harp_key, position, colors);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── called_scale ───────────────────────────────────────────────────────────

    #[test]
    fn the_first_cycle_window_calls_first_position() {
        assert_eq!(called_scale(0), Scale::FirstPosition);
        assert_eq!(called_scale(POSITION_CYCLE_BARS - 1), Scale::FirstPosition);
    }

    #[test]
    fn the_second_and_third_windows_call_second_and_third_position() {
        assert_eq!(called_scale(POSITION_CYCLE_BARS), Scale::SecondPosition);
        assert_eq!(called_scale(POSITION_CYCLE_BARS * 2), Scale::ThirdPosition);
    }

    #[test]
    fn the_cycle_wraps_back_to_first_position() {
        assert_eq!(called_scale(POSITION_CYCLE_BARS * 3), Scale::FirstPosition);
        assert_eq!(called_scale(POSITION_CYCLE_BARS * 4), Scale::SecondPosition);
    }

    // ── scale_as_position ─────────────────────────────────────────────────────

    #[test]
    fn the_three_position_scales_map_to_their_matching_position() {
        assert_eq!(scale_as_position(Scale::FirstPosition), Some(Position::First));
        assert_eq!(scale_as_position(Scale::SecondPosition), Some(Position::Second));
        assert_eq!(scale_as_position(Scale::ThirdPosition), Some(Position::Third));
    }

    #[test]
    fn the_non_position_scale_shapes_map_to_nothing() {
        assert_eq!(scale_as_position(Scale::Major), None);
        assert_eq!(scale_as_position(Scale::MinorPentatonic), None);
        assert_eq!(scale_as_position(Scale::Country), None);
    }
}
