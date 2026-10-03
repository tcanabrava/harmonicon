// SPDX-License-Identifier: MIT

use super::*;

#[test]
fn zoom_label_shows_the_rounded_percent() {
    let loc = Localization::default();
    assert_eq!(zoom_label_text(&loc, 1.0), "options-zoom-label");
}

#[test]
fn zoom_fraction_spans_min_to_max() {
    use harmonicon_ui::dialogs::ui_scale::{MAX_SCALE, MIN_SCALE};
    assert_eq!(zoom_fraction(MIN_SCALE), 0.0);
    assert_eq!(zoom_fraction(MAX_SCALE), 1.0);
    assert!((zoom_fraction((MIN_SCALE + MAX_SCALE) / 2.0) - 0.5).abs() < 1e-6);
}

#[test]
fn zoom_fraction_clamps_outside_the_range() {
    use harmonicon_ui::dialogs::ui_scale::{MAX_SCALE, MIN_SCALE};
    assert_eq!(zoom_fraction(MIN_SCALE - 5.0), 0.0);
    assert_eq!(zoom_fraction(MAX_SCALE + 5.0), 1.0);
}

#[test]
fn mic_banner_hidden_only_once_connected() {
    assert!(!mic_banner_visible(&MicStatus::Connected { device_name: "Mic".into() }));
    assert!(mic_banner_visible(&MicStatus::Failed { reason: "no device".into() }));
    assert!(mic_banner_visible(&MicStatus::AwaitingPermission));
}

#[test]
fn connected_device_name_is_none_unless_connected() {
    assert_eq!(
        connected_device_name(&MicStatus::Connected { device_name: "USB Mic".into() }),
        Some("USB Mic")
    );
    assert_eq!(connected_device_name(&MicStatus::Failed { reason: "no device".into() }), None);
    assert_eq!(connected_device_name(&MicStatus::AwaitingPermission), None);
}

#[test]
fn mic_banner_text_is_distinct_per_status() {
    assert_eq!(
        mic_banner_key(&MicStatus::Connected { device_name: "Mic".into() }),
        None,
        "a working mic has nothing to say"
    );
    assert_ne!(
        mic_banner_key(&MicStatus::AwaitingPermission),
        mic_banner_key(&MicStatus::Failed { reason: "no device".into() }),
        "awaiting-permission needs its own message, not the generic failure one"
    );
}
