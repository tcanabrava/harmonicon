// SPDX-License-Identifier: MIT

//! The Fluent asset layer: a `.ftl` translation file ([`FtlResource`]) and
//! the per-locale [`FluentBundle`] that groups the resources one
//! `main.ftl.ron` lists ([`LocaleBundle`]).
//!
//! Both go through a real [`AssetLoader`] rather than `std::fs`, so the
//! whole chain resolves through [`AssetServer`] — which is what lets
//! translations load on targets whose `assets/` is not a readable local
//! directory (wasm fetches over HTTP, Android reads the APK). The bundle
//! loader resolves each `.ftl` path its `.ftl.ron` names *explicitly*, so
//! nothing here ever has to enumerate a directory either; see the
//! [`localization`](super) module doc for why that matters.
//!
//! Derived from [`bevy_fluent`] 0.15.0 (MIT OR Apache-2.0, © 2021 Kazakov
//! Giorgi — see `LICENSE-bevy_fluent`), reduced to the surface this crate
//! actually uses: the `.ftl.yaml`/`.ftl.yml` bundle formats and the
//! `LoadedFolder`-based `LocalizationBuilder` are dropped, since
//! [`build_from_bundles`](super::build_from_bundles) negotiates the
//! fallback chain from explicit handles instead.
//!
//! [`bevy_fluent`]: https://github.com/kgv/bevy_fluent

use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use bevy::asset::{AssetLoader, AsyncReadExt, LoadContext, LoadDirectError, io::Reader};
use bevy::prelude::*;
use fluent::{FluentResource, bundle::FluentBundle};
use intl_memoizer::concurrent::IntlLangMemoizer;
use serde::Deserialize;
use thiserror::Error;
use unic_langid::LanguageIdentifier;

/// A parsed `.ftl` file: one locale's message definitions, shared behind an
/// [`Arc`] because a [`LocaleBundle`] holds it alongside the other
/// resources for its locale.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct FtlResource(Arc<FluentResource>);

impl Deref for FtlResource {
    type Target = FluentResource;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// One locale's [`FluentBundle`]: every `.ftl` resource its `main.ftl.ron`
/// lists, merged, and tagged with the locale it speaks for.
#[derive(Asset, TypePath, Clone)]
pub struct LocaleBundle(Arc<FluentBundle<Arc<FluentResource>, IntlLangMemoizer>>);

impl LocaleBundle {
    /// The locale this bundle was built for.
    ///
    /// [`FluentBundle::locales`] is a list because Fluent allows a bundle to
    /// declare several, but [`load_bundle`] always constructs one with
    /// exactly the single locale its `.ftl.ron` named, so index 0 is it.
    pub fn locale(&self) -> &LanguageIdentifier {
        &self.0.locales[0]
    }

    /// A bundle built from FTL source text directly rather than through the
    /// asset server: how a pack's translations arrive, since packs are read
    /// from disk the same way their lessons are. A message defined twice
    /// keeps its first definition, so earlier `sources` win.
    pub fn from_sources(
        locale: LanguageIdentifier,
        sources: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut bundle = FluentBundle::new_concurrent(vec![locale]);
        for source in sources {
            if let Err(errors) = bundle.add_resource(Arc::new(parse_resource(source))) {
                for error in errors {
                    warn!("FTL resource overrides an existing message: {error}");
                }
            }
        }
        Self(Arc::new(bundle))
    }
}

impl Deref for LocaleBundle {
    type Target = FluentBundle<Arc<FluentResource>, IntlLangMemoizer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Error, Debug)]
pub enum FtlLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// A `.ftl` resource a bundle references could not be loaded.
    #[error("nested load error: {0}")]
    LoadDirect(#[from] LoadDirectError),
    #[error("RON parse error: {0}")]
    Ron(#[from] ron::error::SpannedError),
    /// The path a `.ftl.ron` listed loaded, but as some other asset type —
    /// which means it didn't go through [`FtlResourceLoader`].
    #[error("{0} is not a .ftl resource")]
    NotAnFtlResource(PathBuf),
}

/// Loads a `.ftl` file into an [`FtlResource`].
#[derive(Default, TypePath)]
pub struct FtlResourceLoader;

impl AssetLoader for FtlResourceLoader {
    type Asset = FtlResource;
    type Settings = ();
    type Error = FtlLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<FtlResource, FtlLoadError> {
        let mut content = String::new();
        reader.read_to_string(&mut content).await?;
        Ok(FtlResource(Arc::new(parse_resource(content))))
    }

    fn extensions(&self) -> &[&str] {
        &["ftl"]
    }
}

/// Parses FTL source, logging any syntax errors rather than failing the
/// load: Fluent's parser is resilient and hands back every message it did
/// manage to parse, so one malformed entry costs that entry rather than
/// the whole locale.
fn parse_resource(content: String) -> FluentResource {
    match FluentResource::try_new(content) {
        Ok(resource) => resource,
        Err((resource, errors)) => {
            for error in errors {
                error!("FTL syntax error: {error}");
            }
            resource
        }
    }
}

/// A `main.ftl.ron` bundle manifest: the locale it defines, and the `.ftl`
/// resources that make it up (paths relative to the `.ftl.ron` itself).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleManifest {
    locale: LanguageIdentifier,
    resources: Vec<PathBuf>,
}

/// Loads a `.ftl.ron` manifest into a [`LocaleBundle`], loading each `.ftl`
/// resource it names along the way.
#[derive(Default, TypePath)]
pub struct LocaleBundleLoader;

impl AssetLoader for LocaleBundleLoader {
    type Asset = LocaleBundle;
    type Settings = ();
    type Error = FtlLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<LocaleBundle, FtlLoadError> {
        let mut content = String::new();
        reader.read_to_string(&mut content).await?;
        load_bundle(ron::de::from_str(&content)?, load_context).await
    }

    fn extensions(&self) -> &[&str] {
        &["ftl.ron"]
    }
}

async fn load_bundle(
    manifest: BundleManifest,
    load_context: &mut LoadContext<'_>,
) -> Result<LocaleBundle, FtlLoadError> {
    let mut bundle = FluentBundle::new_concurrent(vec![manifest.locale]);
    let parent = load_context.path().path().parent().map(PathBuf::from);
    for path in manifest.resources {
        // Relative paths are relative to the `.ftl.ron` that listed them,
        // not to the asset root.
        let path = match (&parent, path.is_relative()) {
            (Some(parent), true) => parent.join(path),
            _ => path,
        };
        let loaded = load_context.load_builder().load_untyped_value(path.clone()).await?;
        let resource = loaded.get::<FtlResource>().ok_or(FtlLoadError::NotAnFtlResource(path))?;
        if let Err(errors) = bundle.add_resource(resource.0.clone()) {
            for error in errors {
                warn!("FTL resource overrides an existing message: {error}");
            }
        }
    }
    Ok(LocaleBundle(Arc::new(bundle)))
}
