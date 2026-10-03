// SPDX-License-Identifier: MIT

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;

use super::note_ribbon::ribbon_technique;
use super::note_ribbon_2d::NoteRibbon2dMaterial;
use harmonicon_core::chart::Modifier;
use harmonicon_platform::localization::{Localization, LocalizationExt};

/// Whether the techniques legend body is collapsed, toggled by clicking its
/// header. Not reset on song load — like [`super::metronome_overlay::
/// MetronomeMuted`], a player's preference should outlive one song.
#[derive(Resource, Default)]
pub struct TechniqueLegendCollapsed(pub bool);

/// The column of technique rows, hidden/shown by [`TechniqueLegendCollapsed`].
#[derive(Component)]
struct TechniqueLegendBody;

/// The header's text, carrying the collapse/expand arrow.
#[derive(Component, Default, Clone)]
struct TechniqueLegendToggleLabel;

/// The techniques shown in the legend, paired with their label. The drawing
/// comes from the same `ribbon_technique` the falling notes use, so the
/// legend can't drift from what the notes do.
fn legend_techniques() -> [(Modifier, &'static str); 6] {
    use harmonicon_core::chart::Modifier::*;
    [
        (Bend { semitones: -1.0, intensity: None }, "mod-bend"),
        (Vibrato { oscillation_hz: 5.0, intensity: Some(0.9) }, "mod-vibrato"),
        (WahWah { oscillation_hz: 3.0, intensity: Some(0.9) }, "mod-wah"),
        (Overblow, "mod-overblow"),
        (Overdraw, "mod-overdraw"),
        (Slide, "mod-slide"),
    ]
}

fn modifier_kind(modifier: &Modifier) -> u8 {
    match modifier {
        Modifier::Bend { .. } => 0,
        Modifier::Vibrato { .. } => 1,
        Modifier::WahWah { .. } => 2,
        Modifier::Overblow => 3,
        Modifier::Overdraw => 4,
        Modifier::Slide => 5,
    }
}

/// Representative legend entries for only the techniques present in a chart.
/// Keeping this decision independent of rendering makes it impossible for a
/// plain-note song to spend permanent HUD space teaching five unrelated
/// symbols, and gives chromatic slide charts their slide entry.
fn used_legend_techniques(modifiers: &[Modifier]) -> Vec<(Modifier, &'static str)> {
    legend_techniques()
        .into_iter()
        .filter(|(candidate, _)| {
            modifiers.iter().any(|used| modifier_kind(used) == modifier_kind(candidate))
        })
        .collect()
}

/// Builds one ribbon material per technique for the legend previews:
/// regular `NoteRibbon2dMaterial`s, so `animate_note_ribbons` drives them in
/// time with everything else. A falling note's pattern moves because the
/// note does; a preview stands still, so it scrolls its pattern instead
/// (`shape.w`), at the same rate a note would carry it past the hit line. A
/// neutral colour is used on purpose — the *drawing*, not the colour, tells
/// the techniques apart.
pub fn build_legend_materials(
    materials: &mut Assets<NoteRibbon2dMaterial>,
    used_modifiers: &[Modifier],
) -> Vec<(Handle<NoteRibbon2dMaterial>, &'static str)> {
    /// Note time the preview spans: enough for a few swings to read.
    const PREVIEW_SECS: f32 = 0.9;
    /// A small cap, in px, to show which end is the attack.
    const PREVIEW_CAP_PX: f32 = 5.0;
    let color = Color::srgba(0.74, 0.82, 1.0, 0.95).to_linear();

    used_legend_techniques(used_modifiers)
        .into_iter()
        .map(|(modifier, name)| {
            let handle = materials.add(NoteRibbon2dMaterial {
                color,
                technique: ribbon_technique(std::slice::from_ref(&modifier)),
                shape: Vec4::new(PREVIEW_SECS, PREVIEW_CAP_PX, 0.0, 1.0),
                hold: Vec4::ZERO,
            });
            (handle, name)
        })
        .collect()
}

/// Arrow shown on the collapse/expand toggle, matched to `collapsed`.
fn toggle_arrow(collapsed: bool) -> &'static str {
    if collapsed { "\u{25B6}" } else { "\u{25BC}" }
}

/// The "▼ TECHNIQUES" toggle-header text for the given collapsed state —
/// shared by the initial `bsn!` placeholder and
/// [`update_technique_legend_visibility`] so the two can't drift apart.
fn technique_legend_toggle_text(loc: &Localization, collapsed: bool) -> String {
    loc.msg_args("gameplay-techniques-toggle", &[("arrow", toggle_arrow(collapsed).to_string())])
        .into()
}

/// Spawns the techniques legend: a small ribbon preview beside each
/// technique's name, drawn as the notes draw it, stacked
/// one per row under a clickable header that collapses/expands the list. Used
/// by both the 2D and 3D HUDs. `entries` come from [`build_legend_materials`].
pub fn spawn_modifier_legend(
    parent: &mut ChildSpawnerCommands,
    loc: &Localization,
    entries: &[(Handle<NoteRibbon2dMaterial>, &'static str)],
) {
    parent
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .with_children(|col| {
            col.spawn_empty().apply_scene(bsn! {
                WidgetButton
                TabIndex(0)
                Node { padding: {UiRect::ZERO} }
                BackgroundColor({Color::NONE})
                on(toggle_technique_legend)
                Children [
                    Text({technique_legend_toggle_text(loc, false)})
                    TextFont { font_size: {FontSize::Px(15.0)} }
                    TextColor({Color::srgb(0.55, 0.55, 0.62)})
                    TechniqueLegendToggleLabel
                    Pickable { should_block_lower: {false}, is_hoverable: {false} }
                ]
            });

            // One technique per row (icon left, name right), stacked vertically
            // instead of wrapping, so the legend's width never varies with how
            // many entries fit per line.
            col.spawn((
                Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() },
                TechniqueLegendBody,
            ))
            .with_children(|list| {
                for (handle, label_key) in entries {
                    list.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|row| {
                        // A short ribbon drawing this technique. `MaterialNode`
                        // has no `Template` impl (its handle isn't `Unpin`),
                        // so this stays a plain tuple spawn.
                        row.spawn((
                            Node { width: Val::Px(24.0), height: Val::Px(44.0), ..default() },
                            MaterialNode(handle.clone()),
                        ));
                        row.spawn_empty().apply_scene(bsn! {
                            Text({String::from(loc.msg(label_key))})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.70, 0.72, 0.78)})
                        });
                    });
                }
            });
        });
}

fn toggle_technique_legend(_: On<Activate>, mut collapsed: ResMut<TechniqueLegendCollapsed>) {
    collapsed.0 = !collapsed.0;
}

/// Mirrors [`TechniqueLegendCollapsed`] onto the body's visibility and the
/// header's arrow when the setting or locale changes, and onto a freshly
/// spawned legend (a new one is spawned per song) so it isn't stale.
fn update_technique_legend_visibility(
    collapsed: Res<TechniqueLegendCollapsed>,
    loc: Res<Localization>,
    mut bodies: Query<(&mut Node, Ref<TechniqueLegendBody>)>,
    mut labels: Query<(&mut Text, Ref<TechniqueLegendToggleLabel>)>,
) {
    let all = collapsed.is_changed() || loc.is_changed();
    for (mut node, marker) in &mut bodies {
        if all || marker.is_added() {
            node.display = if collapsed.0 { Display::None } else { Display::Flex };
        }
    }
    for (mut text, marker) in &mut labels {
        if all || marker.is_added() {
            *text = Text::new(technique_legend_toggle_text(&loc, collapsed.0));
        }
    }
}

pub struct ModifierLegendPlugin;

impl Plugin for ModifierLegendPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TechniqueLegendCollapsed>()
            .add_systems(Update, update_technique_legend_visibility);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_covers_all_techniques() {
        assert_eq!(legend_techniques().len(), 6);
    }

    #[test]
    fn legend_names_match_the_techniques() {
        let names: Vec<&str> = legend_techniques().iter().map(|(_, n)| *n).collect();
        assert_eq!(
            names,
            ["mod-bend", "mod-vibrato", "mod-wah", "mod-overblow", "mod-overdraw", "mod-slide"]
        );
    }

    #[test]
    fn legend_contains_only_techniques_the_chart_uses_in_canonical_order() {
        let used = [
            Modifier::Slide,
            Modifier::Vibrato { oscillation_hz: 4.0, intensity: None },
            Modifier::Slide,
        ];
        let names: Vec<&str> =
            used_legend_techniques(&used).into_iter().map(|(_, name)| name).collect();
        assert_eq!(names, ["mod-vibrato", "mod-slide"]);
    }

    #[test]
    fn plain_notes_need_no_technique_legend() {
        assert!(used_legend_techniques(&[]).is_empty());
    }
}
