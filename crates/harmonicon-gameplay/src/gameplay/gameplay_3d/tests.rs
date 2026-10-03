// SPDX-License-Identifier: MIT

use harmonicon_core::chart::Modifier;

use super::*;

#[test]
fn lanes_are_centered_and_ordered() {
    // The ten lanes straddle x = 0 symmetrically.
    assert!((lane_x(1, 10) + lane_x(10, 10)).abs() < 1e-6);
    // ...and march left-to-right with hole number.
    assert!(lane_x(2, 10) > lane_x(1, 10));
    assert!(lane_x(10, 10) > lane_x(1, 10));
}

#[test]
fn lanes_recenter_for_a_different_hole_count() {
    // A 12-hole chromatic layout must still straddle x = 0 symmetrically.
    assert!((lane_x(1, 12) + lane_x(12, 12)).abs() < 1e-6);
}

#[test]
fn lane_spacing_is_one_lane_width() {
    assert!((lane_x(2, 10) - lane_x(1, 10) - LANE_WIDTH).abs() < 1e-6);
}

// ── note_label_position ───────────────────────────────────────────────────

#[test]
fn note_label_position_cancels_out_ui_scale() {
    // At 2x UI zoom, bevy_ui will double whatever Val::Px we emit when it
    // converts to physical pixels — so we must emit half the viewport
    // coordinate up front for the two to cancel out to the right place.
    let pos = note_label_position(Vec2::new(200.0, 100.0), 2.0);
    assert_eq!(pos, Vec2::new(100.0, 50.0));
}

#[test]
fn note_label_position_is_unchanged_at_default_ui_scale() {
    let pos = note_label_position(Vec2::new(300.0, 150.0), 1.0);
    assert_eq!(pos, Vec2::new(300.0, 150.0));
}

#[test]
fn a_label_anchors_just_past_the_ribbons_side() {
    let right = label_anchor_offset(true);
    let left = label_anchor_offset(false);
    assert!(right.x > LANE_WIDTH * NOTE_W * 0.5);
    assert_eq!(left.x, -right.x);
    assert_eq!((right.y, right.z), (0.0, 0.0));
}

#[test]
fn note_depth_scales_with_duration() {
    // Half a lookahead of duration is half the lane, inside the clamp band.
    assert!((note_depth(LOOKAHEAD / 2.0) - LANE_DEPTH / 2.0).abs() < 1e-4);
}

#[test]
fn note_depth_is_clamped() {
    assert_eq!(note_depth(0.0), 0.4); // tiny notes keep room for their cap
    assert_eq!(note_depth(100.0), LANE_DEPTH * 1.5); // the rest is off screen
}

// ── note ribbons ───────────────────────────────────────────────────────────

#[test]
fn a_ribbon_leaves_a_gap_behind_it_but_always_fits_its_cap() {
    let (_, long) = note_dimensions(LOOKAHEAD / 2.0);
    assert!((long - (LANE_DEPTH / 2.0 - RIBBON_END_GAP)).abs() < 1e-4);
    let (_, tiny) = note_dimensions(0.0);
    assert!(tiny > RIBBON_CAP);
}

#[test]
fn a_ribbon_clears_every_surface_it_crosses() {
    // Equal heights z-fight: the ribbon flickered over the hole pads when
    // it sat exactly on their top face.
    let pad_top = LANE_Y + PAD_H;
    let hit_line_top = HIT_LINE_Y + HIT_LINE_H * 0.5;
    assert!(RIBBON_Y - pad_top.max(hit_line_top) >= 0.05);
}

#[test]
fn a_wobble_crosses_the_hit_line_at_the_charted_rate() {
    // Crests `1 / cycles` units apart, scrolling at `lane_speed`, pass the
    // hit line `cycles * lane_speed` times a second.
    let vibrato = [Modifier::Vibrato { oscillation_hz: 5.0, intensity: None }];
    let t = ribbon_technique_3d(&vibrato);
    assert!((t.y * lane_speed() - 5.0).abs() < 1e-4);
}

#[test]
fn a_missed_ribbon_dims_red_and_a_hit_keeps_its_colour_for_the_hold_state() {
    let colors = NoteColors::default();
    let base = ribbon_color(false, true, colors);
    assert_eq!(base, {
        let c = colors.blow.to_srgba();
        Color::srgba(c.red, c.green, c.blue, 0.95).to_linear()
    });
    assert_ne!(ribbon_color(true, true, colors), base);
    assert_ne!(ribbon_color(false, false, colors), base);
}

#[test]
fn leaving_3d_restores_the_2d_camera() {
    // While in 3D the shared Camera2d is pushed behind (order 1) and stops
    // clearing; restore_camera must return it to the menu's defaults.
    let mut world = World::new();
    let cam = world
        .spawn((
            Camera2d,
            Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
            Transform::default(),
        ))
        .id();

    let mut schedule = Schedule::default();
    schedule.add_systems(restore_camera);
    schedule.run(&mut world);

    let camera = world.get::<Camera>(cam).unwrap();
    assert_eq!(camera.order, 0);
    assert!(matches!(camera.clear_color, ClearColorConfig::Default));
}

#[test]
fn expired_note_cannot_respawn_until_rewind() {
    let mut note = super::super::tests::overlap_test_note(0.0);
    note.duration = 0.1;
    assert!(note_has_left_view(&note, 1.0));
    assert!(note_has_left_view(&note, 2.0));
    assert!(!note_has_left_view(&note, 0.0));
}
