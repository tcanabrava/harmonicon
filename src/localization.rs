// SPDX-License-Identifier: MIT

//! UI localization, built on [`bevy_fluent`].
//!
//! Translations live under `assets/locales/<lang>/` as [Fluent] files: one
//! `main.ftl.ron` bundle per locale listing the `.ftl` resources it pulls in
//! (see `assets/locales/en-US/`). At startup the whole `locales` folder is loaded
//! asynchronously; once ready the [`Localization`] resource is built by
//! negotiating the OS UI language ([`SelectedLanguage`], taken from the system
//! locale at startup) against the available locales, falling back to
//! [`DEFAULT_LANGUAGE`] so the UI never shows an untranslated key. The language
//! is not persisted — to change it, change the system locale.
//!
//! Call sites fetch strings through [`LocalizationExt::msg`]:
//!
//! ```ignore
//! use crate::localization::LocalizationExt;
//! let label = localization.msg("menu-play");
//! ```
//!
//! [Fluent]: https://projectfluent.org/

use bevy::asset::LoadedFolder;
use bevy::prelude::*;
use bevy_fluent::prelude::*;
use fluent_content::Content;
use unic_langid::LanguageIdentifier;

/// Fallback language, used when neither the player's choice nor the system locale
/// has a matching bundle. Must always have a folder under `assets/locales/`, so it
/// is the one guaranteed translation. Also the `bevy_fluent` negotiation default.
pub const DEFAULT_LANGUAGE: &str = "en-US";

/// The OS UI language as a BCP-47 tag (e.g. `"pt-BR"`), or [`DEFAULT_LANGUAGE`]
/// when it can't be detected. Used as the initial [`SelectedLanguage`] so the game
/// starts in the player's locale; English remains the fallback for any locale
/// without a translation (via [`Locale`]'s default and [`LocalizationExt::msg`]).
pub fn system_language() -> String {
    match sys_locale::get_locale() {
        Some(locale) => {
            info!("Detected system locale: {locale}");
            locale
        }
        None => {
            info!("No system locale detected; using {DEFAULT_LANGUAGE}");
            DEFAULT_LANGUAGE.to_string()
        }
    }
}

/// The active UI language as a BCP-47 tag (e.g. `"en-US"`, `"pt-BR"`).
///
/// Set once at startup from the [`system_language`] and not persisted: the game
/// always follows the OS locale, so changing the language means changing the
/// system locale. The value is mirrored onto the `bevy_fluent` [`Locale`] by
/// [`sync_locale`].
#[derive(Resource, Clone, Debug)]
pub struct SelectedLanguage(pub String);

impl Default for SelectedLanguage {
    fn default() -> Self {
        Self(system_language())
    }
}

/// Handle to the in-flight `locales` folder load, kept around so
/// [`build_localization`] can (re)build [`Localization`] from it on demand.
#[derive(Resource)]
struct LocaleFolder(Handle<LoadedFolder>);

/// Set once the first [`Localization`] has been built, so later frames only
/// rebuild when the [`Locale`] actually changes. Also drives [`localization_ready`]
/// so screens with translated text don't open before strings are available.
#[derive(Resource, Default)]
pub struct LocalizationReady(bool);

/// Run condition: `true` once the locale folder has loaded and the initial
/// [`Localization`] has been built. Gate the first transition into any
/// translated screen on this so it never flashes raw message keys.
pub fn localization_ready(ready: Res<LocalizationReady>) -> bool {
    ready.0
}

pub struct LocalizationPlugin;

impl Plugin for LocalizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FluentPlugin)
            .init_resource::<SelectedLanguage>()
            .init_resource::<LocalizationReady>()
            // An empty Localization until the folder loads, so consumers can
            // always take `Res<Localization>` without ordering against the load.
            .init_resource::<Localization>()
            .insert_resource(default_locale())
            .add_systems(Startup, load_locales)
            .add_systems(Update, (sync_locale, build_localization).chain());
    }
}

/// Parse a BCP-47 tag, falling back to [`DEFAULT_LANGUAGE`] on a malformed value.
fn parse_lang(tag: &str) -> LanguageIdentifier {
    tag.parse().unwrap_or_else(|err| {
        warn!("Invalid language tag {tag:?} ({err}); using {DEFAULT_LANGUAGE}");
        DEFAULT_LANGUAGE
            .parse()
            .expect("DEFAULT_LANGUAGE must be a valid language tag")
    })
}

/// A [`Locale`] requesting and defaulting to [`DEFAULT_LANGUAGE`]; the requested
/// part is overwritten from [`SelectedLanguage`] by [`sync_locale`].
fn default_locale() -> Locale {
    let default = parse_lang(DEFAULT_LANGUAGE);
    Locale::new(default.clone()).with_default(default)
}

/// Kick off the asynchronous load of every file under `assets/locales/`.
fn load_locales(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(LocaleFolder(asset_server.load_folder("locales")));
}

/// Mirror the persisted [`SelectedLanguage`] onto the `bevy_fluent` [`Locale`].
/// Marking `Locale` changed is what triggers [`build_localization`] to rebuild.
fn sync_locale(selected: Res<SelectedLanguage>, mut locale: ResMut<Locale>) {
    if selected.is_changed() {
        let requested = parse_lang(&selected.0);
        if requested != locale.requested {
            locale.requested = requested;
        }
    }
}

/// Build [`Localization`] once the folder has finished loading, and rebuild it
/// whenever the requested [`Locale`] changes.
fn build_localization(
    mut commands: Commands,
    builder: LocalizationBuilder,
    asset_server: Res<AssetServer>,
    locale: Res<Locale>,
    folder: Option<Res<LocaleFolder>>,
    mut ready: ResMut<LocalizationReady>,
) {
    let Some(folder) = folder else { return };
    // Wait for the bundles *and* the `.ftl` resources they reference.
    if !asset_server.is_loaded_with_dependencies(&folder.0) {
        return;
    }
    if ready.0 && !locale.is_changed() {
        return;
    }
    ready.0 = true;
    commands.insert_resource(builder.build(&folder.0));
}

/// Ergonomic string lookup on [`Localization`].
pub trait LocalizationExt {
    /// The localized string for `key`, or the key itself when it is missing — so
    /// a forgotten or not-yet-loaded translation is visible rather than blank.
    fn msg(&self, key: &str) -> String;
}

impl LocalizationExt for Localization {
    fn msg(&self, key: &str) -> String {
        self.content(key).unwrap_or_else(|| {
            warn!("Missing translation for {key:?}");
            key.to_string()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Message identifiers defined in a `.ftl` file (lines of the form
    /// `key = value`), ignoring comments, blank lines and continuations.
    fn message_keys(ftl: &str) -> BTreeSet<String> {
        ftl.lines()
            .filter_map(|line| {
                let line = line.trim_start();
                if line.starts_with('#') {
                    return None;
                }
                let (key, _) = line.split_once('=')?;
                let key = key.trim();
                // A bare identifier before '=' (no spaces) is a message id; an
                // indented continuation or attribute is not.
                if key.is_empty() || key.contains(char::is_whitespace) {
                    None
                } else {
                    Some(key.to_string())
                }
            })
            .collect()
    }

    /// Every locale must define exactly the same message keys as the default
    /// `en-US` locale, so no screen falls back to a raw key in another language.
    #[test]
    fn locales_define_the_same_keys() {
        let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/locales");
        let reference = std::fs::read_to_string(locales.join("en-US/main/ui.ftl"))
            .expect("en-US ui.ftl must exist");
        let reference_keys = message_keys(&reference);
        assert!(
            !reference_keys.is_empty(),
            "en-US ui.ftl defined no message keys"
        );

        for entry in std::fs::read_dir(&locales).expect("locales dir must exist") {
            let dir = entry.unwrap().path();
            let ftl = dir.join("main/ui.ftl");
            if !ftl.exists() {
                continue;
            }
            let keys = message_keys(&std::fs::read_to_string(&ftl).unwrap());
            assert_eq!(
                keys,
                reference_keys,
                "locale {:?} keys diverge from en-US",
                dir.file_name().unwrap(),
            );
        }
    }
}
