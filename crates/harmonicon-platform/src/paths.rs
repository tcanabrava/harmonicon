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

/// Reads a persisted JSON file over `T`'s defaults, for `settings.json` and
/// `profile.json`. A missing file is the defaults.
///
/// **One bad field costs that field, not the file.** Both files are written
/// back soon after they're read (settings a moment after startup, the
/// profile at the latest on exit), so falling back to the defaults wholesale
/// would erase everything the moment one field stopped parsing — which a
/// downgrade does (an older build meeting a value a newer one wrote), as
/// does a hand-edit typo. What can't be read is left out
/// ([`json_over_defaults`]) and the original is copied to
/// `<name>.unreadable` first, so nothing is lost even then.
pub fn load_json_over_defaults<T>(path: &std::path::Path) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return T::default(),
        Err(err) => {
            bevy::log::warn!("Could not read {} ({err}); using defaults", path.display());
            return T::default();
        }
    };
    let (value, dropped) = json_over_defaults::<T>(&text);
    if !dropped.is_empty() {
        let mut backup = path.as_os_str().to_owned();
        backup.push(".unreadable");
        match std::fs::copy(path, &backup) {
            Ok(_) => bevy::log::warn!(
                "Could not read {} in {}; using defaults for those. The original was kept as {}",
                dropped.join(", "),
                path.display(),
                std::path::Path::new(&backup).display()
            ),
            Err(err) => bevy::log::warn!(
                "Could not read {} in {}; using defaults for those (and could not keep a copy: \
                 {err})",
                dropped.join(", "),
                path.display()
            ),
        }
    }
    value
}

/// `text` over `T`'s defaults, and the top-level fields that had to be left
/// out, sorted. The whole text is tried first; only if that fails is it
/// layered in one field at a time, keeping each that still extracts. Text
/// that isn't a JSON object at all yields the defaults and
/// `"(the whole file)"`.
pub fn json_over_defaults<T>(text: &str) -> (T, Vec<String>)
where
    T: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    use figment::Figment;
    use figment::providers::{Format, Json, Serialized};
    let defaults = || Figment::from(Serialized::defaults(T::default()));
    if let Ok(value) = defaults().merge(Json::string(text)).extract() {
        return (value, Vec::new());
    }
    let Ok(serde_json::Value::Object(fields)) = serde_json::from_str(text) else {
        return (T::default(), vec!["(the whole file)".to_string()]);
    };
    let mut figment = defaults();
    let mut dropped = Vec::new();
    for (key, value) in fields {
        let one = serde_json::Value::Object([(key.clone(), value)].into_iter().collect());
        let candidate = figment.clone().merge(Json::string(&one.to_string()));
        if candidate.extract::<T>().is_ok() {
            figment = candidate;
        } else {
            dropped.push(key);
        }
    }
    dropped.sort();
    (figment.extract().unwrap_or_default(), dropped)
}
