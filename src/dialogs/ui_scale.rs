use bevy::prelude::*;

/// UI scale never goes below the natural size, and caps out well before it
/// gets impractical.
const MIN_SCALE: f32 = 1.0;
const MAX_SCALE: f32 = 8.0;
const SCALE_STEP: f32 = 1.2;

/// Arrow-key UI scaling. Snaps `UiScale` straight to the new value rather
/// than tweening toward it: Bevy's font atlas caches a rasterized glyph per
/// *exact* effective size (font_size × scale), so smoothly animating the
/// scale over many frames requested a fresh atlas texture on almost every
/// frame of the transition — with a song's HUD on screen (a dozen-plus
/// distinct font sizes), that was enough GPU texture churn during active
/// gameplay to exhaust GPU memory and crash. One request per key press
/// instead of dozens avoids that entirely.
pub fn change_scaling(input: Res<ButtonInput<KeyCode>>, mut ui_scale: ResMut<UiScale>) {
    if input.just_pressed(KeyCode::ArrowUp) {
        ui_scale.0 = (ui_scale.0 * SCALE_STEP).min(MAX_SCALE);
        info!("Scaling up! Scale: {}", ui_scale.0);
    }
    if input.just_pressed(KeyCode::ArrowDown) {
        ui_scale.0 = (ui_scale.0 / SCALE_STEP).max(MIN_SCALE);
        info!("Scaling down! Scale: {}", ui_scale.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_scale(scale: f32) -> App {
        let mut app = App::new();
        app.insert_resource(UiScale(scale))
            .insert_resource(ButtonInput::<KeyCode>::default())
            .add_systems(Update, change_scaling);
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
    }

    #[test]
    fn arrow_up_scales_up_by_the_step() {
        let mut app = app_with_scale(1.0);
        press(&mut app, KeyCode::ArrowUp);
        assert!((app.world().resource::<UiScale>().0 - SCALE_STEP).abs() < 1e-6);
    }

    #[test]
    fn arrow_down_scales_down_by_the_step() {
        let mut app = app_with_scale(2.4);
        press(&mut app, KeyCode::ArrowDown);
        assert!((app.world().resource::<UiScale>().0 - 2.0).abs() < 1e-6);
    }

    #[test]
    fn scale_is_clamped_to_the_min_and_max() {
        let mut app = app_with_scale(1.0);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<UiScale>().0, MIN_SCALE);

        let mut app = app_with_scale(MAX_SCALE);
        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().resource::<UiScale>().0, MAX_SCALE);
    }

    #[test]
    fn scale_change_is_immediate_not_animated() {
        // The whole point of the fix: one key press produces the final value
        // in the same frame, not a multi-frame tween.
        let mut app = app_with_scale(1.0);
        press(&mut app, KeyCode::ArrowUp);
        let after_one_frame = app.world().resource::<UiScale>().0;
        // `just_pressed` only gets reset by `ButtonInput::clear`, which a real
        // app's input systems call once per frame — a bare test App doesn't
        // run those, so without this the key would read as freshly-pressed
        // forever and the second `update()` would double-apply the scaling.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        let after_two_frames = app.world().resource::<UiScale>().0;
        assert_eq!(
            after_one_frame, after_two_frames,
            "no further drift once the key press is handled"
        );
    }
}
