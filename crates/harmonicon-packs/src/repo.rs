// SPDX-License-Identifier: MIT

//! Where a pack comes from ([`RepoSpec`]) and what is installed
//! ([`Installed`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One configured repository. A remote is cloned and kept up to date; a local
/// `path` is used in place and never touched, which is how an author edits
/// `../harmonicon-lessons` and sees it live without committing anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RepoSpec {
    Remote {
        url: String,
        /// Branch or tag to follow; the remote's default branch when absent.
        #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
        git_ref: Option<String>,
    },
    Local {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpecError {
    #[error("{0:?} is not a repository address Harmonicon can use (https, ssh or file)")]
    UnsupportedUrl(String),
}

impl RepoSpec {
    /// Refuses addresses whose scheme could reach something other than a git
    /// remote (`ext::` runs commands in stock git; plain `http` is sent in
    /// the clear).
    pub fn validate(&self) -> Result<(), SpecError> {
        match self {
            RepoSpec::Local { .. } => Ok(()),
            RepoSpec::Remote { url, .. } => {
                let ok = ["https://", "ssh://", "file://"]
                    .iter()
                    .any(|scheme| url.starts_with(scheme))
                    || is_scp_like(url);
                if ok {
                    Ok(())
                } else {
                    Err(SpecError::UnsupportedUrl(url.clone()))
                }
            }
        }
    }

    /// The directory name this repository installs under. Derived from the
    /// address rather than the pack's id because it must be known before
    /// anything is downloaded.
    pub fn slug(&self) -> String {
        match self {
            RepoSpec::Remote { url, git_ref } => {
                let base = slug_of(url);
                match git_ref {
                    Some(r) => format!("{base}@{}", slug_of(r)),
                    None => base,
                }
            }
            RepoSpec::Local { path } => format!("local-{}", slug_of(&path.to_string_lossy())),
        }
    }
}

/// `git@host:owner/repo.git`.
fn is_scp_like(url: &str) -> bool {
    !url.contains("://")
        && url
            .split_once(':')
            .is_some_and(|(host, path)| host.contains('@') && !path.is_empty())
}

fn slug_of(text: &str) -> String {
    let text = text
        .split_once("://")
        .map_or(text, |(_, rest)| rest)
        .trim_end_matches('/')
        .trim_end_matches(".git");
    let slug: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    slug.trim_matches('-').trim_start_matches('.').to_string()
}

/// What a sync left on disk for one repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledRepo {
    pub url: String,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    /// The commit checked out, as a full hex id.
    pub commit: String,
    pub pack_version: String,
}

/// `installed.json`: every synced repository, by [`RepoSpec::slug`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub repos: BTreeMap<String, InstalledRepo>,
}

impl Installed {
    /// A missing or unreadable file is an empty record, not an error: the
    /// worst outcome is re-syncing, and the checkouts themselves are intact.
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Written to a sibling file and renamed, so a crash mid-write leaves the
    /// previous record rather than a truncated one.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(url: &str) -> RepoSpec {
        RepoSpec::Remote {
            url: url.into(),
            git_ref: None,
        }
    }

    #[test]
    fn only_git_transports_are_accepted() {
        for ok in [
            "https://github.com/tcanabrava/harmonicon-lessons",
            "https://gitlab.com/a/b.git",
            "ssh://git@host/a/b",
            "git@github.com:a/b.git",
            "file:///tmp/repo",
        ] {
            assert_eq!(remote(ok).validate(), Ok(()), "{ok}");
        }
        for bad in [
            "http://example.com/a",
            "ext::sh -c evil",
            "/just/a/path",
            "ftp://x/y",
            "",
        ] {
            assert!(remote(bad).validate().is_err(), "{bad}");
        }
    }

    #[test]
    fn slug_is_a_safe_directory_name() {
        let slug = remote("https://github.com/tcanabrava/harmonicon-lessons.git").slug();
        assert_eq!(slug, "github.com-tcanabrava-harmonicon-lessons");
        for hostile in ["https://../../etc", "git@h:../../x", "https://a/b c?d=e"] {
            let s = remote(hostile).slug();
            assert!(!s.contains('/') && !s.starts_with('.'), "{s}");
        }
    }

    #[test]
    fn different_refs_of_one_url_do_not_share_a_directory() {
        let main = remote("https://h/a/b");
        let tagged = RepoSpec::Remote {
            url: "https://h/a/b".into(),
            git_ref: Some("harmonicon-0.5".into()),
        };
        assert_ne!(main.slug(), tagged.slug());
    }

    #[test]
    fn specs_round_trip_through_json() {
        let specs = vec![
            remote("https://h/a/b"),
            RepoSpec::Remote {
                url: "https://h/a/c".into(),
                git_ref: Some("v1".into()),
            },
            RepoSpec::Local {
                path: "../harmonicon-lessons".into(),
            },
        ];
        let json = serde_json::to_string(&specs).unwrap();
        assert_eq!(serde_json::from_str::<Vec<RepoSpec>>(&json).unwrap(), specs);
    }

    #[test]
    fn installed_survives_a_round_trip_and_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/installed.json");
        assert_eq!(Installed::load(&path), Installed::default());

        let mut installed = Installed::default();
        installed.repos.insert(
            "x".into(),
            InstalledRepo {
                url: "https://h/a/b".into(),
                git_ref: None,
                commit: "abc".into(),
                pack_version: "1.0.0".into(),
            },
        );
        installed.save(&path).unwrap();
        assert_eq!(Installed::load(&path), installed);
    }

    #[test]
    fn a_corrupt_record_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("installed.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Installed::load(&path), Installed::default());
    }
}
