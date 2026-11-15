// SPDX-License-Identifier: MIT

//! The microphone picker and the status banner beside it — the one place
//! the `MicStatus::Failed` reason is shown in full, with a Retry button.

use super::*;

// ── Microphone picker / status banner ───────────────────────────────────────

/// The name of the device actually connected right now, or `None` while
/// [`MicStatus::Failed`]/[`MicStatus::AwaitingPermission`]. Used (rather
/// than the raw `AudioSettings::input_device` preference) so the picker
/// highlights reality — if a saved device went missing and capture fell
/// back to the default, that's what lights up.
pub(super) fn connected_device_name(status: &MicStatus) -> Option<&str> {
    match status {
        MicStatus::Connected { device_name } => Some(device_name.as_str()),
        MicStatus::Failed { .. } | MicStatus::AwaitingPermission => None,
    }
}

/// Whether the mic warning banner (and the device combobox it stands in
/// for) should be visible — anything other than a successful connection.
pub(super) fn mic_banner_visible(status: &MicStatus) -> bool {
    // Delegated rather than re-matched, so this page and the in-play warning
    // overlay can't drift apart on what counts as a working microphone.
    !status.is_connected()
}

/// A dismiss-free warning banner, hidden only once the microphone actually
/// connects (see [`mic_banner_visible`]), with a Retry button that re-runs
/// `audio_input::start_capture`.
pub(super) fn spawn_mic_banner(
    commands: &mut Commands,
    parent: Entity,
    status: &MicStatus,
    loc: &Localization,
) {
    let visible = mic_banner_visible(status);
    let text = mic_banner_text(status, loc);

    let display = if visible { Display::Flex } else { Display::None };
    let banner = commands
        .spawn_empty()
        .apply_scene(bsn! {
            Node {
                width: {Val::Px(560.0)},
                flex_direction: {FlexDirection::Row},
                align_items: {AlignItems::Center},
                column_gap: {Val::Px(14.0)},
                padding: {UiRect::all(Val::Px(10.0))},
                display: {display},
            }
            BackgroundColor({Color::srgba(0.45, 0.12, 0.12, 0.85)})
            Children [
                Text({text})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.95, 0.85, 0.85)})
                MicBannerText
            ]
        })
        .insert(MicBanner)
        .id();

    commands.entity(banner).with_children(|b| {
        b.spawn_empty().apply_scene(mic_retry_button_scene(
            String::from(loc.msg("results-retry")),
            String::from(loc.msg("options-mic-retry-tooltip")),
        ));
    });

    commands.entity(parent).add_child(banner);
}

/// The banner's message. Unlike the in-play warning
/// (`gameplay::mic_warning_overlay`), this one *does* include the raw
/// `Failed { reason }` string: Options is where a player has come to fix
/// the problem, so the underlying cpal error ("Device not available") is
/// the useful part, even untranslated.
pub(super) fn mic_banner_text(status: &MicStatus, loc: &Localization) -> String {
    match status {
        MicStatus::Failed { reason } => String::from(
            loc.msg_args(mic_banner_key(status).unwrap_or_default(), &[("reason", reason.clone())]),
        ),
        MicStatus::AwaitingPermission => {
            String::from(loc.msg(mic_banner_key(status).unwrap_or_default()))
        }
        MicStatus::Connected { .. } => String::new(),
    }
}

/// Which Fluent key the banner uses, or `None` when connected. Split out of
/// [`mic_banner_text`] so that "does each status get its own message" stays
/// testable — the text itself now needs a `Localization`, which a unit test
/// has no cheap way to build.
pub(super) fn mic_banner_key(status: &MicStatus) -> Option<&'static str> {
    match status {
        MicStatus::Connected { .. } => None,
        MicStatus::AwaitingPermission => Some("options-mic-awaiting-permission"),
        MicStatus::Failed { .. } => Some("options-mic-failed"),
    }
}

pub(super) fn mic_retry_button_scene(label: String, tooltip: String) -> impl Scene {
    bsn! {
        WidgetButton
        TabIndex(0)
        Node { padding: {UiRect::axes(Val::Px(12.0), Val::Px(6.0))} }
        BackgroundColor({button::color_default()})
        Tooltip({tooltip})
        on(|_: On<Activate>, mut commands: Commands| {
            commands.queue(audio_input::start_capture);
        })
        Children [
            Text({label})
            TextFont { font_size: {FontSize::Px(15.0)} }
            TextColor({Color::WHITE})
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

/// Show/hide the banner and refresh its reason text when `MicStatus` changes
/// (e.g. after a Retry click or a device-picker selection).
pub(super) fn update_mic_banner(
    status: Res<MicStatus>,
    loc: Res<Localization>,
    mut banners: Query<&mut Node, With<MicBanner>>,
    mut texts: Query<&mut Text, With<MicBannerText>>,
) {
    if !status.is_changed() && !loc.is_changed() {
        return;
    }
    let visible = mic_banner_visible(&status);
    for mut node in &mut banners {
        let display = if visible { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    let text = mic_banner_text(&status, &loc);
    for mut t in &mut texts {
        if t.0 != text {
            t.0.clone_from(&text);
        }
    }
}

/// Marks the Options page's microphone combobox root, so [`sync_mic_combobox`]
/// can find it to push `MicStatus` changes into its display — e.g. after
/// Retry reconnects to a different actual device than was last picked, or a
/// saved device disappears and capture silently falls back to the default.
#[derive(Component)]
pub(super) struct MicCombobox;

/// Wires the shared [`combobox`] widget to the microphone device list:
/// picking an option persists it to `AudioSettings` and reconnects capture
/// immediately.
pub(super) fn spawn_mic_combobox(
    commands: &mut Commands,
    parent: Entity,
    page_root: Entity,
    loc: &Localization,
    devices: &[String],
    connected: Option<&str>,
) {
    spawn_input_meter(commands, parent, loc);
    let root = combobox::spawn_combobox(
        commands,
        parent,
        page_root,
        &loc.msg("options-microphone"),
        devices,
        connected.unwrap_or("None"),
        on_mic_selected,
    );
    commands
        .entity(root)
        .insert((MicCombobox, Tooltip(String::from(loc.msg("options-microphone-tooltip")))));
}

pub(super) fn on_mic_selected(
    ev: On<combobox::ComboboxSelect>,
    mut settings: ResMut<AudioSettings>,
    mut commands: Commands,
) {
    settings.input_device = ev.value.clone();
    commands.queue(audio_input::start_capture);
}

pub(super) fn sync_mic_combobox(
    status: Res<MicStatus>,
    combo: Query<Entity, With<MicCombobox>>,
    mut values: Query<&mut combobox::ComboboxValue>,
) {
    if !status.is_changed() {
        return;
    }
    let Ok(root) = combo.single() else { return };
    let Ok(mut value) = values.get_mut(root) else {
        return;
    };
    let connected = connected_device_name(&status).unwrap_or("None");
    if value.0 != connected {
        value.0.clear();
        value.0.push_str(connected);
    }
}

#[derive(Component)]
pub(super) struct InputLevelFill;
#[derive(Component)]
pub(super) struct InputLevelText;

fn spawn_input_meter(commands: &mut Commands, parent: Entity, loc: &Localization) {
    let panel = commands
        .spawn(Node {
            width: Val::Px(560.0),
            max_width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let label = commands
        .spawn((
            Text::new(loc.msg_args("options-mic-level", &[("percent", "0".to_string())])),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            InputLevelText,
        ))
        .id();
    let track = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(20.0), ..default() },
            BackgroundColor(Color::srgb(0.12, 0.15, 0.2)),
        ))
        .id();
    let fill = commands
        .spawn((
            Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::srgb(0.2, 0.85, 0.4)),
            InputLevelFill,
        ))
        .id();
    commands.entity(track).add_child(fill);
    commands.entity(panel).add_children(&[label, track]);
    commands.entity(parent).add_child(panel);
}

// Map RMS amplitude onto a readable -60 dBFS to 0 dBFS scale.
fn input_percent(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let power = samples.iter().map(|s| if s.is_finite() { s * s } else { 0.0 }).sum::<f32>()
        / samples.len() as f32;
    ((10.0 * power.max(1e-12).log10() + 60.0) / 60.0 * 100.0).clamp(0.0, 100.0)
}

pub(super) fn update_input_meter(
    frame: Res<harmonicon_audio::pitch_detect::AudioFrame>,
    status: Res<MicStatus>,
    loc: Res<Localization>,
    time: Res<Time>,
    mut level: Local<f32>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), With<InputLevelFill>>,
    mut labels: Query<&mut Text, With<InputLevelText>>,
) {
    let target = if status.is_connected() { input_percent(&frame.samples) } else { 0.0 };
    *level = if target >= *level || !status.is_connected() {
        target
    } else {
        target.max(*level - time.delta_secs() * 100.0)
    };
    for (mut node, mut color) in &mut fills {
        node.width = Val::Percent(*level);
        color.0 =
            if *level > 95.0 { Color::srgb(1.0, 0.3, 0.2) } else { Color::srgb(0.2, 0.85, 0.4) };
    }
    for mut label in &mut labels {
        **label =
            loc.msg_args("options-mic-level", &[("percent", format!("{:.0}", *level))]).to_string();
    }
}

#[cfg(test)]
mod level_tests {
    use super::input_percent;
    #[test]
    fn silence_and_louder_input_have_expected_levels() {
        assert_eq!(input_percent(&[]), 0.0);
        assert_eq!(input_percent(&[0.0; 32]), 0.0);
        assert_eq!(input_percent(&[1.0; 32]), 100.0);
        assert!(input_percent(&[0.1; 32]) > input_percent(&[0.01; 32]));
        assert_eq!(input_percent(&[f32::NAN]), 0.0);
    }
}
