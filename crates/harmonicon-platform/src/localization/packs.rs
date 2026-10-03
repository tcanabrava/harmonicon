// SPDX-License-Identifier: MIT

//! Translations shipped inside lesson and song packs.
//!
//! A pack cannot wait for a game release to add a string, so it carries its
//! own: every `locales/<lang>.ftl` file anywhere in the pack — at its root
//! for names shared across it (units, tracks), beside each lesson for that
//! lesson's text. The rule is purely about file names, so this module needs
//! to know nothing about what a lesson is.
//!
//! Each language gets one [`LocaleBundle`] merging every pack's files for
//! it, in configured pack order. [`super::build_localization`] consults
//! them *after* the game's own bundle for the same language, so a pack can
//! add strings but never replace one of the game's.

#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use unic_langid::LanguageIdentifier;

use super::ftl::LocaleBundle;
use crate::content_packs::{ContentPacks, ContentPacksChanged};

/// One bundle per language found in any usable pack, sorted by language tag.
#[derive(Resource, Default, Clone)]
pub struct PackTranslations(pub Vec<LocaleBundle>);

/// Every `locales/<lang>.ftl` under `root`, skipping hidden directories (a
/// checkout's `.git`), sorted by path so the result doesn't depend on the
/// order the filesystem lists entries in.
#[cfg(not(target_arch = "wasm32"))]
fn translation_files(root: &Path) -> Vec<(LanguageIdentifier, PathBuf)> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir && !entry.file_name().to_string_lossy().starts_with('.') {
                stack.push(path);
            } else if !is_dir
                && path.extension().is_some_and(|e| e == "ftl")
                && dir.file_name().is_some_and(|d| d == "locales")
            {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                match stem.parse::<LanguageIdentifier>() {
                    Ok(lang) => found.push((lang, path)),
                    Err(e) => {
                        warn!("Skipping {}: {stem:?} is not a language tag ({e})", path.display())
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}

/// Builds one bundle per language from every translation file under `roots`,
/// in `roots` order. Unreadable files are logged and skipped.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_pack_translations<'a>(roots: impl IntoIterator<Item = &'a Path>) -> Vec<LocaleBundle> {
    let mut by_lang: BTreeMap<String, (LanguageIdentifier, Vec<String>)> = BTreeMap::new();
    for root in roots {
        for (lang, path) in translation_files(root) {
            match std::fs::read_to_string(&path) {
                Ok(source) => by_lang
                    .entry(lang.to_string())
                    .or_insert_with(|| (lang, Vec::new()))
                    .1
                    .push(source),
                Err(e) => warn!("Could not read {}: {e}", path.display()),
            }
        }
    }
    by_lang.into_values().map(|(lang, sources)| LocaleBundle::from_sources(lang, sources)).collect()
}

#[cfg(target_arch = "wasm32")]
pub fn load_pack_translations<'a>(_roots: impl IntoIterator<Item = &'a Path>) -> Vec<LocaleBundle> {
    Vec::new()
}

fn usable_roots(packs: &ContentPacks) -> impl Iterator<Item = &Path> {
    packs
        .0
        .iter()
        .filter(|e| matches!(e.status, crate::content_packs::PackStatus::Ready { .. }))
        .map(|e| e.root.as_path())
}

/// Reads the packs' translations once [`ContentPacks`] is known. Replacing
/// the resource marks it changed, which is what makes
/// [`super::build_localization`] rebuild.
pub(super) fn load_at_startup(
    packs: Option<Res<ContentPacks>>,
    mut translations: ResMut<PackTranslations>,
) {
    if let Some(packs) = packs {
        *translations = PackTranslations(load_pack_translations(usable_roots(&packs)));
    }
}

/// Reads them again whenever a pack is installed, updated or removed.
pub(super) fn reload_on_packs_changed(
    mut changed: MessageReader<ContentPacksChanged>,
    packs: Option<Res<ContentPacks>>,
    translations: ResMut<PackTranslations>,
) {
    if changed.read().count() > 0 {
        load_at_startup(packs, translations);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use fluent_content::Content;

    fn write(root: &Path, rel: &str, contents: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }

    fn lookup(bundles: &[LocaleBundle], lang: &str, key: &str) -> Option<String> {
        bundles.iter().find(|b| *b.locale() == lang).and_then(|b| b.content(key))
    }

    #[test]
    fn collects_every_locales_folder_by_language() {
        let pack = tempfile::tempdir().unwrap();
        write(pack.path(), "locales/en-US.ftl", "unit-basics = Basics\n");
        write(pack.path(), "01_basics/01_first/locales/en-US.ftl", "first-title = First\n");
        write(pack.path(), "01_basics/01_first/locales/pt-BR.ftl", "first-title = Primeira\n");

        let bundles = load_pack_translations([pack.path()]);
        let langs: Vec<String> = bundles.iter().map(|b| b.locale().to_string()).collect();
        assert_eq!(langs, ["en-US", "pt-BR"]);
        assert_eq!(lookup(&bundles, "en-US", "unit-basics").as_deref(), Some("Basics"));
        assert_eq!(lookup(&bundles, "en-US", "first-title").as_deref(), Some("First"));
        assert_eq!(lookup(&bundles, "pt-BR", "first-title").as_deref(), Some("Primeira"));
    }

    #[test]
    fn ignores_ftl_files_outside_a_locales_folder_and_in_hidden_ones() {
        let pack = tempfile::tempdir().unwrap();
        write(pack.path(), "notes/en-US.ftl", "stray = no\n");
        write(pack.path(), ".git/locales/en-US.ftl", "hidden = no\n");
        write(pack.path(), "locales/not a tag!.ftl", "bad = no\n");
        assert!(load_pack_translations([pack.path()]).is_empty());
    }

    #[test]
    fn an_earlier_pack_keeps_a_key_both_define() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        write(first.path(), "locales/en-US.ftl", "shared = first\n");
        write(second.path(), "locales/en-US.ftl", "shared = second\nonly-second = yes\n");
        let bundles = load_pack_translations([first.path(), second.path()]);
        assert_eq!(lookup(&bundles, "en-US", "shared").as_deref(), Some("first"));
        assert_eq!(lookup(&bundles, "en-US", "only-second").as_deref(), Some("yes"));
    }
}
