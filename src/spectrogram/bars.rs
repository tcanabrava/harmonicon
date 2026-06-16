// SPDX-License-Identifier: MIT

//! Bar spectrogram: one vertical bar per frequency band, growing from the
//! bottom. Low frequencies on the left, highs on the right.

use bevy::prelude::*;

use super::{NUM_BANDS, Spectrum};

/// Marks one bar and records which band it draws.
#[derive(Component)]
pub struct SpectrumBar(usize);

/// Spawns the bar row, filling `parent` (anchored to the bottom).
pub fn spawn(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd, // bars grow up from the bottom
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .with_children(|row| {
            for b in 0..NUM_BANDS {
                row.spawn((
                    Node {
                        width: Val::Px(10.0),
                        height: Val::Percent(1.0),
                        ..default()
                    },
                    BackgroundColor(band_color(b, 0.0)),
                    SpectrumBar(b),
                ));
            }
        });
}

/// Drives bar heights and colors from the current [`Spectrum`].
pub fn update_bars(
    spectrum: Res<Spectrum>,
    mut bars: Query<(&SpectrumBar, &mut Node, &mut BackgroundColor)>,
) {
    for (bar, mut node, mut bg) in &mut bars {
        let level = spectrum
            .bands
            .get(bar.0)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        // Keep a 1% floor so idle bars stay visible as a baseline.
        node.height = Val::Percent(1.0 + level * 99.0);
        *bg = BackgroundColor(band_color(bar.0, level));
    }
}

/// A blue→magenta gradient across the band index, brightened by level.
fn band_color(band: usize, level: f32) -> Color {
    let hue = band as f32 / NUM_BANDS as f32; // 0 (low) .. 1 (high)
    let bright = 0.35 + 0.65 * level;
    Color::srgb(
        (0.20 + 0.75 * hue) * bright,
        (0.45 + 0.25 * (1.0 - hue)) * bright,
        (0.95 - 0.45 * hue) * bright,
    )
}
