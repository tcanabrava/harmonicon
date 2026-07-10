// SPDX-License-Identifier: MIT

//! Shared pitch-detection algorithm picker: a [`combobox`] plus a read-only
//! explanation of whichever algorithm is selected. Used on the Options page
//! and, so a player can quickly compare algorithms while actually bending
//! notes, in the Bending Trainer — both drive the same global
//! [`AudioSettings::pitch_algorithm`] via [`on_algo_selected`], so picking
//! one anywhere takes effect everywhere immediately.

use bevy::prelude::*;

use crate::audio_system::pitch_detect::PitchAlgorithm;
use crate::dialogs::combobox::ComboboxSelect;
use crate::settings::AudioSettings;

/// The read-only text explaining the currently selected algorithm.
#[derive(Component)]
pub struct AlgoExplanation;

/// Every algorithm's label, in [`PitchAlgorithm::all`]'s order — the options
/// list for a `dialogs::combobox`-based algorithm picker.
pub fn algo_labels() -> Vec<String> {
    PitchAlgorithm::all()
        .iter()
        .map(|a| a.label().to_string())
        .collect()
}

/// A combobox `on_select` that writes straight to the shared global
/// [`AudioSettings::pitch_algorithm`] — picking an algorithm from either the
/// Options page's or the Bending Trainer's combobox takes effect everywhere
/// immediately. Unrecognized values (shouldn't happen — [`algo_labels`]
/// always matches [`PitchAlgorithm::all`]) are ignored rather than silently
/// resetting the setting.
pub fn on_algo_selected(ev: On<ComboboxSelect>, mut settings: ResMut<AudioSettings>) {
    if let Some(algo) = PitchAlgorithm::from_label(&ev.value) {
        settings.pitch_algorithm = algo;
    }
}

/// A read-only box explaining the currently selected pitch algorithm, `width`
/// pixels wide (the Options page and the Bending Trainer's side column want
/// different widths).
pub fn spawn_algo_explanation(
    commands: &mut Commands,
    parent: Entity,
    width: f32,
    selected: PitchAlgorithm,
) {
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(width),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.10, 0.14, 0.85)),
        ))
        .id();
    commands.entity(panel).with_children(|p| {
        p.spawn((
            Text::new(selected.description()),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.75, 0.78, 0.88)),
            AlgoExplanation,
        ));
    });
    commands.entity(parent).add_child(panel);
}

/// Keep every explanation box in step with the chosen algorithm.
pub fn update_algo_explanation(
    settings: Res<AudioSettings>,
    mut texts: Query<&mut Text, With<AlgoExplanation>>,
) {
    if !settings.is_changed() {
        return;
    }
    for mut text in &mut texts {
        *text = Text::new(settings.pitch_algorithm.description());
    }
}

/// Runs [`update_algo_explanation`] unconditionally: it only touches entities
/// carrying [`AlgoExplanation`], so it's a no-op on any screen that hasn't
/// spawned this widget.
pub struct AlgoPickerPlugin;

impl Plugin for AlgoPickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_algo_explanation);
    }
}
