// SPDX-License-Identifier: MIT

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
    assert_eq!(pos, Vec2::new(100.0, 50.0) + NOTE_LABEL_OFFSET);
}

#[test]
fn note_label_position_is_unchanged_at_default_ui_scale() {
    let pos = note_label_position(Vec2::new(300.0, 150.0), 1.0);
    assert_eq!(pos, Vec2::new(300.0, 150.0) + NOTE_LABEL_OFFSET);
}

#[test]
fn note_label_position_offsets_up_and_left() {
    let pos = note_label_position(Vec2::ZERO, 1.0);
    assert_eq!(pos, NOTE_LABEL_OFFSET);
    assert!(pos.x < 0.0, "should sit left of the note");
    assert!(pos.y < 0.0, "should sit above the note");
}

#[test]
fn note_depth_scales_with_duration() {
    // Half a lookahead of duration is proportional, inside the clamp band.
    assert!((note_depth(0.3) - 6.0).abs() < 1e-4);
}

#[test]
fn note_depth_is_clamped() {
    assert_eq!(note_depth(0.0), 0.4); // tiny notes keep a visible minimum
    assert_eq!(note_depth(100.0), 12.0); // long notes are capped
}

// ── note_tint_3d ───────────────────────────────────────────────────────────

#[test]
fn note_tint_3d_is_gold_when_hit() {
    let (base, _, _) = note_tint_3d(true, false, true);
    assert_eq!(base, Color::srgb(1.0, 0.9, 0.3));
}

#[test]
fn note_tint_3d_is_dark_red_when_missed() {
    let (base, _, _) = note_tint_3d(false, true, true);
    assert_eq!(base, Color::srgb(0.4, 0.12, 0.12));
}

#[test]
fn note_tint_3d_restores_the_base_blow_draw_appearance_once_neither() {
    let (base, emissive, tail_color) = note_tint_3d(false, false, true);
    let (r, g, b, emit_r, emit_g, emit_b) = note_base_appearance(true);
    assert_eq!(base, Color::srgb(r, g, b));
    assert_eq!(emissive, LinearRgba::new(emit_r, emit_g, emit_b, 1.0));
    assert_eq!(tail_color, Color::srgba(r, g, b, 0.9).to_linear());
}

#[test]
fn leaving_3d_restores_the_2d_camera() {
    // While in 3D the shared Camera2d is pushed behind (order 1) and stops
    // clearing; restore_camera must return it to the menu's defaults.
    let mut world = World::new();
    let cam = world
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
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
