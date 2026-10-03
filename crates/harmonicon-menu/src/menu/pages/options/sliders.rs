// SPDX-License-Identifier: MIT

//! The music / metronome volume sliders and the input-latency slider,
//! plus the systems that mirror `AudioSettings` back onto their fills.

use super::*;

// ── Dedicated slider callbacks ────────────────────────────────────────────────

pub(super) fn set_music_volume(ev: On<ValueChange<f32>>, mut settings: ResMut<AudioSettings>) {
    if settings.music_volume != ev.value {
        settings.music_volume = ev.value;
    }
}

pub(super) fn set_metronome_volume(ev: On<ValueChange<f32>>, mut settings: ResMut<AudioSettings>) {
    if settings.metronome_volume != ev.value {
        settings.metronome_volume = ev.value;
    }
}

pub(super) fn set_input_latency(ev: On<ValueChange<f32>>, mut settings: ResMut<AudioSettings>) {
    let latency = ev.value.round() as i32;
    if settings.input_latency_ms != latency {
        settings.input_latency_ms = latency;
    }
}

// ── Volume sliders ──────────────────────────────────────────────────────────

/// The shared row shell every Options slider builds on: a 420px row with a
/// 110px label carrying `tooltip`, already attached to `parent` —
/// `spawn_volume_slider`/`spawn_latency_slider`/`spawn_zoom_slider` differ
/// only in the label/tooltip text and, once this returns, which slider
/// track/value-readout they add to the row.
pub(super) fn spawn_slider_row(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    tooltip: &str,
) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Px(420.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .id();
    commands.entity(row).with_children(|r| {
        r.spawn_empty().apply_scene(bsn! {
            Node { width: {Val::Px(OPTIONS_LABEL_WIDTH)} }
            Text({label.to_string()})
            TextFont { font_size: {FontSize::Px(20.0)} }
            TextColor({Color::WHITE})
        });
    });
    commands.entity(row).insert(Tooltip(tooltip.to_string()));
    commands.entity(parent).add_child(row);
    row
}

/// The shared trailing value-readout `spawn_volume_slider`/
/// `spawn_latency_slider` both append after their track — a 50px right-hand
/// label tagged with whichever marker component that caller's own sync
/// system reads (`SliderValueLabel`/`LatencySliderLabel`).
pub(super) fn spawn_slider_value_label(
    commands: &mut Commands,
    row: Entity,
    text: String,
    marker: impl Component,
) {
    commands.entity(row).with_children(|r| {
        r.spawn_empty()
            .apply_scene(bsn! {
                Node { width: {Val::Px(50.0)} }
                Text({text})
                TextFont { font_size: {FontSize::Px(18.0)} }
                TextColor({Color::srgb(0.6, 0.6, 0.7)})
            })
            .insert(marker);
    });
}

pub(super) fn spawn_volume_slider<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    tooltip: &str,
    kind: VolumeSlider,
    value: f32,
    on_change: impl IntoObserverSystem<ValueChange<f32>, M> + Clone + Sync + 'static,
) {
    let row = spawn_slider_row(commands, parent, label, tooltip);

    // SliderRange/SliderStep have no Default, so they can't be bsn! patches —
    // insert them after the scene is spawned.
    let track = commands
        .spawn_scene(volume_slider_scene(kind, value, on_change))
        .insert((SliderRange::new(0.0, 1.0), SliderStep(0.01)))
        .id();
    commands.entity(row).add_child(track);

    spawn_slider_value_label(
        commands,
        row,
        format!("{:.0}%", value * 100.0),
        SliderValueLabel(kind),
    );
}

/// The volume slider track itself: a `bsn!` `Slider` with its fill, wired to the
/// given value-change callback inline via `on(...)`.
pub(super) fn volume_slider_scene<M: 'static>(
    kind: VolumeSlider,
    value: f32,
    on_change: impl IntoObserverSystem<ValueChange<f32>, M> + Clone + Sync + 'static,
) -> impl Scene {
    bsn! {
        Slider { track_click: {TrackClick::Snap} }
        TabIndex(0)
        SliderValue({value})
        Node { width: {Val::Px(220.0)}, height: {Val::Px(14.0)} }
        BackgroundColor({TRACK_BG})
        on(on_change)
        Children [
            Node { width: {Val::Percent(value * 100.0)}, height: {Val::Percent(100.0)} }
            BackgroundColor({Color::srgb(0.35, 0.75, 1.0)})
            SliderFill({kind})
            // Don't let the fill steal the slider's pointer events.
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

// ── Input-latency slider ──────────────────────────────────────────────────────

pub(super) const LATENCY_MAX_MS: i32 = 200;

/// One labelled slider row for the mic input-latency offset.
/// The track maps 0–200 ms linearly; the label shows "Xms".
pub(super) fn spawn_latency_slider(
    commands: &mut Commands,
    parent: Entity,
    value_ms: i32,
    loc: &Localization,
) {
    let frac = (value_ms as f32 / LATENCY_MAX_MS as f32).clamp(0.0, 1.0);
    let label = String::from(loc.msg("options-input-lag"));
    let tooltip = String::from(loc.msg("options-input-lag-tooltip"));
    let row = spawn_slider_row(commands, parent, &label, &tooltip);

    let track = commands
        .spawn_scene(latency_slider_scene(value_ms as f32, frac))
        .insert((SliderRange::new(0.0, LATENCY_MAX_MS as f32), SliderStep(1.0)))
        .id();
    commands.entity(row).add_child(track);

    spawn_slider_value_label(commands, row, format!("{value_ms}ms"), LatencySliderLabel);
}

/// The latency slider track: a `bsn!` `Slider` + fill, wired to `set_input_latency`.
pub(super) fn latency_slider_scene(value: f32, frac: f32) -> impl Scene {
    bsn! {
        Slider { track_click: {TrackClick::Snap} }
        TabIndex(0)
        SliderValue({value})
        Node { width: {Val::Px(220.0)}, height: {Val::Px(14.0)} }
        BackgroundColor({TRACK_BG})
        LatencySlider
        on(set_input_latency)
        Children [
            Node { width: {Val::Percent(frac * 100.0)}, height: {Val::Percent(100.0)} }
            BackgroundColor({Color::srgb(0.80, 0.55, 0.25)})
            LatencySliderFill
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

/// Mirror `input_latency_ms` onto the fill bar and label.
pub(super) fn update_latency_slider(
    settings: Res<AudioSettings>,
    mut fills: Query<&mut Node, With<LatencySliderFill>>,
    mut labels: Query<&mut Text, With<LatencySliderLabel>>,
) {
    if !settings.is_changed() {
        return;
    }
    let frac = (settings.input_latency_ms as f32 / LATENCY_MAX_MS as f32).clamp(0.0, 1.0);
    for mut node in &mut fills {
        let width = Val::Percent(frac * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
    let label = format!("{}ms", settings.input_latency_ms);
    for mut text in &mut labels {
        if text.0 != label {
            text.0.clone_from(&label);
        }
    }
}

/// Mirror the current levels onto the slider fills and percentage readouts.
pub(super) fn update_sliders(
    settings: Res<AudioSettings>,
    mut fills: Query<(&mut Node, &SliderFill)>,
    mut labels: Query<(&mut Text, &SliderValueLabel)>,
) {
    if !settings.is_changed() {
        return;
    }
    for (mut node, fill) in &mut fills {
        let width = Val::Percent(audio_level(&settings, fill.0) * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
    for (mut text, label) in &mut labels {
        let wanted = format!("{:.0}%", audio_level(&settings, label.0) * 100.0);
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}
