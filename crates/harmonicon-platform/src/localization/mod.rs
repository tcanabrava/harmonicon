// SPDX-License-Identifier: MIT

//! UI localization, built on [Fluent].
//!
//! Translations live under `assets/locales/<lang>/` as Fluent files: one
//! `main.ftl.ron` bundle per locale listing the `.ftl` resources it pulls in
//! (see `assets/locales/en-US/`). At startup each locale's bundle is loaded
//! by an explicit path — [`LOCALES`], not a directory scan — since the wasm
//! build's HTTP asset reader can't enumerate a directory
//! (`bevy_asset::io::wasm::HttpWasmAssetReader` — `Reading directories is
//! not supported`); once every bundle is ready the [`Localization`] resource
//! is built by negotiating the OS UI language ([`SelectedLanguage`], taken
//! from the system locale at startup) against the loaded locales, falling
//! back to [`DEFAULT_LANGUAGE`] so the UI never shows an untranslated key.
//! The language is not persisted — to change it, change the system locale.
//!
//! The asset layer underneath — the `.ftl` and `.ftl.ron` [`AssetLoader`]s
//! and the [`LocaleBundle`] they produce — lives in [`ftl`]. Lesson and song
//! packs bring their own translations ([`packs`]), consulted after the
//! game's own for each language.
//!
//! Call sites fetch strings through [`LocalizationExt::msg`]:
//!
//! ```no_run
//! # use harmonicon_platform::localization::{Localization, LocalizationExt};
//! # fn example(localization: &Localization) {
//! let label = localization.msg("menu-play");
//! # }
//! ```
//!
//! [Fluent]: https://projectfluent.org/
//! [`AssetLoader`]: bevy::asset::AssetLoader

pub mod ftl;
pub mod packs;

use std::borrow::Borrow;
use std::fmt;

use bevy::prelude::*;
use fluent::FluentArgs;
use fluent_content::{Content, Request};
use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use unic_langid::LanguageIdentifier;

use self::ftl::{FtlResource, FtlResourceLoader, LocaleBundle, LocaleBundleLoader};
use self::packs::PackTranslations;
use crate::content_packs::{ContentPacksChanged, ContentPacksSet};

/// Every shipped locale, matched 1:1 with a folder under `assets/locales/`
/// — a fixed list rather than a directory scan (see the module doc
/// comment for why); `tests::locales_const_matches_the_assets_directory`
/// keeps it honest against what's actually on disk.
const LOCALES: [&str; 3] = ["en-US", "pt-BR", "es-ES"];

/// Fallback language, used when neither the player's choice nor the system locale
/// has a matching bundle. Must always have a folder under `assets/locales/`, so it
/// is the one guaranteed translation. Also the language negotiation default.
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
/// system locale. The value is mirrored onto the [`Locale`] by [`sync_locale`].
#[derive(Resource, Clone, Debug)]
pub struct SelectedLanguage(pub String);

impl Default for SelectedLanguage {
    fn default() -> Self {
        Self(system_language())
    }
}

/// Handles to each [`LOCALES`] entry's in-flight `main.ftl.ron` bundle load,
/// kept around so [`build_localization`] can (re)build [`Localization`]
/// from them on demand.
#[derive(Resource)]
struct LocaleBundles(Vec<Handle<LocaleBundle>>);

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
        app.init_asset::<FtlResource>()
            .register_asset_loader(FtlResourceLoader)
            .init_asset::<LocaleBundle>()
            .register_asset_loader(LocaleBundleLoader)
            .init_resource::<SelectedLanguage>()
            .init_resource::<LocalizationReady>()
            // An empty Localization until the folder loads, so consumers can
            // always take `Res<Localization>` without ordering against the load.
            .init_resource::<Localization>()
            .insert_resource(default_locale())
            .init_resource::<PackTranslations>()
            // Read by `packs::reload_on_packs_changed`; registered here too
            // so an app without `ContentPacksPlugin` (tests, tools) runs.
            .add_message::<ContentPacksChanged>()
            .add_systems(Startup, (load_locales, packs::load_at_startup.after(ContentPacksSet)))
            .add_systems(
                Update,
                (
                    sync_locale,
                    packs::reload_on_packs_changed.after(ContentPacksSet),
                    build_localization,
                )
                    .chain(),
            );
    }
}

/// Parse a BCP-47 tag, falling back to [`DEFAULT_LANGUAGE`] on a malformed value.
fn parse_lang(tag: &str) -> LanguageIdentifier {
    tag.parse().unwrap_or_else(|err| {
        warn!("Invalid language tag {tag:?} ({err}); using {DEFAULT_LANGUAGE}");
        DEFAULT_LANGUAGE.parse().expect("DEFAULT_LANGUAGE must be a valid language tag")
    })
}

/// A [`Locale`] requesting and defaulting to [`DEFAULT_LANGUAGE`]; the requested
/// part is overwritten from [`SelectedLanguage`] by [`sync_locale`].
fn default_locale() -> Locale {
    let default = parse_lang(DEFAULT_LANGUAGE);
    Locale::new(default.clone()).with_default(default)
}

/// Kick off the asynchronous load of every [`LOCALES`] entry's bundle — one
/// explicit `load()` per locale, each of which (via
/// [`LocaleBundleLoader`](ftl::LocaleBundleLoader)) transitively loads its
/// own referenced `.ftl` resource by an equally explicit path, so nothing
/// in this whole chain ever needs to enumerate a directory.
fn load_locales(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handles = LOCALES
        .iter()
        .map(|lang| asset_server.load(format!("locales/{lang}/main.ftl.ron")))
        .collect();
    commands.insert_resource(LocaleBundles(handles));
}

/// Mirror the persisted [`SelectedLanguage`] onto the [`Locale`].
/// Marking `Locale` changed is what triggers [`build_localization`] to rebuild.
fn sync_locale(selected: Res<SelectedLanguage>, mut locale: ResMut<Locale>) {
    if selected.is_changed() {
        let requested = parse_lang(&selected.0);
        if requested != locale.requested {
            locale.requested = requested;
        }
    }
}

/// Build [`Localization`] once every bundle has finished loading, and
/// rebuild it whenever the requested [`Locale`] or the packs' translations
/// change.
fn build_localization(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    bundles: Res<Assets<LocaleBundle>>,
    locale: Res<Locale>,
    handles: Option<Res<LocaleBundles>>,
    pack_translations: Res<PackTranslations>,
    mut ready: ResMut<LocalizationReady>,
) {
    // Built and nothing changed: the common case every frame after startup,
    // so answer it before asking the asset server.
    if ready.0 && !locale.is_changed() && !pack_translations.is_changed() {
        return;
    }
    let Some(handles) = handles else { return };
    // Wait for every bundle *and* the `.ftl` resource each references.
    if !handles.0.iter().all(|handle| asset_server.is_loaded_with_dependencies(handle)) {
        return;
    }
    ready.0 = true;
    commands.insert_resource(build_from_bundles(
        &handles.0,
        &bundles,
        &pack_translations.0,
        &locale,
    ));
}

/// Assembles [`Localization`] from every loaded locale bundle, inserted in
/// the [`Locale`]'s own fallback-chain order (most-preferred first —
/// [`Localization`]'s own `content` lookup returns the first bundle with a
/// matching key, so insertion order is what makes the fallback actually
/// take effect).
///
/// For each language the game's bundle goes before the packs', so a pack
/// can add strings but not replace the game's. The languages negotiated
/// over include the packs' own: a pack translated into a language the game
/// isn't still shows its lessons in it, around a UI that falls back.
///
/// Works from the explicit per-locale handles [`load_locales`] collected
/// rather than a `Handle<LoadedFolder>`, since this module deliberately
/// never enumerates a directory — see the module doc comment.
fn build_from_bundles(
    handles: &[Handle<LocaleBundle>],
    bundles: &Assets<LocaleBundle>,
    pack_bundles: &[LocaleBundle],
    locale: &Locale,
) -> Localization {
    let entries: Vec<(LanguageIdentifier, &Handle<LocaleBundle>, &LocaleBundle)> = handles
        .iter()
        .filter_map(|handle| {
            let asset = bundles.get(handle)?;
            Some((asset.locale().clone(), handle, asset))
        })
        .collect();
    let mut languages: Vec<LanguageIdentifier> =
        entries.iter().map(|(l, _, _)| l.clone()).collect();
    for bundle in pack_bundles {
        if !languages.contains(bundle.locale()) {
            languages.push(bundle.locale().clone());
        }
    }
    let fallback = locale.fallback_chain(languages.iter());

    let mut localization = Localization::new();
    for lang in fallback {
        if let Some((_, handle, asset)) = entries.iter().find(|(l, _, _)| l == lang) {
            localization.insert(Some(handle), asset);
        }
        if let Some(pack) = pack_bundles.iter().find(|b| b.locale() == lang) {
            localization.insert(None, pack);
        }
    }
    localization
}

// ── Locale negotiation ────────────────────────────────────────────────────────

/// Which language the UI should be in, and which to fall back to.
///
/// Held as a resource and kept in step with [`SelectedLanguage`] by
/// [`sync_locale`]; [`build_localization`] watches it for changes.
#[derive(Resource, Clone, Debug, Default)]
pub struct Locale {
    /// The language the player's system asked for.
    pub requested: LanguageIdentifier,
    /// Where to land when `requested` has no bundle — [`DEFAULT_LANGUAGE`]
    /// in practice, so the UI always has *some* translation.
    pub default: Option<LanguageIdentifier>,
}

impl Locale {
    pub fn new(requested: LanguageIdentifier) -> Self {
        Self { requested, default: None }
    }

    pub fn with_default(mut self, default: LanguageIdentifier) -> Self {
        self.default = Some(default);
        self
    }

    /// The available locales that can serve [`Self::requested`], best first,
    /// with [`Self::default`] appended as the last resort.
    ///
    /// `Filtering` is Fluent's broadest strategy: it keeps every acceptable
    /// match rather than just the single best one, which is what gives
    /// [`Localization`] a real chain to walk when a key is missing from the
    /// preferred locale but present in another.
    pub fn fallback_chain<'a, I>(&'a self, available: I) -> Vec<&'a LanguageIdentifier>
    where
        I: Iterator<Item = &'a LanguageIdentifier>,
    {
        let available = &available.collect::<Vec<_>>();
        let default = self.default.as_ref();
        let supported = negotiate_languages(
            std::slice::from_ref(&self.requested),
            available,
            default.as_ref(),
            NegotiationStrategy::Filtering,
        );
        supported.into_iter().copied().collect()
    }
}

// ── The negotiated bundle set ─────────────────────────────────────────────────

/// The loaded locale bundles that can answer a message lookup, in fallback
/// order: most-preferred first, [`DEFAULT_LANGUAGE`] last.
///
/// Reach for strings through [`LocalizationExt`] rather than [`Content`]
/// directly — `msg`/`msg_args` add the missing-key fallback and the
/// bidi-mark cleanup [`strip_bidi_isolates`] describes.
///
/// [`Default`] is an empty set, which answers every lookup with `None`: it's
/// what the plugin inserts before the locale bundles finish loading (so a
/// system can take `Res<Localization>` without ordering against the load),
/// and what tests construct to assert on the key-echoing fallback.
#[derive(Resource, Default)]
pub struct Localization(Vec<(Option<Handle<LocaleBundle>>, LocaleBundle)>);

impl Localization {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every locale in this set, in the order lookups consult them.
    pub fn locales(&self) -> impl Iterator<Item = &LanguageIdentifier> {
        self.0.iter().map(|(_, bundle)| bundle.locale())
    }

    /// Appends `bundle` to the end of the fallback order.
    ///
    /// The `handle` is kept alongside it purely to hold the asset alive, so
    /// `Assets<LocaleBundle>` can't drop the bundle out from under a
    /// long-lived `Localization`; a pack's bundle is not an asset and has
    /// none. Each language appears at most twice, the game's bundle before
    /// the packs' — [`build_from_bundles`] walks a fallback chain of
    /// distinct languages.
    fn insert(&mut self, handle: Option<&Handle<LocaleBundle>>, bundle: &LocaleBundle) {
        self.0.push((handle.cloned(), bundle.clone()));
    }
}

impl<'a, T, U> Content<'a, T, U> for Localization
where
    T: Copy + Into<Request<'a, U>>,
    U: Borrow<FluentArgs<'a>>,
{
    /// The first bundle in fallback order that defines `request`'s key.
    fn content(&self, request: T) -> Option<String> {
        self.0.iter().find_map(|(_, bundle)| (**bundle).content(request))
    }
}

impl fmt::Debug for Localization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Localization").field(&self.locales().collect::<Vec<_>>()).finish()
    }
}

// ── Localized-string newtype ──────────────────────────────────────────────────

/// A string that was produced by the localization system.
///
/// Cannot be constructed from a raw `&str` or `String`; only from
/// [`LocalizationExt::msg`] or [`LocalizationExt::msg_args`]. This makes it a
/// compile-time error to pass a raw string literal where a user-visible label is
/// expected, as long as your own UI helpers accept `LocalizedStr` rather than
/// `&str`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalizedStr(String);

impl LocalizedStr {
    fn new(s: String) -> Self {
        Self(s)
    }
}

impl std::ops::Deref for LocalizedStr {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LocalizedStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<LocalizedStr> for String {
    fn from(s: LocalizedStr) -> Self {
        s.0
    }
}

// ── Ergonomic lookup trait ────────────────────────────────────────────────────

/// Ergonomic string lookup on [`Localization`].
/// The locale key for one variant of a UI-facing enum, from the English
/// label the enum already uses as its identity — `("progression", "Quick
/// Change")` → `progression-quick-change`. A page shows `loc.msg(&key)` in
/// a combobox and maps the selection back by index (`ComboboxSelect::
/// index`), so the enum's `label()`/`from_label()` stay the stable id
/// (charts and manifests store it) while the player sees their language.
/// `tests::enum_label_keys_are_kebab_case` pins the shape; each page's own
/// test pins that every variant's key exists in the locale file.
pub fn enum_label_key(prefix: &str, label: &str) -> String {
    let mut key = String::with_capacity(prefix.len() + 1 + label.len());
    key.push_str(prefix);
    key.push('-');
    let mut last_dash = false;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            key.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            key.push('-');
            last_dash = true;
        }
    }
    key.trim_end_matches('-').to_string()
}

pub trait LocalizationExt {
    /// The localized string for `key`, or the key itself when it is missing — so
    /// a forgotten or not-yet-loaded translation is visible rather than blank.
    fn msg(&self, key: &str) -> LocalizedStr;

    /// Like [`msg`] but resolves Fluent's own `{$name}` variables in the FTL
    /// template against the supplied values.
    ///
    /// FTL example:
    /// ```text
    /// practice-done = Done — {$hits}/{$total} notes · {$score} pts
    /// ```
    /// Call site:
    /// ```no_run
    /// # use harmonicon_platform::localization::{Localization, LocalizationExt};
    /// # fn example(loc: &Localization, hits: u32, total: u32, score: u32) {
    /// loc.msg_args(
    ///     "practice-done",
    ///     &[
    ///         ("hits", hits.to_string()),
    ///         ("total", total.to_string()),
    ///         ("score", score.to_string()),
    ///     ],
    /// );
    /// # }
    /// ```
    ///
    /// [`msg`]: LocalizationExt::msg
    fn msg_args(&self, key: &str, args: &[(&str, String)]) -> LocalizedStr;
}

impl LocalizationExt for Localization {
    fn msg(&self, key: &str) -> LocalizedStr {
        LocalizedStr::new(self.content(key).unwrap_or_else(|| {
            warn!("Missing translation for {key:?}");
            key.to_string()
        }))
    }

    fn msg_args(&self, key: &str, args: &[(&str, String)]) -> LocalizedStr {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, value.as_str());
        }
        let request = Request::new(key).args(&fluent_args);
        let s = self.content(request).unwrap_or_else(|| {
            warn!("Missing translation for {key:?}");
            key.to_string()
        });
        LocalizedStr::new(strip_bidi_isolates(s))
    }
}

/// Strips Fluent's bidi-isolation marks (FSI/PDI, U+2068/U+2069), which
/// `format_pattern` wraps around every interpolated argument by default so
/// RTL/LTR content can't bleed into each other — meant for prose mixing
/// scripts, not our short, single-language UI labels. Stripped after the
/// fact rather than turned off at the bundle, so a locale that *does* mix
/// scripts still gets correct isolation from Fluent itself and only the
/// rendered label loses the invisible marks.
fn strip_bidi_isolates(s: String) -> String {
    const MARKS: [char; 2] = ['\u{2068}', '\u{2069}'];
    if s.contains(MARKS) { s.replace(MARKS, "") } else { s }
}

#[cfg(test)]
mod tests {
    #[test]
    fn enum_label_keys_are_kebab_case() {
        use super::enum_label_key;
        assert_eq!(enum_label_key("progression", "Quick Change"), "progression-quick-change");
        assert_eq!(enum_label_key("scale", "1st Position"), "scale-1st-position");
        assert_eq!(enum_label_key("position", "12th"), "position-12th");
        assert_eq!(enum_label_key("genre", "Blues"), "genre-blues");
    }

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

    #[test]
    fn packs_add_strings_but_never_replace_the_games() {
        use super::{Locale, LocaleBundle, LocalizationExt, build_from_bundles};
        use bevy::asset::Assets;
        use unic_langid::LanguageIdentifier;

        let lang = |tag: &str| tag.parse::<LanguageIdentifier>().unwrap();
        let mut assets = Assets::<LocaleBundle>::default();
        let game = assets.add(LocaleBundle::from_sources(
            lang("en-US"),
            ["menu-play = Play\nshared = game\n".to_string()],
        ));
        let packs = [
            LocaleBundle::from_sources(
                lang("en-US"),
                ["shared = pack\nlesson-x = Lesson\n".to_string()],
            ),
            LocaleBundle::from_sources(lang("de-DE"), ["lesson-x = Lektion\n".to_string()]),
        ];

        // German isn't one of the game's languages, but a pack speaks it.
        let locale = Locale::new(lang("de-DE")).with_default(lang("en-US"));
        let loc = build_from_bundles(std::slice::from_ref(&game), &assets, &packs, &locale);
        assert_eq!(&*loc.msg("lesson-x"), "Lektion");
        assert_eq!(&*loc.msg("menu-play"), "Play");
        assert_eq!(&*loc.msg("shared"), "game");

        let english = Locale::new(lang("en-US")).with_default(lang("en-US"));
        let loc = build_from_bundles(&[game], &assets, &packs, &english);
        assert_eq!(&*loc.msg("lesson-x"), "Lesson");
    }

    #[test]
    fn strip_bidi_isolates_removes_fsi_pdi_marks() {
        let wrapped = "Score: \u{2068}100\u{2069} pts".to_string();
        assert_eq!(super::strip_bidi_isolates(wrapped), "Score: 100 pts");
    }

    /// The whole point of the fallback chain: the requested locale comes
    /// first, [`super::DEFAULT_LANGUAGE`] last, and an unrelated locale
    /// never gets consulted.
    #[test]
    fn fallback_chain_prefers_requested_then_default() {
        let locale = super::default_locale();
        let locale = super::Locale { requested: super::parse_lang("pt-BR"), ..locale };
        let available: Vec<_> =
            ["en-US", "pt-BR", "es-ES"].iter().map(|tag| super::parse_lang(tag)).collect();

        let chain: Vec<String> = locale
            .fallback_chain(available.iter())
            .into_iter()
            .map(|lang| lang.to_string())
            .collect();

        assert_eq!(chain, vec!["pt-BR".to_string(), "en-US".to_string()]);
    }

    /// Every locale must define exactly the same message keys as the default
    /// `en-US` locale, so no screen falls back to a raw key in another language.
    #[test]
    fn locales_define_the_same_keys() {
        let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/locales");
        let reference = std::fs::read_to_string(locales.join("en-US/main/ui.ftl"))
            .expect("en-US ui.ftl must exist");
        let reference_keys = message_keys(&reference);
        assert!(!reference_keys.is_empty(), "en-US ui.ftl defined no message keys");

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

    /// A key whose text uses a Fluent variable (`{$note}`) must be resolved
    /// with `msg_args`, never plain `msg`. Fluent doesn't fail the lookup —
    /// it renders the placeholder literally and logs `Unknown variable`, so
    /// the mistake compiles, passes every other test, and is often visually
    /// invisible too: a label spawned that way is usually overwritten by its
    /// update system a frame later, leaving a log line as the only trace.
    ///
    /// Scans every workspace source file for `.msg("literal-key")`. A key
    /// chosen at runtime (a `match` returning a `&str`) can't be checked
    /// statically and isn't; the literal case is the one that recurs.
    #[test]
    fn keys_with_variables_are_never_resolved_without_arguments() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ftl = std::fs::read_to_string(root.join("assets/locales/en-US/main/ui.ftl"))
            .expect("en-US ui.ftl must exist");
        let takes_args: std::collections::HashSet<&str> = ftl
            .lines()
            .filter(|line| line.contains("{$"))
            .filter_map(|line| line.split_once(" = ").map(|(key, _)| key.trim()))
            .collect();
        assert!(!takes_args.is_empty(), "expected some keys to use variables");

        let mut dirs = vec![root.join("src")];
        for entry in std::fs::read_dir(root.join("crates")).expect("crates dir") {
            dirs.push(entry.unwrap().path().join("src"));
        }
        let mut offenders = Vec::new();
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                if path.extension().and_then(|e| e.to_str()) != Some("rs")
                    || path.file_name().and_then(|n| n.to_str()) == Some("tests.rs")
                {
                    continue;
                }
                let source = std::fs::read_to_string(&path).unwrap();
                for (number, line) in source.lines().enumerate() {
                    for (index, _) in line.match_indices(".msg(\"") {
                        let rest = &line[index + ".msg(\"".len()..];
                        let Some(key) = rest.split('"').next() else {
                            continue;
                        };
                        if takes_args.contains(key) {
                            offenders.push(format!("{}:{}: {key}", path.display(), number + 1));
                        }
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these keys use a Fluent variable but are resolved with `msg` \
             instead of `msg_args`, so the variable renders unresolved:\n{}",
            offenders.join("\n")
        );
    }

    /// [`super::LOCALES`] is a fixed list (loaded by explicit path rather
    /// than a directory scan — see the module doc comment for why), so
    /// nothing else catches it silently drifting from what's actually
    /// shipped under `assets/locales/` the way a real directory scan would.
    #[test]
    fn locales_const_matches_the_assets_directory() {
        let locales_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/locales");
        let mut on_disk: Vec<String> = std::fs::read_dir(&locales_dir)
            .expect("locales dir must exist")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();

        let mut expected: Vec<String> = super::LOCALES.iter().map(|s| s.to_string()).collect();
        expected.sort();

        assert_eq!(on_disk, expected, "localization::LOCALES is out of sync with assets/locales/");
    }
}
