// SPDX-License-Identifier: MIT

//! The UI zoom slider: `UiScale` as a percentage, with the track kept in
//! step when the scale changes from the keyboard shortcut instead.

use super::*;

/// Marks the zoom slider's own track, so its `SliderValue` can be told apart
/// from the volume/latency sliders', which also carry one.
#[derive(Component, Default, Clone)]
pub(super) struct ZoomSlider;

/// The fill bar inside the zoom slider track.
#[derive(Component, Default, Clone)]
pub(super) struct ZoomSliderFill;

pub(super) fn zoom_label_text(loc: &Localization, scale: f32) -> String {
    loc.msg_args("options-zoom-label", &[("percent", (scale * 100.0).round().to_string())]).into()
}

/// Where `scale` sits between `dialogs::ui_scale`'s `MIN_SCALE`/`MAX_SCALE`,
/// as a `0.0..=1.0` fraction — shared by the slider's initial spawn position
/// and its live fill width.
pub(super) fn zoom_fraction(scale: f32) -> f32 {
    use harmonicon_ui::dialogs::ui_scale::{MAX_SCALE, MIN_SCALE};
    ((scale - MIN_SCALE) / (MAX_SCALE - MIN_SCALE)).clamp(0.0, 1.0)
}

/// Commits the dragged/stepped value to the real `UiScale` only once the
/// interaction is finished (`is_final`), never on every drag frame:
/// `UiScale` changing forces Bevy to re-rasterize every visible glyph at
/// the new effective size, and applying that continuously mid-drag risks
/// the same GPU-memory-exhaustion crash `dialogs::ui_scale`'s doc comment
/// describes for the keyboard shortcut. The live drag preview comes from
/// `SliderValue` instead (mirrored onto the fill/label by
/// [`update_zoom_slider_visuals`]), which costs nothing to update every
/// frame.
pub(super) fn set_zoom(ev: On<ValueChange<f32>>, mut ui_scale: ResMut<UiScale>) {
    if ev.is_final && ui_scale.0 != ev.value {
        ui_scale.0 = ev.value;
    }
}

/// A labelled zoom slider — the only way to change `UiScale`; an earlier
/// Arrow Up/Down keyboard shortcut was removed because it conflicted with
/// Tab/arrow-key UI navigation.
pub(super) fn spawn_zoom_slider(
    commands: &mut Commands,
    parent: Entity,
    scale: f32,
    loc: &Localization,
) {
    use harmonicon_ui::dialogs::ui_scale::{MAX_SCALE, MIN_SCALE};

    let label = String::from(loc.msg("options-zoom"));
    let tooltip = String::from(loc.msg("options-zoom-tooltip"));
    let row = spawn_slider_row(commands, parent, &label, &tooltip);
    let frac = zoom_fraction(scale);

    let track = commands
        .spawn_scene(zoom_slider_scene(scale, frac))
        .insert((SliderRange::new(MIN_SCALE, MAX_SCALE), SliderStep(0.1)))
        .id();
    commands.entity(row).add_child(track);

    spawn_slider_value_label(commands, row, zoom_label_text(loc, scale), ZoomLabel);
}

/// The zoom slider track: a `bsn!` `Slider` + fill, wired to [`set_zoom`].
pub(super) fn zoom_slider_scene(value: f32, frac: f32) -> impl Scene {
    bsn! {
        Slider { track_click: {TrackClick::Snap} }
        TabIndex(0)
        SliderValue({value})
        Node { width: {Val::Px(220.0)}, height: {Val::Px(14.0)} }
        BackgroundColor({TRACK_BG})
        ZoomSlider
        on(set_zoom)
        Children [
            Node { width: {Val::Percent(frac * 100.0)}, height: {Val::Percent(100.0)} }
            BackgroundColor({Color::srgb(0.55, 0.45, 0.85)})
            ZoomSliderFill
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

/// Mirrors changed slider values onto the fill and percentage label.
pub(super) fn update_zoom_slider_visuals(
    loc: Res<Localization>,
    sliders: Query<Ref<SliderValue>, With<ZoomSlider>>,
    mut fills: Query<&mut Node, With<ZoomSliderFill>>,
    mut labels: Query<&mut Text, With<ZoomLabel>>,
) {
    let Ok(value) = sliders.single() else {
        return;
    };
    if !value.is_changed() && !loc.is_changed() {
        return;
    }
    for mut node in &mut fills {
        let width = Val::Percent(zoom_fraction(value.0) * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
    let label = zoom_label_text(&loc, value.0);
    for mut text in &mut labels {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}

/// Keeps the slider's own `SliderValue` in step with `UiScale` if it changes
/// outside the slider. `SliderValue` is an immutable component (replace via
/// `insert`, not `&mut`), same as every other `bevy_ui_widgets` value type.
pub(super) fn sync_zoom_slider_from_ui_scale(
    ui_scale: Res<UiScale>,
    sliders: Query<(Entity, &SliderValue), With<ZoomSlider>>,
    mut commands: Commands,
) {
    if !ui_scale.is_changed() {
        return;
    }
    for (entity, value) in &sliders {
        if value.0 != ui_scale.0 {
            commands.entity(entity).insert(SliderValue(ui_scale.0));
        }
    }
}
