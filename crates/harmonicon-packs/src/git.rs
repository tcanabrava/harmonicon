// SPDX-License-Identifier: MIT

//! Fetching packs: a depth-1 clone into a staging directory, checked and then
//! swapped into place, plus a remote-head query that downloads no objects.
//!
//! History never accumulates. An update is a fresh shallow clone rather than
//! a fetch into the existing checkout, so the on-disk size stays that of one
//! commit however many years of history the repository grows, and a failed or
//! refused update leaves the installed copy exactly as it was.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use gix::progress::Discard;
use gix::remote::Direction;
use gix::remote::fetch::Shallow;

use crate::pack::{EngineSupport, Incompatible, PackError, PackKind, PackManifest};
use crate::repo::{InstalledRepo, RepoSpec};

/// A pack's whole checkout may not exceed this. Content is charts, small
/// images and audio; anything larger is a mistake or hostile.
pub const MAX_PACK_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("a local folder is used in place and has nothing to download")]
    LocalSpec,
    #[error("could not download the repository: {0}")]
    Fetch(String),
    #[error("the remote has no branch or tag named {0:?}")]
    NoSuchRef(String),
    #[error("the repository has no commits yet")]
    EmptyRepository,
    #[error("the repository has no pack.json at its root")]
    NoManifest,
    #[error(transparent)]
    Manifest(#[from] PackError),
    #[error(transparent)]
    Incompatible(#[from] Incompatible),
    #[error("the repository contains a symbolic link ({0}), which packs may not")]
    Symlink(PathBuf),
    #[error("the repository is larger than {} MB", MAX_PACK_BYTES / (1024 * 1024))]
    TooLarge,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn fetch_err(e: impl std::fmt::Display) -> SyncError {
    SyncError::Fetch(e.to_string())
}

/// rustls is built without a default crypto provider (see this crate's
/// `Cargo.toml`), so one must be installed before the first https
/// connection. Installing twice is refused harmlessly, which is why the
/// result is ignored.
fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// The commit `spec`'s ref currently points at on the remote, as hex. Asks
/// only for the ref advertisement, so it is cheap enough to run on every
/// start. `checkout` is an installed copy of the repository, whose `origin`
/// it queries.
pub fn remote_head(checkout: &Path, spec: &RepoSpec) -> Result<String, SyncError> {
    let RepoSpec::Remote { git_ref, .. } = spec else {
        return Err(SyncError::LocalSpec);
    };
    ensure_crypto_provider();
    let repo = gix::open(checkout).map_err(fetch_err)?;
    let remote = repo.find_remote("origin").map_err(fetch_err)?;
    let connection = remote.connect(Direction::Fetch).map_err(fetch_err)?;
    let options = gix::remote::ref_map::Options {
        // The clone's refspec may name only the followed branch; ask for
        // everything so HEAD and tags are advertised too.
        prefix_from_spec_as_filter_on_remote: false,
        ..Default::default()
    };
    let (ref_map, _) = connection.ref_map(Discard, options).map_err(fetch_err)?;
    let wanted = wanted_ref_names(git_ref.as_deref());
    for name in &wanted {
        for remote_ref in &ref_map.remote_refs {
            let (full_name, target, peeled) = remote_ref.unpack();
            if full_name == name.as_str() {
                // An annotated tag advertises the tag object and the commit
                // it peels to; the commit is what a checkout records.
                if let Some(id) = peeled.or(target) {
                    return Ok(id.to_string());
                }
            }
        }
    }
    Err(SyncError::NoSuchRef(
        git_ref.clone().unwrap_or_else(|| "HEAD".into()),
    ))
}

/// Full ref names `git_ref` may mean, in the order git itself resolves them.
fn wanted_ref_names(git_ref: Option<&str>) -> Vec<String> {
    match git_ref {
        None => vec!["HEAD".into()],
        Some(r) if r.starts_with("refs/") => vec![r.into()],
        Some(r) => vec![format!("refs/heads/{r}"), format!("refs/tags/{r}")],
    }
}

/// Downloads `spec` at depth 1 into `packs_root/<slug>`, replacing whatever
/// was there only once the new copy has been checked: it must hold a
/// `pack.json` of the `kind` expected, compatible with `engine`, with no
/// symbolic links and within [`MAX_PACK_BYTES`].
///
/// `should_interrupt` cancels a download in progress; the staging directory
/// is removed and the installed copy is untouched.
pub fn install(
    spec: &RepoSpec,
    packs_root: &Path,
    kind: PackKind,
    engine: &EngineSupport,
    should_interrupt: &AtomicBool,
) -> Result<(InstalledRepo, PackManifest), SyncError> {
    let RepoSpec::Remote { url, git_ref } = spec else {
        return Err(SyncError::LocalSpec);
    };
    let slug = spec.slug();
    std::fs::create_dir_all(packs_root)?;
    let staging = packs_root.join(format!(".staging-{slug}"));
    remove_if_present(&staging)?;

    let result =
        clone_shallow(url, git_ref.as_deref(), &staging, should_interrupt).and_then(|commit| {
            let manifest = validate_checkout(&staging, kind, engine)?;
            if kind == PackKind::Songs {
                crate::retained_songs::preserve_removed(&packs_root.join(&slug), &staging)?;
                // Validate the combined copy before touching the installed pack.
                validate_checkout(&staging, kind, engine)?;
            }
            Ok((commit, manifest))
        });
    let (commit, manifest) = match result {
        Ok(ok) => ok,
        Err(e) => {
            let _ = remove_if_present(&staging);
            return Err(e);
        }
    };

    swap_into_place(&staging, &packs_root.join(&slug))?;
    Ok((
        InstalledRepo {
            url: url.clone(),
            git_ref: git_ref.clone(),
            commit,
            pack_version: manifest.version.to_string(),
        },
        manifest,
    ))
}

fn clone_shallow(
    url: &str,
    git_ref: Option<&str>,
    dest: &Path,
    should_interrupt: &AtomicBool,
) -> Result<String, SyncError> {
    ensure_crypto_provider();
    let mut prepare = gix::prepare_clone(url, dest)
        .map_err(fetch_err)?
        .with_shallow(Shallow::DepthAtRemote(NonZeroU32::MIN));
    if let Some(name) = git_ref {
        prepare = prepare.with_ref_name(Some(name)).map_err(fetch_err)?;
    }
    let (mut checkout, _) = prepare
        .fetch_then_checkout(Discard, should_interrupt)
        .map_err(|e| match (&e, git_ref) {
            // Nothing on the remote matched what we asked for: the named
            // branch or tag doesn't exist, or, following the default branch,
            // the repository has none yet. gix's own wording for both lists
            // refspecs, which says nothing to a player.
            (
                gix::clone::fetch::Error::RefNameMissing { .. }
                | gix::clone::fetch::Error::Fetch(gix::remote::fetch::Error::NoMapping { .. }),
                Some(name),
            ) => SyncError::NoSuchRef(name.into()),
            (
                gix::clone::fetch::Error::Fetch(gix::remote::fetch::Error::NoMapping { .. }),
                None,
            ) => SyncError::EmptyRepository,
            _ => fetch_err(e),
        })?;
    let (repo, _) = checkout
        .main_worktree(Discard, should_interrupt)
        .map_err(fetch_err)?;
    let head = repo.head_id().map_err(fetch_err)?;
    Ok(head.to_string())
}

/// Reads and checks a fresh checkout before it may replace an installed one.
pub fn validate_checkout(
    dir: &Path,
    kind: PackKind,
    engine: &EngineSupport,
) -> Result<PackManifest, SyncError> {
    check_tree(dir)?;
    read_manifest(dir, kind, engine)
}

/// Reads `dir/pack.json` and checks it against `engine`, without walking the
/// tree: for a pack already validated when it was installed, or a local
/// folder its author is editing.
pub fn read_manifest(
    dir: &Path,
    kind: PackKind,
    engine: &EngineSupport,
) -> Result<PackManifest, SyncError> {
    let bytes = match std::fs::read(dir.join("pack.json")) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(SyncError::NoManifest),
        Err(e) => return Err(e.into()),
    };
    let manifest = PackManifest::parse(&bytes)??;
    manifest.check(kind, engine)?;
    Ok(manifest)
}

/// No symbolic links (one could point an asset path anywhere on the
/// player's disk) and a bounded size. `.git` is skipped: it is ours, not the
/// pack's.
fn check_tree(dir: &Path) -> Result<(), SyncError> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                return Err(SyncError::Symlink(
                    path.strip_prefix(dir).unwrap_or(&path).to_path_buf(),
                ));
            }
            if file_type.is_dir() {
                if current == dir && entry.file_name() == ".git" {
                    continue;
                }
                stack.push(path);
            } else {
                total += entry.metadata()?.len();
                if total > MAX_PACK_BYTES {
                    return Err(SyncError::TooLarge);
                }
            }
        }
    }
    Ok(())
}

/// Moves `staging` to `dest`. The old copy is renamed aside first and only
/// deleted after the new one is in place, so at every instant one complete
/// copy exists under a known name.
fn swap_into_place(staging: &Path, dest: &Path) -> std::io::Result<()> {
    let old = dest.with_file_name(format!(
        ".old-{}",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));
    remove_if_present(&old)?;
    if dest.exists() {
        std::fs::rename(dest, &old)?;
    }
    if let Err(e) = std::fs::rename(staging, dest) {
        // Put the previous copy back rather than leave nothing installed.
        if old.exists() {
            let _ = std::fs::rename(&old, dest);
        }
        return Err(e);
    }
    remove_if_present(&old)
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    //! These run against real repositories made with the `git` binary and
    //! served over `file://`, which gix also serves through `git
    //! upload-pack`. The game itself only ever talks https or ssh.

    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git is installed");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn engine() -> EngineSupport {
        EngineSupport {
            harmonicon: semver::Version::new(0, 5, 0),
            lesson_format: 1,
        }
    }

    const PACK: &str =
        r#"{"schema":1,"kind":"lessons","id":"core","name":"Core","version":"1.0.0"}"#;

    /// A repository with `pack.json` and `n` commits, each touching a file.
    fn upstream(n: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        std::fs::write(dir.path().join("pack.json"), PACK).unwrap();
        for i in 0..n {
            std::fs::write(dir.path().join("history.txt"), "x".repeat(10_000 * (i + 1))).unwrap();
            git(dir.path(), &["add", "-A"]);
            git(dir.path(), &["commit", "-q", "-m", &format!("c{i}")]);
        }
        dir
    }

    fn spec_for(dir: &Path, git_ref: Option<&str>) -> RepoSpec {
        RepoSpec::Remote {
            url: format!("file://{}", dir.display()),
            git_ref: git_ref.map(Into::into),
        }
    }

    fn install_into(spec: &RepoSpec, root: &Path) -> Result<InstalledRepo, SyncError> {
        install(
            spec,
            root,
            PackKind::Lessons,
            &engine(),
            &AtomicBool::new(false),
        )
        .map(|(r, _)| r)
    }

    #[test]
    fn installs_only_the_latest_commit() {
        let up = upstream(5);
        let root = tempfile::tempdir().unwrap();
        let spec = spec_for(up.path(), None);

        let installed = install_into(&spec, root.path()).unwrap();
        assert_eq!(installed.commit, git(up.path(), &["rev-parse", "HEAD"]));

        let checkout = root.path().join(spec.slug());
        assert!(checkout.join("pack.json").is_file());
        let count = git(&checkout, &["rev-list", "--count", "HEAD"]);
        assert_eq!(count, "1", "history was downloaded");
        assert!(
            !root
                .path()
                .join(format!(".staging-{}", spec.slug()))
                .exists()
        );
    }

    #[test]
    fn updating_a_song_pack_preserves_upstream_deletions() {
        let upstream = upstream(1);
        let folder = upstream.path().join("Band/Removed");
        std::fs::create_dir_all(folder.join("song")).unwrap();
        std::fs::write(folder.join("song/chart.harpchart"), "chart").unwrap();
        std::fs::write(folder.join("backing.ogg"), "audio").unwrap();
        std::fs::write(
            upstream.path().join("pack.json"),
            PACK.replace("lessons", "songs"),
        )
        .unwrap();
        git(upstream.path(), &["add", "-A"]);
        git(upstream.path(), &["commit", "-q", "-m", "songs"]);
        let root = tempfile::tempdir().unwrap();
        let spec = spec_for(upstream.path(), None);
        let update = || {
            install(
                &spec,
                root.path(),
                PackKind::Songs,
                &engine(),
                &AtomicBool::new(false),
            )
            .unwrap()
        };
        update();
        std::fs::remove_dir_all(&folder).unwrap();
        git(upstream.path(), &["add", "-A"]);
        git(upstream.path(), &["commit", "-q", "-m", "remove song"]);
        update();
        let checkout = root.path().join(spec.slug());
        assert!(checkout.join("Band/Removed/song/chart.harpchart").exists());
        assert_eq!(
            std::fs::read_to_string(checkout.join("Band/Removed/backing.ogg")).unwrap(),
            "audio"
        );
        assert!(crate::retained_songs::read_index(&checkout).contains("Band/Removed"));
        update();
        assert!(checkout.join("Band/Removed/backing.ogg").exists());
    }

    #[test]
    fn remote_head_sees_a_new_commit_without_downloading_it() {
        let up = upstream(1);
        let root = tempfile::tempdir().unwrap();
        let spec = spec_for(up.path(), None);
        let installed = install_into(&spec, root.path()).unwrap();
        let checkout = root.path().join(spec.slug());

        assert_eq!(remote_head(&checkout, &spec).unwrap(), installed.commit);

        std::fs::write(up.path().join("new.txt"), "n").unwrap();
        git(up.path(), &["add", "-A"]);
        git(up.path(), &["commit", "-q", "-m", "new"]);
        let new_head = git(up.path(), &["rev-parse", "HEAD"]);
        assert_eq!(remote_head(&checkout, &spec).unwrap(), new_head);
        assert!(
            !checkout.join("new.txt").exists(),
            "a check must not update"
        );

        let updated = install_into(&spec, root.path()).unwrap();
        assert_eq!(updated.commit, new_head);
        assert!(checkout.join("new.txt").exists());
        assert_eq!(git(&checkout, &["rev-list", "--count", "HEAD"]), "1");
    }

    #[test]
    fn follows_a_named_branch_and_tag() {
        let up = upstream(1);
        git(up.path(), &["tag", "-a", "v1", "-m", "v1"]);
        let tagged = git(up.path(), &["rev-parse", "HEAD"]);
        git(up.path(), &["checkout", "-q", "-b", "harmonicon-0.5"]);
        std::fs::write(up.path().join("branch.txt"), "b").unwrap();
        git(up.path(), &["add", "-A"]);
        git(up.path(), &["commit", "-q", "-m", "on branch"]);
        let branch_head = git(up.path(), &["rev-parse", "HEAD"]);
        git(up.path(), &["checkout", "-q", "main"]);

        let root = tempfile::tempdir().unwrap();
        let branch = spec_for(up.path(), Some("harmonicon-0.5"));
        assert_eq!(
            install_into(&branch, root.path()).unwrap().commit,
            branch_head
        );
        assert!(root.path().join(branch.slug()).join("branch.txt").exists());
        let checkout = root.path().join(branch.slug());
        assert_eq!(remote_head(&checkout, &branch).unwrap(), branch_head);

        let tag = spec_for(up.path(), Some("v1"));
        assert_eq!(remote_head(&checkout, &tag).unwrap(), tagged);
    }

    #[test]
    fn an_incompatible_update_keeps_the_installed_copy() {
        let up = upstream(1);
        let root = tempfile::tempdir().unwrap();
        let spec = spec_for(up.path(), None);
        let first = install_into(&spec, root.path()).unwrap();

        std::fs::write(
            up.path().join("pack.json"),
            PACK.replace(
                "\"version\":\"1.0.0\"",
                "\"version\":\"2.0.0\",\"requires\":{\"lesson_format\":9}",
            ),
        )
        .unwrap();
        git(up.path(), &["commit", "-q", "-am", "needs newer engine"]);

        let err = install_into(&spec, root.path()).unwrap_err();
        assert!(matches!(
            err,
            SyncError::Incompatible(Incompatible::LessonFormatTooNew { .. })
        ));
        let checkout = root.path().join(spec.slug());
        assert_eq!(git(&checkout, &["rev-parse", "HEAD"]), first.commit);
        assert!(std::fs::read_dir(root.path()).unwrap().all(|e| {
            !e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".staging")
        }));
    }

    #[test]
    fn a_repository_without_pack_json_is_refused() {
        let up = tempfile::tempdir().unwrap();
        git(up.path(), &["init", "-q"]);
        std::fs::write(up.path().join("readme"), "hi").unwrap();
        git(up.path(), &["add", "-A"]);
        git(up.path(), &["commit", "-q", "-m", "c"]);
        let root = tempfile::tempdir().unwrap();
        let err = install_into(&spec_for(up.path(), None), root.path()).unwrap_err();
        assert!(matches!(err, SyncError::NoManifest), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_the_pack_is_refused() {
        let up = upstream(1);
        std::os::unix::fs::symlink("/etc/passwd", up.path().join("escape")).unwrap();
        git(up.path(), &["add", "-A"]);
        git(up.path(), &["commit", "-q", "-m", "link"]);
        let root = tempfile::tempdir().unwrap();
        let spec = spec_for(up.path(), None);
        let err = install_into(&spec, root.path()).unwrap_err();
        assert!(matches!(err, SyncError::Symlink(_)), "{err}");
        assert!(!root.path().join(spec.slug()).exists());
    }

    #[test]
    fn a_missing_branch_is_reported_as_such() {
        let up = upstream(1);
        let root = tempfile::tempdir().unwrap();
        let err = install_into(&spec_for(up.path(), Some("nope")), root.path()).unwrap_err();
        assert!(matches!(err, SyncError::NoSuchRef(_)), "{err}");
    }

    /// What a freshly created GitHub repository looks like.
    #[test]
    fn an_empty_repository_is_reported_as_such() {
        let up = tempfile::tempdir().unwrap();
        git(up.path(), &["init", "-q"]);
        let root = tempfile::tempdir().unwrap();
        let err = install_into(&spec_for(up.path(), None), root.path()).unwrap_err();
        assert!(matches!(err, SyncError::EmptyRepository), "{err}");
    }

    /// The https transport end to end, TLS included. Needs the network, so
    /// it runs only on request: `cargo test -p harmonicon-packs -- --ignored`.
    /// The repository has no `pack.json`, so getting as far as noticing that
    /// is the success condition.
    #[test]
    #[ignore = "needs network access"]
    fn fetches_over_https() {
        let root = tempfile::tempdir().unwrap();
        let spec = RepoSpec::Remote {
            url: "https://github.com/octocat/Hello-World".into(),
            git_ref: None,
        };
        let err = install_into(&spec, root.path()).unwrap_err();
        assert!(matches!(err, SyncError::NoManifest), "{err}");
    }

    #[test]
    fn local_specs_are_never_downloaded() {
        let root = tempfile::tempdir().unwrap();
        let spec = RepoSpec::Local {
            path: root.path().into(),
        };
        assert!(matches!(
            install_into(&spec, root.path()),
            Err(SyncError::LocalSpec)
        ));
    }
}
