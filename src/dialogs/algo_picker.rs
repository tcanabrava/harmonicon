// SPDX-License-Identifier: MIT

//! Shared pitch-detection algorithm picker: a row of choice buttons plus a
//! read-only explanation of whichever one is selected. Used on the Options
//! page and, so a player can quickly compare algorithms while actually
//! bending notes, in the Bending Trainer — both drive the same global
//! [`AudioSettings::pitch_algorithm`], so picking one anywhere takes effect
//! everywhere immediately.

use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;

use crate::audio_system::pitch_detect::PitchAlgorithm;
use crate::dialogs::button::{self, CHOICE_HOVER, CHOICE_SELECTED};
use crate::settings::AudioSettings;

/// Marks one algorithm choice button, carrying which algorithm it selects.
#[derive(Component, Default, Clone)]
pub struct AlgoButton(pub PitchAlgorithm);

/// The read-only text explaining the currently selected algorithm.
#[derive(Component)]
pub struct AlgoExplanation;

/// A row of one button per [`PitchAlgorithm`], highlighting `selected`. Wraps
/// onto further lines if the row is narrower than all five buttons need (the
/// Bending Trainer's side column, unlike the Options page, doesn't have the
/// width to spare). `label`, if given, is a leading caption for the row.
pub fn spawn_algo_row(
    commands: &mut Commands,
    parent: Entity,
    label: Option<&str>,
    selected: PitchAlgorithm,
) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    commands.entity(row).with_children(|r| {
        if let Some(label) = label {
            r.spawn((
                Node {
                    width: Val::Px(110.0),
                    ..default()
                },
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        }
        for &algo in PitchAlgorithm::all() {
            r.spawn_empty()
                .apply_scene(algo_button_scene(algo, algo == selected));
        }
    });

    commands.entity(parent).add_child(row);
}

/// One algorithm choice button: its label + a dedicated "select this algorithm"
/// click callback (capturing the algorithm) plus hover — all inline `on(...)`.
fn algo_button_scene(algo: PitchAlgorithm, is_selected: bool) -> impl Scene {
    let color = if is_selected {
        CHOICE_SELECTED
    } else {
        button::color_default()
    };
    bsn! {
        Button
        Node {
            padding: {UiRect::axes(Val::Px(14.0), Val::Px(8.0))},
        }
        BackgroundColor({color})
        AlgoButton({algo})
        on(move |_: On<Pointer<Click>>, mut settings: ResMut<AudioSettings>| {
            settings.pitch_algorithm = algo;
        })
        on(algo_over)
        on(algo_out)
        Children [
            (
                Text({algo.label().to_string()})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

fn algo_over(
    ev: On<Pointer<Over>>,
    settings: Res<AudioSettings>,
    mut buttons: Query<(&AlgoButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != settings.pitch_algorithm
    {
        *bg = BackgroundColor(CHOICE_HOVER);
    }
}

fn algo_out(
    ev: On<Pointer<Out>>,
    settings: Res<AudioSettings>,
    mut buttons: Query<(&AlgoButton, &mut BackgroundColor)>,
) {
    if let Ok((btn, mut bg)) = buttons.get_mut(ev.entity)
        && btn.0 != settings.pitch_algorithm
    {
        *bg = BackgroundColor(button::color_default());
    }
}

/// Recolour every algorithm button — on the Options page or in the Bending
/// Trainer, wherever they're on screen — when the selection changes.
pub fn algo_button_visuals(
    settings: Res<AudioSettings>,
    mut buttons: Query<(&AlgoButton, &mut BackgroundColor)>,
) {
    if !settings.is_changed() {
        return;
    }
    for (button, mut bg) in &mut buttons {
        bg.0 = if button.0 == settings.pitch_algorithm {
            CHOICE_SELECTED
        } else {
            button::color_default()
        };
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
                font_size: FontSize::Px(14.0),
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

/// Runs the two reactive systems above unconditionally: they only touch
/// entities carrying [`AlgoButton`]/[`AlgoExplanation`], so they're a no-op
/// on any screen that hasn't spawned this widget.
pub struct AlgoPickerPlugin;

impl Plugin for AlgoPickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (algo_button_visuals, update_algo_explanation));
    }
}
