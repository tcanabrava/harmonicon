// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use super::gameplay_2d::{note_anim_mode, note_techniques};
use super::note_tail_2d::{NoteTail2dMaterial, tail_params};
use crate::song::chart::Modifier;

/// The six techniques shown in the legend, paired with the label to display. The
/// example modifiers carry representative intensities so each preview animates
/// clearly; the actual params/animation come from the same `note_techniques` /
/// `note_anim_mode` / `tail_params` the falling notes use, so the legend
/// can never drift from what the notes do.
fn legend_techniques() -> [(Modifier, &'static str); 5] {
    use crate::song::chart::Modifier::*;
    [
        (Bend { semitones: -1.0, intensity: None }, "bend"),
        (Vibrato { oscillation_hz: 5.0, intensity: Some(0.9) }, "vibrato"),
        (WahWah { oscillation_hz: 3.0, intensity: Some(0.9) }, "wah-wah"),
        (Overblow, "overblow"),
        (Overdraw, "overdraw"),
    ]
}

/// Builds one comet-tail material per technique for the legend previews. They are
/// regular `NoteTail2dMaterial`s, so `animate_note_tails` drives them in time with
/// everything else. A neutral colour is used on purpose — the *animation*, not the
/// colour, now tells the techniques apart.
pub fn build_legend_materials(
    materials: &mut Assets<NoteTail2dMaterial>,
) -> Vec<(Handle<NoteTail2dMaterial>, &'static str)> {
    // A short, fixed preview "note": enough length for the animations to read.
    const PREVIEW_H_PCT: f32 = 20.0;
    let color = Color::srgba(0.74, 0.82, 1.0, 0.95).to_linear();

    legend_techniques()
        .into_iter()
        .enumerate()
        .map(|(i, (modifier, name))| {
            let slice = std::slice::from_ref(&modifier);
            let (vib, shift, wah) = note_techniques(Some(slice));
            let mode = note_anim_mode(Some(slice));
            let (mut params, mut wah_v) = tail_params(PREVIEW_H_PCT, vib, shift, wah);
            params.z = 0.0; // animation clock, driven by animate_note_tails
            wah_v.z = mode; // which technique animation
            wah_v.w = i as f32 * 1.3; // stagger the phases
            let handle = materials.add(NoteTail2dMaterial {
                color,
                params,
                wah: wah_v,
            });
            (handle, name)
        })
        .collect()
}

/// Spawns the techniques legend: a small *animated tail* preview beside each
/// technique's name, so players learn to read a note by its motion. Used by both
/// the 2D and 3D HUDs. `entries` come from [`build_legend_materials`].
pub fn spawn_modifier_legend(
    parent: &mut ChildSpawnerCommands,
    font: &FontSource,
    entries: &[(Handle<NoteTail2dMaterial>, &'static str)],
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new("TECHNIQUES"),
                TextFont { font_size: FontSize::Px(10.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.62)),
            ));

            // Wrapping row so the six previews pack into a small footprint.
            col.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(12.0),
                row_gap: Val::Px(6.0),
                max_width: Val::Px(260.0),
                ..default()
            })
            .with_children(|wrap| {
                for (handle, name) in entries {
                    wrap.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|row| {
                        // The live, animated comet tail for this technique.
                        row.spawn((
                            Node {
                                width: Val::Px(18.0),
                                height: Val::Px(38.0),
                                ..default()
                            },
                            MaterialNode(handle.clone()),
                        ));
                        row.spawn((
                            Text::new(*name),
                            TextFont { font_size: FontSize::Px(10.0), font: font.clone(), ..default() },
                            TextColor(Color::srgb(0.70, 0.72, 0.78)),
                        ));
                    });
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_covers_all_techniques() {
        assert_eq!(legend_techniques().len(), 5);
    }

    #[test]
    fn legend_names_match_the_techniques() {
        let names: Vec<&str> = legend_techniques().iter().map(|(_, n)| *n).collect();
        assert_eq!(
            names,
            ["bend", "vibrato", "wah-wah", "overblow", "overdraw"]
        );
    }
}
