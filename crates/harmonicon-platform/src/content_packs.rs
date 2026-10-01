// SPDX-License-Identifier: MIT

//! Lesson and song packs: which repositories the player has configured
//! ([`ContentSources`], persisted with the other settings), what state each
//! one is in on disk ([`ContentPacks`]), and the `packs://` asset source the
//! usable ones load through.
//!
//! This module only *reads* what is installed. Downloading is
//! `harmonicon_packs::git`, driven by the startup sync and the Options page.
//! Nothing here knows what a lesson or a song is: the scans that do
//! (`assets_management::scan_all_songs`, `lessons::catalog`) ask
//! [`ContentPacks::usable`] for roots and run after [`ContentPacksSet`].
//!
//! # `packs://`
//!
//! One asset source serves every pack. Its first path segment names the pack
//! ([`PackEntry::slug`]) and [`PackRoots`] maps that to a directory, so a
//! song's chart is `packs://<slug>/<artist>/<song>/song/<file>.harpchart`.
//! A custom reader rather than one `FileAssetReader` per pack because asset
//! sources are fixed once `AssetPlugin` is built, while packs come and go at
//! runtime, and because a local pack can live anywhere on disk. It reads with
//! `std::fs`, which is also why it works on Android, where the default
//! reader is the APK's.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, Reader, VecReader,
};
use bevy::prelude::*;
use harmonicon_packs::pack::{EngineSupport, PackKind, PackManifest};
use harmonicon_packs::repo::{Installed, RepoSpec};
use serde::{Deserialize, Serialize};

/// The asset source name: `packs://...`.
pub const PACKS_SOURCE: &str = "packs";

/// The repositories the game ships configured with.
pub const OFFICIAL_SONGS: &str = "https://github.com/tcanabrava/harmonicon-songs";
pub const OFFICIAL_LESSONS: &str = "https://github.com/tcanabrava/harmonicon-lessons";

/// The configured repositories, in the order their content is listed.
/// Persisted in `settings.json`; a player who removes every repository of a
/// kind gets an empty list back, not the defaults.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentSources {
    pub songs: Vec<RepoSpec>,
    pub lessons: Vec<RepoSpec>,
}

impl Default for ContentSources {
    fn default() -> Self {
        let official = |url: &str| RepoSpec::Remote {
            url: url.into(),
            git_ref: None,
        };
        Self {
            songs: vec![official(OFFICIAL_SONGS)],
            lessons: vec![official(OFFICIAL_LESSONS)],
        }
    }
}

impl ContentSources {
    /// Every configured repository with its kind, songs first.
    pub fn all(&self) -> impl Iterator<Item = (PackKind, &RepoSpec)> {
        self.songs
            .iter()
            .map(|spec| (PackKind::Songs, spec))
            .chain(self.lessons.iter().map(|spec| (PackKind::Lessons, spec)))
    }
}

/// What this build can read. Built by the composition root, which is the
/// only place that knows both the game's version and the lesson format.
#[derive(Resource, Debug, Clone)]
pub struct PackEngine(pub EngineSupport);

/// The state of one configured repository on disk.
#[derive(Debug, Clone, PartialEq)]
pub enum PackStatus {
    /// A remote that has never been downloaded.
    NotInstalled,
    /// Installed and compatible. `commit` is `None` for a local folder.
    Ready {
        manifest: PackManifest,
        commit: Option<String>,
    },
    /// Present but unusable; the text is what the player is shown.
    Unusable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackEntry {
    pub kind: PackKind,
    pub spec: RepoSpec,
    /// First segment of this pack's `packs://` paths, and its directory name
    /// under [`crate::paths::packs_dir`] when it is a download.
    pub slug: String,
    /// Where the pack's files are.
    pub root: PathBuf,
    pub status: PackStatus,
}

impl PackEntry {
    /// What a usable pack's asset paths start with, e.g.
    /// `packs://github.com-tcanabrava-harmonicon-songs`.
    pub fn asset_prefix(&self) -> String {
        format!("{PACKS_SOURCE}://{}", self.slug)
    }
}

/// Every configured repository and its state, in [`ContentSources`] order.
#[derive(Resource, Debug, Default, Clone)]
pub struct ContentPacks(pub Vec<PackEntry>);

impl ContentPacks {
    /// The packs of `kind` whose content may be scanned.
    pub fn usable(&self, kind: PackKind) -> impl Iterator<Item = &PackEntry> {
        self.0
            .iter()
            .filter(move |e| e.kind == kind && matches!(e.status, PackStatus::Ready { .. }))
    }

    /// Whether a configured remote has never been downloaded: the condition
    /// for holding the game at the startup sync screen.
    pub fn any_missing(&self) -> bool {
        self.0.iter().any(|e| e.status == PackStatus::NotInstalled)
    }
}

/// Works out one repository's state from what is on disk. Pure apart from
/// reading `packs_dir`, so it tests against a temp directory.
pub fn evaluate(
    kind: PackKind,
    spec: &RepoSpec,
    packs_dir: &Path,
    installed: &Installed,
    engine: &EngineSupport,
) -> PackEntry {
    let slug = spec.slug();
    let root = match spec {
        RepoSpec::Remote { .. } => packs_dir.join(&slug),
        RepoSpec::Local { path } => path.clone(),
    };
    let status = match spec {
        RepoSpec::Remote { .. } if !root.join("pack.json").is_file() => PackStatus::NotInstalled,
        RepoSpec::Local { .. } if !root.is_dir() => {
            PackStatus::Unusable(format!("{} is not a folder", root.display()))
        }
        _ => read_status(&root, kind, engine, || {
            installed.repos.get(&slug).map(|r| r.commit.clone())
        }),
    };
    PackEntry {
        kind,
        spec: spec.clone(),
        slug,
        root,
        status,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_status(
    root: &Path,
    kind: PackKind,
    engine: &EngineSupport,
    commit: impl FnOnce() -> Option<String>,
) -> PackStatus {
    match harmonicon_packs::git::read_manifest(root, kind, engine) {
        Ok(manifest) => PackStatus::Ready {
            manifest,
            commit: commit(),
        },
        Err(e) => PackStatus::Unusable(e.to_string()),
    }
}

/// wasm bundles its packs at build time and never evaluates a configured
/// one; this only keeps the module compiling there.
#[cfg(target_arch = "wasm32")]
fn read_status(
    _root: &Path,
    _kind: PackKind,
    _engine: &EngineSupport,
    _commit: impl FnOnce() -> Option<String>,
) -> PackStatus {
    PackStatus::NotInstalled
}

/// The `packs://` routing table, shared between the asset reader (which
/// lives on Bevy's IO threads) and the ECS. Cloning shares the table.
#[derive(Resource, Clone, Default)]
pub struct PackRoots(Arc<RwLock<HashMap<String, PathBuf>>>);

impl PackRoots {
    fn replace(&self, roots: HashMap<String, PathBuf>) {
        *self.0.write().unwrap_or_else(|e| e.into_inner()) = roots;
    }

    /// The file `path` names, or `None` if its pack is unknown or the rest
    /// of the path tries to leave the pack's directory.
    fn resolve(&self, path: &Path) -> Option<PathBuf> {
        let mut components = path.components();
        let Some(Component::Normal(slug)) = components.next() else {
            return None;
        };
        let rest = components.as_path();
        if !rest.components().all(|c| matches!(c, Component::Normal(_))) {
            return None;
        }
        let roots = self.0.read().unwrap_or_else(|e| e.into_inner());
        let root = roots.get(slug.to_str()?)?;
        Some(root.join(rest))
    }
}

struct PackAssetReader(PackRoots);

impl PackAssetReader {
    fn locate(&self, path: &Path) -> Result<PathBuf, AssetReaderError> {
        self.0
            .resolve(path)
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    fn read_file(&self, path: &Path) -> Result<VecReader, AssetReaderError> {
        let full = self.locate(path)?;
        match std::fs::read(&full) {
            Ok(bytes) => Ok(VecReader::new(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl AssetReader for PackAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.read_file(path)
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let mut meta = path.as_os_str().to_owned();
        meta.push(".meta");
        self.read_file(Path::new(&meta))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let full = self.locate(path)?;
        let entries =
            std::fs::read_dir(&full).map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?;
        let base = path.to_path_buf();
        let children: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| base.join(entry.file_name()))
            .filter(|p| p.extension().is_none_or(|ext| ext != "meta"))
            .collect();
        Ok(Box::new(bevy::tasks::futures_lite::stream::iter(children)))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let full = self.locate(path)?;
        let metadata = full
            .metadata()
            .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?;
        Ok(metadata.is_dir())
    }
}

/// Registers `packs://`. Must run before `DefaultPlugins`, like every asset
/// source; the routing table starts empty and is filled once settings load.
pub fn register_pack_asset_source(app: &mut App) {
    let roots = PackRoots::default();
    let reader_roots = roots.clone();
    app.register_asset_source(
        PACKS_SOURCE,
        AssetSourceBuilder::new(move || Box::new(PackAssetReader(reader_roots.clone()))),
    );
    app.insert_resource(roots);
}

/// Ordering point: [`ContentPacks`] is current once this set has run, at
/// `Startup` and whenever a [`RefreshContentPacks`] is handled.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentPacksSet;

/// Ask for [`ContentPacks`] to be re-read from disk, e.g. after a sync or
/// after the player edits [`ContentSources`].
#[derive(Message)]
pub struct RefreshContentPacks;

/// [`ContentPacks`] changed at runtime; content scans should run again.
/// Not sent for the `Startup` read, which the scans already follow.
#[derive(Message)]
pub struct ContentPacksChanged;

/// Re-reads every configured repository's state and points `packs://` at the
/// usable ones.
fn refresh(
    sources: &ContentSources,
    engine: &EngineSupport,
    roots: Option<&PackRoots>,
    packs: &mut ContentPacks,
) {
    let Some(packs_dir) = crate::paths::packs_dir() else {
        warn!("No writable data directory; lesson and song packs are unavailable");
        packs.0.clear();
        return;
    };
    let installed = Installed::load(&packs_dir.join("installed.json"));
    packs.0 = sources
        .all()
        .map(|(kind, spec)| evaluate(kind, spec, &packs_dir, &installed, engine))
        .collect();
    for entry in &packs.0 {
        match &entry.status {
            PackStatus::Ready { manifest, .. } => info!(
                "{} pack {} {} from {}",
                entry.kind,
                manifest.id,
                manifest.version,
                entry.root.display()
            ),
            PackStatus::NotInstalled => {
                info!("{} pack {} is not installed", entry.kind, entry.slug)
            }
            PackStatus::Unusable(why) => warn!("{} pack {}: {why}", entry.kind, entry.slug),
        }
    }
    if let Some(roots) = roots {
        roots.replace(
            packs
                .0
                .iter()
                .filter(|e| matches!(e.status, PackStatus::Ready { .. }))
                .map(|e| (e.slug.clone(), e.root.clone()))
                .collect(),
        );
    }
}

fn refresh_at_startup(
    sources: Res<ContentSources>,
    engine: Res<PackEngine>,
    roots: Option<Res<PackRoots>>,
    mut packs: ResMut<ContentPacks>,
) {
    refresh(&sources, &engine.0, roots.as_deref(), &mut packs);
}

fn refresh_on_request(
    mut requests: MessageReader<RefreshContentPacks>,
    sources: Res<ContentSources>,
    engine: Res<PackEngine>,
    roots: Option<Res<PackRoots>>,
    mut packs: ResMut<ContentPacks>,
    mut changed: MessageWriter<ContentPacksChanged>,
) {
    if requests.read().count() == 0 {
        return;
    }
    refresh(&sources, &engine.0, roots.as_deref(), &mut packs);
    changed.write(ContentPacksChanged);
}

/// Needs [`PackEngine`] inserted first, and `SettingsPlugin` for
/// [`ContentSources`] to hold the player's list rather than the defaults.
pub struct ContentPacksPlugin;

impl Plugin for ContentPacksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ContentSources>()
            .init_resource::<ContentPacks>()
            .add_message::<RefreshContentPacks>()
            .add_message::<ContentPacksChanged>()
            .add_systems(
                Startup,
                refresh_at_startup
                    .in_set(ContentPacksSet)
                    .after(crate::settings::apply_loaded_settings),
            )
            .add_systems(Update, refresh_on_request.in_set(ContentPacksSet));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use harmonicon_packs::repo::InstalledRepo;

    fn engine() -> EngineSupport {
        EngineSupport {
            harmonicon: semver::Version::new(0, 5, 0),
            lesson_format: 1,
        }
    }

    fn remote(url: &str) -> RepoSpec {
        RepoSpec::Remote {
            url: url.into(),
            git_ref: None,
        }
    }

    fn write_pack(dir: &Path, kind: &str, requires: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("pack.json"),
            format!(
                r#"{{"schema":1,"kind":"{kind}","id":"p","name":"P","version":"1.0.0","requires":{{{requires}}}}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn a_remote_never_downloaded_is_not_installed() {
        let packs_dir = tempfile::tempdir().unwrap();
        let entry = evaluate(
            PackKind::Songs,
            &remote("https://h/a/b"),
            packs_dir.path(),
            &Installed::default(),
            &engine(),
        );
        assert_eq!(entry.status, PackStatus::NotInstalled);
        assert_eq!(entry.root, packs_dir.path().join(entry.slug.clone()));
    }

    #[test]
    fn an_installed_remote_is_ready_with_its_commit() {
        let packs_dir = tempfile::tempdir().unwrap();
        let spec = remote("https://h/a/b");
        write_pack(&packs_dir.path().join(spec.slug()), "songs", "");
        let mut installed = Installed::default();
        installed.repos.insert(
            spec.slug(),
            InstalledRepo {
                url: "https://h/a/b".into(),
                git_ref: None,
                commit: "abc".into(),
                pack_version: "1.0.0".into(),
            },
        );
        let entry = evaluate(
            PackKind::Songs,
            &spec,
            packs_dir.path(),
            &installed,
            &engine(),
        );
        let PackStatus::Ready { commit, manifest } = entry.status else {
            panic!("{:?}", entry.status);
        };
        assert_eq!(commit.as_deref(), Some("abc"));
        assert_eq!(manifest.id, "p");
    }

    #[test]
    fn an_incompatible_pack_is_unusable_with_a_reason() {
        let packs_dir = tempfile::tempdir().unwrap();
        let spec = remote("https://h/a/b");
        write_pack(
            &packs_dir.path().join(spec.slug()),
            "lessons",
            r#""harmonicon":">=9""#,
        );
        let entry = evaluate(
            PackKind::Lessons,
            &spec,
            packs_dir.path(),
            &Installed::default(),
            &engine(),
        );
        let PackStatus::Unusable(why) = entry.status else {
            panic!("{:?}", entry.status);
        };
        assert!(why.contains(">=9"), "{why}");
    }

    #[test]
    fn a_local_folder_is_used_in_place() {
        let author = tempfile::tempdir().unwrap();
        write_pack(author.path(), "lessons", "");
        let spec = RepoSpec::Local {
            path: author.path().into(),
        };
        let entry = evaluate(
            PackKind::Lessons,
            &spec,
            Path::new("/nonexistent"),
            &Installed::default(),
            &engine(),
        );
        assert_eq!(entry.root, author.path());
        assert!(matches!(
            entry.status,
            PackStatus::Ready { commit: None, .. }
        ));
    }

    #[test]
    fn usable_filters_by_kind_and_status() {
        let ready = |kind| PackEntry {
            kind,
            spec: remote("https://h/x"),
            slug: "x".into(),
            root: "/x".into(),
            status: PackStatus::Ready {
                manifest: PackManifest::parse(
                    br#"{"schema":1,"kind":"songs","id":"p","name":"P","version":"1.0.0"}"#,
                )
                .unwrap()
                .unwrap(),
                commit: None,
            },
        };
        let mut missing = ready(PackKind::Songs);
        missing.status = PackStatus::NotInstalled;
        let packs = ContentPacks(vec![
            ready(PackKind::Songs),
            ready(PackKind::Lessons),
            missing,
        ]);
        assert_eq!(packs.usable(PackKind::Songs).count(), 1);
        assert_eq!(packs.usable(PackKind::Lessons).count(), 1);
        assert!(packs.any_missing());
    }

    #[test]
    fn pack_paths_cannot_leave_their_pack() {
        let roots = PackRoots::default();
        roots.replace(HashMap::from([("p".to_string(), PathBuf::from("/data/p"))]));
        assert_eq!(
            roots.resolve(Path::new("p/Artist/Song/song/x.harpchart")),
            Some(PathBuf::from("/data/p/Artist/Song/song/x.harpchart"))
        );
        assert_eq!(roots.resolve(Path::new("p/../q/secret")), None);
        assert_eq!(roots.resolve(Path::new("unknown/file")), None);
        assert_eq!(roots.resolve(Path::new("/etc/passwd")), None);
    }

    #[test]
    fn the_defaults_are_the_official_repositories() {
        let sources = ContentSources::default();
        let urls: Vec<_> = sources
            .all()
            .map(|(kind, spec)| match spec {
                RepoSpec::Remote { url, .. } => (kind, url.as_str()),
                RepoSpec::Local { .. } => unreachable!(),
            })
            .collect();
        assert_eq!(
            urls,
            [
                (PackKind::Songs, OFFICIAL_SONGS),
                (PackKind::Lessons, OFFICIAL_LESSONS)
            ]
        );
    }
}
