// SPDX-License-Identifier: MIT

//! `pack.json`: the file at a content repository's root that says what the
//! repository is and which engine it needs.
//!
//! Compatibility is a pure function of the manifest and an [`EngineSupport`]
//! the caller builds from its own constants, so this crate never has to know
//! the engine's version and the rules test without one.

use std::fmt;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// The newest `pack.json` `schema` this build understands.
pub const PACK_SCHEMA: u32 = 1;

/// What a pack holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackKind {
    Lessons,
    Songs,
}

impl fmt::Display for PackKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PackKind::Lessons => "lessons",
            PackKind::Songs => "songs",
        })
    }
}

/// What a pack needs from the engine. Every field is optional: a pack that
/// states nothing loads on any engine that understands its `schema`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    /// A semver requirement on the game's version, e.g. `">=0.5"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harmonicon: Option<VersionReq>,
    /// The lesson manifest format the pack's lessons are written for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lesson_format: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub kind: PackKind,
    /// Stable identifier; also the name of the pack's directory on disk.
    pub id: String,
    pub name: String,
    pub version: Version,
    #[serde(default)]
    pub requires: Requires,
}

/// What this build of the engine can read.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineSupport {
    pub harmonicon: Version,
    pub lesson_format: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("pack.json is not valid: {0}")]
    Invalid(String),
    #[error("pack id {0:?} is not a plain directory name")]
    BadId(String),
}

/// Why a well-formed pack cannot be used by this engine. Its `Display` is
/// what the player reads, so it names what to do about it.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Incompatible {
    #[error(
        "this pack uses pack format {found}, newer than the {supported} this Harmonicon understands; update Harmonicon"
    )]
    SchemaTooNew { found: u32, supported: u32 },
    #[error("this pack needs Harmonicon {required}, but this is {engine}")]
    EngineMismatch {
        required: VersionReq,
        engine: Version,
    },
    #[error(
        "this pack's lessons use format {found}, newer than the {supported} this Harmonicon understands; update Harmonicon"
    )]
    LessonFormatTooNew { found: u32, supported: u32 },
    #[error("this repository holds {found} but was added as {expected}")]
    WrongKind { expected: PackKind, found: PackKind },
}

#[derive(Deserialize)]
struct SchemaProbe {
    schema: u32,
}

impl PackManifest {
    /// Parses `pack.json`. A pack declaring a newer `schema` than
    /// [`PACK_SCHEMA`] is reported as [`Incompatible::SchemaTooNew`] by
    /// [`Self::check`] only if it still parses; one whose newer shape no
    /// longer fits is reported here as that incompatibility too, rather than
    /// as a confusing field error.
    pub fn parse(bytes: &[u8]) -> Result<Result<Self, Incompatible>, PackError> {
        let probe: SchemaProbe =
            serde_json::from_slice(bytes).map_err(|e| PackError::Invalid(e.to_string()))?;
        if probe.schema > PACK_SCHEMA {
            return Ok(Err(Incompatible::SchemaTooNew {
                found: probe.schema,
                supported: PACK_SCHEMA,
            }));
        }
        let manifest: Self =
            serde_json::from_slice(bytes).map_err(|e| PackError::Invalid(e.to_string()))?;
        if !is_plain_name(&manifest.id) {
            return Err(PackError::BadId(manifest.id));
        }
        Ok(Ok(manifest))
    }

    /// Whether `engine` can use this pack as a `expected` pack.
    pub fn check(&self, expected: PackKind, engine: &EngineSupport) -> Result<(), Incompatible> {
        if self.kind != expected {
            return Err(Incompatible::WrongKind {
                expected,
                found: self.kind,
            });
        }
        if let Some(req) = &self.requires.harmonicon {
            // A release candidate satisfies the requirement its release
            // will: `>=0.5` must accept `0.5.0-rc1`, which semver alone
            // refuses for any pre-release.
            let mut release = engine.harmonicon.clone();
            release.pre = semver::Prerelease::EMPTY;
            if !req.matches(&release) {
                return Err(Incompatible::EngineMismatch {
                    required: req.clone(),
                    engine: engine.harmonicon.clone(),
                });
            }
        }
        if let Some(found) = self.requires.lesson_format
            && found > engine.lesson_format
        {
            return Err(Incompatible::LessonFormatTooNew {
                found,
                supported: engine.lesson_format,
            });
        }
        Ok(())
    }
}

/// An id becomes a directory name under the packs root, so it must not be
/// able to name anything else.
fn is_plain_name(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> EngineSupport {
        EngineSupport {
            harmonicon: Version::parse("0.5.0").unwrap(),
            lesson_format: 2,
        }
    }

    fn parse_ok(json: &str) -> PackManifest {
        PackManifest::parse(json.as_bytes()).unwrap().unwrap()
    }

    const MINIMAL: &str =
        r#"{"schema":1,"kind":"lessons","id":"core","name":"Core","version":"1.2.0"}"#;

    #[test]
    fn a_pack_stating_no_requirements_loads_anywhere() {
        let pack = parse_ok(MINIMAL);
        assert_eq!(pack.check(PackKind::Lessons, &engine()), Ok(()));
    }

    #[test]
    fn a_newer_schema_is_incompatible_not_invalid() {
        let outcome = PackManifest::parse(br#"{"schema":9,"shape":"unknown to us"}"#).unwrap();
        assert_eq!(
            outcome.unwrap_err(),
            Incompatible::SchemaTooNew {
                found: 9,
                supported: PACK_SCHEMA
            }
        );
    }

    #[test]
    fn engine_requirement_is_checked() {
        let pack = parse_ok(
            r#"{"schema":1,"kind":"lessons","id":"c","name":"C","version":"1.0.0",
                "requires":{"harmonicon":">=0.6"}}"#,
        );
        assert!(matches!(
            pack.check(PackKind::Lessons, &engine()),
            Err(Incompatible::EngineMismatch { .. })
        ));
    }

    #[test]
    fn a_release_candidate_meets_its_own_releases_requirement() {
        let pack = parse_ok(
            r#"{"schema":1,"kind":"songs","id":"s","name":"S","version":"1.0.0",
                "requires":{"harmonicon":">=0.5"}}"#,
        );
        let rc = EngineSupport {
            harmonicon: Version::parse("0.5.0-rc.1").unwrap(),
            lesson_format: 1,
        };
        assert_eq!(pack.check(PackKind::Songs, &rc), Ok(()));
    }

    #[test]
    fn lesson_format_newer_than_the_engine_is_refused() {
        let pack = parse_ok(
            r#"{"schema":1,"kind":"lessons","id":"c","name":"C","version":"1.0.0",
                "requires":{"lesson_format":3}}"#,
        );
        assert_eq!(
            pack.check(PackKind::Lessons, &engine()),
            Err(Incompatible::LessonFormatTooNew {
                found: 3,
                supported: 2
            })
        );
    }

    #[test]
    fn a_songs_pack_is_not_accepted_as_lessons() {
        let pack = parse_ok(&MINIMAL.replace("lessons", "songs"));
        assert!(matches!(
            pack.check(PackKind::Lessons, &engine()),
            Err(Incompatible::WrongKind { .. })
        ));
    }

    #[test]
    fn ids_cannot_escape_the_packs_directory() {
        for id in ["", "..", "a/b", "../x", "a b"] {
            let json = MINIMAL.replace("\"core\"", &format!("{id:?}"));
            assert!(
                matches!(
                    PackManifest::parse(json.as_bytes()),
                    Err(PackError::BadId(_))
                ),
                "{id:?} was accepted"
            );
        }
    }

    #[test]
    fn unknown_requirement_keys_are_rejected() {
        let json = MINIMAL.replace(
            "\"version\":\"1.2.0\"",
            "\"version\":\"1.2.0\",\"requires\":{\"typo\":1}",
        );
        assert!(PackManifest::parse(json.as_bytes()).is_err());
    }
}
