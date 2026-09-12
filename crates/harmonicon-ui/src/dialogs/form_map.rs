// SPDX-License-Identifier: MIT

//! Reusable map of named song sections such as A–A–B–A.

use bevy::prelude::*;

pub fn section_bg(label: &str) -> Color {
    let hash = label.bytes().fold(0_u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    let hue = (hash % 360) as f32;
    Color::hsl(hue, 0.42, 0.30)
}

pub fn spawn_form_map(parent: &mut ChildSpawnerCommands, sections: &[String]) -> Vec<Entity> {
    let mut cells = Vec::with_capacity(sections.len());
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            max_width: Val::Px(760.0),
            ..default()
        })
        .with_children(|row| {
            for (index, label) in sections.iter().enumerate() {
                let cell = row
                    .spawn((
                        Node {
                            width: Val::Px(76.0),
                            height: Val::Px(54.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(if index == 0 {
                            Color::srgba(0.82, 0.62, 0.10, 1.0)
                        } else {
                            section_bg(label)
                        }),
                        BorderColor::all(Color::srgb(0.40, 0.40, 0.55)),
                    ))
                    .with_child((
                        Text::new(format!("{}  {}", index + 1, label)),
                        TextFont {
                            font_size: FontSize::Px(20.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ))
                    .id();
                cells.push(cell);
            }
        });
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_section_labels_keep_the_same_colour() {
        assert_eq!(section_bg("A"), section_bg("A"));
        assert_ne!(section_bg("A"), section_bg("B"));
    }
}
