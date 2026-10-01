// SPDX-License-Identifier: MIT

//! Where this game keeps the files it writes.
//!
//! One place, because every persisted file has to agree: `settings.json`
//! (`super::settings`), `profile.json` (`harmonicon-app`'s `profile`), and
//! anything added later.

use std::path::PathBuf;

/// The directory persisted state is written to, or `None` when the platform
/// offers nowhere to write.
///
/// On desktop this is `<config>/harmonicon` via `dirs`.
///
/// **On Android it deliberately isn't.** `dirs::config_dir()` returns `None`
/// there — an app has no XDG config directory, only a sandbox the system
/// hands it — so every save silently no-opped and lesson progress, best
/// scores and options were all lost on exit. The sandbox path comes from
/// `AndroidApp::internal_data_path()`, which the platform provides at
/// startup and Bevy stashes in `ANDROID_APP` (see
/// `crates/harmonicon-android`).
///
/// `None` is still possible on Android — `internal_data_path` is itself an
/// `Option`, and `ANDROID_APP` is unset if something calls this before
/// `android_main` — so callers must keep handling it rather than unwrapping.
#[cfg(not(target_os = "android"))]
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("harmonicon"))
}

#[cfg(target_os = "android")]
pub fn config_dir() -> Option<PathBuf> {
    // No "harmonicon" subdirectory: the path is already private to this
    // application, so nesting by app name would just be noise.
    bevy::android::ANDROID_APP.get()?.internal_data_path()
}

/// `config_dir()` joined with `name`, so a caller states only the filename.
pub fn config_file(name: &str) -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(name))
}

/// Where downloaded content (lesson and song packs) lives: `<data>/harmonicon`
/// on desktop, separate from the config directory because it is large,
/// re-downloadable and nothing a player would back up or edit.
///
/// Android has one private directory for everything, so it is the same as
/// [`config_dir`] there.
#[cfg(not(target_os = "android"))]
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("harmonicon"))
}

#[cfg(target_os = "android")]
pub fn data_dir() -> Option<PathBuf> {
    config_dir()
}

/// The directory installed packs are cloned into, one subdirectory each.
pub fn packs_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("packs"))
}
