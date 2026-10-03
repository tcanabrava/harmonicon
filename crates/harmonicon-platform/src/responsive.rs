// SPDX-License-Identifier: MIT

//! Shared "is the window small enough to need a compact layout" signal —
//! [`CompactLayout`], read by gameplay 2D/3D and the Song Editor to decide
//! which of two layouts to spawn. No live-reflow: each screen reads this
//! once when it's entered (`OnEnter(AppState::Playing)` /
//! `OnEnter(AppState::SongEditor2)`), not continuously, so resizing the
//! window mid-session doesn't retroactively change an already-spawned
//! screen's layout — deliberately, to avoid needing despawn/respawn logic
//! for a scenario (resizing *during* a song or edit session) that matters
//! far less than "the screen you land on already fits."

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Below this effective width (logical px, already divided by [`UiScale`] —
/// see [`update_compact_layout`]), a screen should use its compact layout.
/// Matches the Song Editor's own measured rigid floor (its 3-column meta
/// form needs ~1122px before clipping), the tightest constraint found
/// across gameplay and the editor — one shared threshold keeps "compact"
/// meaning the same thing everywhere.
const COMPACT_BREAKPOINT_PX: f32 = 900.0;

/// Whether the current window is narrow enough that gameplay/the Song
/// Editor should spawn their compact layout instead of the full one.
#[derive(Resource, Default)]
pub struct CompactLayout(pub bool);

/// Pure predicate behind [`CompactLayout`] — effective width (already
/// divided by [`UiScale`]) below the breakpoint means compact.
pub fn is_compact(effective_width_px: f32) -> bool {
    effective_width_px < COMPACT_BREAKPOINT_PX
}

/// Keeps [`CompactLayout`] in sync with the primary window's width, divided
/// by [`UiScale`] the same way `song_editor::interaction`'s own width reads
/// already do — a `Val::Px` measurement is scaled by `UiScale` at render
/// time, so the *effective* width a fixed-pixel layout has to fit in is the
/// window's own width divided by that scale, not the raw window width.
/// Only writes `CompactLayout` when the value actually changes — a `ResMut`
/// deref marks the resource changed unconditionally, so writing every
/// frame regardless of value would make `resource_changed::<CompactLayout>()`
/// fire every frame too.
fn update_compact_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut compact: ResMut<CompactLayout>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let effective_width = window.width() / ui_scale.0;
    let compact_now = is_compact(effective_width);
    if compact.0 != compact_now {
        compact.0 = compact_now;
    }
}

pub struct ResponsivePlugin;

impl Plugin for ResponsivePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CompactLayout>().add_systems(Update, update_compact_layout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_window_is_not_compact() {
        assert!(!is_compact(1280.0));
    }

    #[test]
    fn narrow_window_is_compact() {
        assert!(is_compact(400.0));
    }

    #[test]
    fn exactly_at_the_breakpoint_is_not_compact() {
        assert!(!is_compact(COMPACT_BREAKPOINT_PX));
    }

    #[test]
    fn just_below_the_breakpoint_is_compact() {
        assert!(is_compact(COMPACT_BREAKPOINT_PX - 1.0));
    }
}
