use bevy::prelude::*;
use core::time::Duration;

#[derive(Resource)]
pub struct TargetScale {
    pub start_scale: f32,
    pub target_scale: f32,
    pub target_time: Timer,
}

impl TargetScale {
    pub fn set_scale(&mut self, scale: f32) {
        self.start_scale = self.current_scale();
        self.target_scale = scale;
        self.target_time.reset();
    }

    pub fn current_scale(&self) -> f32 {
        let completion = self.target_time.fraction();
        let t = ease_in_expo(completion);
        self.start_scale.lerp(self.target_scale, t)
    }

    pub fn tick(&mut self, delta: Duration) -> &Self {
        self.target_time.tick(delta);
        self
    }

    pub fn already_completed(&self) -> bool {
        self.target_time.is_finished() && !self.target_time.just_finished()
    }
}

pub fn apply_scaling(
    time: Res<Time>,
    mut target_scale: ResMut<TargetScale>,
    mut ui_scale: ResMut<UiScale>,
) {
    if target_scale.tick(time.delta()).already_completed() {
        return;
    }

    ui_scale.0 = target_scale.current_scale();
}

pub fn ease_in_expo(x: f32) -> f32 {
    if x == 0. {
        0.
    } else {
        ops::powf(2.0f32, 5. * x - 5.)
    }
}

pub fn change_scaling(input: Res<ButtonInput<KeyCode>>, mut ui_scale: ResMut<TargetScale>) {
    if input.just_pressed(KeyCode::ArrowUp) {
        let scale = (ui_scale.target_scale * 1.2).min(8.);
        ui_scale.set_scale(scale);
        info!("Scaling up! Scale: {}", ui_scale.target_scale);
    }
    if input.just_pressed(KeyCode::ArrowDown) {
        let scale = (ui_scale.target_scale / 1.2).max(1.);
        ui_scale.set_scale(scale);
        info!("Scaling down! Scale: {}", ui_scale.target_scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_in_expo_starts_at_zero_and_ends_at_one() {
        assert_eq!(ease_in_expo(0.0), 0.0);
        assert!((ease_in_expo(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ease_in_expo_is_monotonically_increasing() {
        let samples: Vec<f32> = (0..=10).map(|i| ease_in_expo(i as f32 / 10.0)).collect();
        for w in samples.windows(2) {
            assert!(w[1] >= w[0], "expected non-decreasing, got {:?}", samples);
        }
    }

    #[test]
    fn ease_in_expo_stays_near_zero_for_most_of_the_range() {
        // The defining shape of an ease-*in* curve: slow start, sharp finish.
        assert!(ease_in_expo(0.5) < 0.2, "midpoint should still be near the floor");
        assert!(ease_in_expo(0.9) < ease_in_expo(1.0));
    }
}
