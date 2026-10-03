// SPDX-License-Identifier: MIT

//! Downloading packs and checking them for updates, in the background.
//!
//! Each clone or update check runs on its own named thread rather than a
//! Bevy task pool: they are long, blocking network calls, and parking one on
//! the IO pool would stall asset loading for its whole duration. Results
//! come back over a channel and are applied on the main thread, which is
//! the only writer of `installed.json`.
//!
//! Nothing is ever updated without asking: at startup a repository that has
//! never been downloaded is installed (the game can't run without its
//! lessons and songs), while an installed one is only *checked*, and the
//! answer lands in [`PackSync::updates`] for the Options page to offer.
//!
//! Native only: wasm has no sockets, and gets its packs at build time.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use crossbeam_channel::{Receiver, TryRecvError};
use harmonicon_packs::git;
use harmonicon_packs::pack::{PackKind, PackManifest};
use harmonicon_packs::repo::{Installed, InstalledRepo, RepoSpec};

use crate::content_packs::{
    ContentPacks, ContentPacksSet, PackEngine, PackStatus, RefreshContentPacks,
};

/// Download (or re-download, for an update) one repository.
#[derive(Message, Clone, Debug)]
pub struct InstallPack {
    pub kind: PackKind,
    pub spec: RepoSpec,
}

/// Ask every installed remote whether its branch has moved.
#[derive(Message, Clone, Debug, Default)]
pub struct CheckPackUpdates;

/// An [`InstallPack`] finished; `error` is `None` on success.
#[derive(Message, Clone, Debug)]
pub struct PackSyncFinished {
    pub slug: String,
    pub error: Option<String>,
}

/// What a check found for one installed remote, by slug.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Checking,
    UpToDate,
    /// The remote's branch points at `commit`, which is not installed.
    Available {
        commit: String,
    },
    Failed(String),
}

type InstallResult = Result<(InstalledRepo, PackManifest), String>;

struct Job {
    slug: String,
    rx: Receiver<InstallResult>,
    /// Finished with nothing sent means the thread panicked.
    thread: std::thread::JoinHandle<()>,
    cancel: Arc<AtomicBool>,
}

struct Check {
    slug: String,
    installed_commit: Option<String>,
    rx: Receiver<Result<String, String>>,
    thread: std::thread::JoinHandle<()>,
}

/// Every download and check in flight, and what the finished ones found.
#[derive(Resource, Default)]
pub struct PackSync {
    jobs: Vec<Job>,
    checks: Vec<Check>,
    /// The last download error per slug; cleared by a successful one.
    pub failures: HashMap<String, String>,
    pub updates: HashMap<String, UpdateState>,
}

impl PackSync {
    /// Whether a download of `slug` is running.
    pub fn installing(&self, slug: &str) -> bool {
        self.jobs.iter().any(|j| j.slug == slug)
    }

    /// Whether any download is running. Update checks don't count: they
    /// never block the game.
    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }
}

fn spawn_install(sync: &mut PackSync, kind: PackKind, spec: RepoSpec, engine: &PackEngine) {
    let slug = spec.slug();
    if sync.installing(&slug) || !matches!(spec, RepoSpec::Remote { .. }) {
        return;
    }
    let Some(packs_dir) = crate::paths::packs_dir() else {
        sync.failures.insert(slug, "no writable data directory".into());
        return;
    };
    let (tx, rx) = crossbeam_channel::bounded(1);
    let cancel = Arc::new(AtomicBool::new(false));
    let thread_cancel = cancel.clone();
    let engine = engine.0.clone();
    let thread_slug = slug.clone();
    let spawned = std::thread::Builder::new().name(format!("pack-sync {slug}")).spawn(move || {
        let _span = info_span!("pack_install", slug = thread_slug).entered();
        let result = git::install(&spec, &packs_dir, kind, &engine, &thread_cancel)
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    match spawned {
        Ok(thread) => {
            info!("Downloading {kind} pack {slug}");
            sync.failures.remove(&slug);
            sync.jobs.push(Job { slug, rx, thread, cancel });
        }
        Err(e) => {
            sync.failures.insert(slug, e.to_string());
        }
    }
}

fn spawn_check(
    sync: &mut PackSync,
    entry_slug: &str,
    root: &std::path::Path,
    spec: &RepoSpec,
    installed_commit: Option<String>,
) {
    if sync.checks.iter().any(|c| c.slug == entry_slug) {
        return;
    }
    let (tx, rx) = crossbeam_channel::bounded(1);
    let root = root.to_path_buf();
    let spec = spec.clone();
    let slug = entry_slug.to_string();
    let spawned = std::thread::Builder::new().name(format!("pack-check {slug}")).spawn(move || {
        let _span = info_span!("pack_check", slug).entered();
        let _ = tx.send(git::remote_head(&root, &spec).map_err(|e| e.to_string()));
    });
    if let Ok(thread) = spawned {
        sync.updates.insert(entry_slug.to_string(), UpdateState::Checking);
        sync.checks.push(Check { slug: entry_slug.to_string(), installed_commit, rx, thread });
    }
}

/// At startup: download what has never been downloaded, check the rest.
fn sync_at_startup(packs: Res<ContentPacks>, engine: Res<PackEngine>, mut sync: ResMut<PackSync>) {
    for entry in &packs.0 {
        match &entry.status {
            PackStatus::NotInstalled => {
                spawn_install(&mut sync, entry.kind, entry.spec.clone(), &engine);
            }
            PackStatus::Ready { commit, .. } if matches!(entry.spec, RepoSpec::Remote { .. }) => {
                spawn_check(&mut sync, &entry.slug, &entry.root, &entry.spec, commit.clone());
            }
            _ => {}
        }
    }
}

fn handle_requests(
    mut installs: MessageReader<InstallPack>,
    mut checks: MessageReader<CheckPackUpdates>,
    packs: Res<ContentPacks>,
    engine: Res<PackEngine>,
    mut sync: ResMut<PackSync>,
) {
    for request in installs.read() {
        spawn_install(&mut sync, request.kind, request.spec.clone(), &engine);
    }
    if checks.read().count() > 0 {
        for entry in &packs.0 {
            if let (PackStatus::Ready { commit, .. }, RepoSpec::Remote { .. }) =
                (&entry.status, &entry.spec)
            {
                spawn_check(&mut sync, &entry.slug, &entry.root, &entry.spec, commit.clone());
            }
        }
    }
}

/// Collects finished downloads and checks. A finished download is recorded
/// in `installed.json` here, on the main thread, and triggers one
/// [`RefreshContentPacks`] for however many finished this frame.
fn collect_results(
    mut sync_res: ResMut<PackSync>,
    mut finished: MessageWriter<PackSyncFinished>,
    mut refresh: MessageWriter<RefreshContentPacks>,
) {
    // Polled every frame, so it must not mark the resource changed unless
    // something actually finished: the Options page rebuilds on a change.
    let anything_done = sync_res.jobs.iter().any(|j| !j.rx.is_empty() || j.thread.is_finished())
        || sync_res.checks.iter().any(|c| !c.rx.is_empty() || c.thread.is_finished());
    if !anything_done {
        return;
    }
    let sync = &mut *sync_res;
    let mut any_finished = false;
    let mut still_running = Vec::new();
    for job in sync.jobs.drain(..) {
        let result = match job.rx.try_recv() {
            Err(TryRecvError::Empty) => {
                still_running.push(job);
                continue;
            }
            Ok(result) => result,
            Err(TryRecvError::Disconnected) => Err("the download stopped unexpectedly".into()),
        };
        any_finished = true;
        let error = match result {
            Ok((repo, manifest)) => match record_install(&job.slug, repo) {
                Ok(()) => {
                    info!("Installed pack {} {}", manifest.id, manifest.version);
                    sync.updates.insert(job.slug.clone(), UpdateState::UpToDate);
                    None
                }
                Err(e) => Some(e),
            },
            Err(e) => Some(e),
        };
        match &error {
            Some(e) => {
                warn!("Could not download pack {}: {e}", job.slug);
                sync.failures.insert(job.slug.clone(), e.clone());
            }
            None => {
                sync.failures.remove(&job.slug);
            }
        }
        finished.write(PackSyncFinished { slug: job.slug, error });
    }
    sync.jobs = still_running;

    let mut still_checking = Vec::new();
    for check in sync.checks.drain(..) {
        let state = match check.rx.try_recv() {
            Err(TryRecvError::Empty) => {
                still_checking.push(check);
                continue;
            }
            Ok(Ok(remote)) if Some(&remote) == check.installed_commit.as_ref() => {
                UpdateState::UpToDate
            }
            Ok(Ok(remote)) => UpdateState::Available { commit: remote },
            Ok(Err(e)) => UpdateState::Failed(e),
            Err(TryRecvError::Disconnected) => {
                UpdateState::Failed("the check stopped unexpectedly".into())
            }
        };
        sync.updates.insert(check.slug, state);
    }
    sync.checks = still_checking;

    if any_finished {
        refresh.write(RefreshContentPacks);
    }
}

/// Deletes a downloaded pack's checkout and its `installed.json` entry. Only
/// ever touches `packs_dir/<slug>`, which is why it takes a [`RepoSpec`]
/// rather than a path: a local folder is the author's own and is never
/// deleted, only dropped from the list by the caller.
pub fn uninstall(spec: &RepoSpec) -> Result<(), String> {
    if !matches!(spec, RepoSpec::Remote { .. }) {
        return Ok(());
    }
    let packs_dir = crate::paths::packs_dir().ok_or("no writable data directory")?;
    let slug = spec.slug();
    match std::fs::remove_dir_all(packs_dir.join(&slug)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    let path = packs_dir.join("installed.json");
    let mut installed = Installed::load(&path);
    if installed.repos.remove(&slug).is_some() {
        installed.save(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn record_install(slug: &str, repo: InstalledRepo) -> Result<(), String> {
    let path =
        crate::paths::packs_dir().ok_or("no writable data directory")?.join("installed.json");
    let mut installed = Installed::load(&path);
    installed.repos.insert(slug.to_string(), repo);
    installed.save(&path).map_err(|e| e.to_string())
}

/// Stops downloads in progress when the game quits. A stopped download
/// leaves only its staging directory, which the next attempt removes.
fn cancel_on_exit(mut exit: MessageReader<AppExit>, sync: Res<PackSync>) {
    if exit.read().count() > 0 {
        for job in &sync.jobs {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<PackSync>()
        .add_message::<InstallPack>()
        .add_message::<CheckPackUpdates>()
        .add_message::<PackSyncFinished>()
        .add_systems(Startup, sync_at_startup.after(ContentPacksSet))
        .add_systems(Update, (handle_requests, collect_results).chain().before(ContentPacksSet))
        .add_systems(Last, cancel_on_exit);
}
