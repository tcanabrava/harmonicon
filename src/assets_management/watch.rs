// SPDX-License-Identifier: MIT

//! Watches `~/Harmonicon` (the external drop folder registered in `main.rs`)
//! for changes and turns them into one generic [`ExternalFolderChanged`]
//! message per debounced batch, naming which of its immediate subfolders
//! (`songs`, `themes`, `lessons`, ...) had something change under it.
//!
//! Deliberately agnostic of what any of those subfolders *mean* — this
//! module is shared low-level vocabulary (`docs/physical_design_plan.md`:
//! "dependencies point downward"), so it doesn't reach up into
//! feature-specific knowledge like what a lesson is. `mod.rs`'s own
//! `rescan_on_external_change` consumes this message for `songs`/`themes`
//! (the two kinds this module already owns); `lessons::catalog` consumes
//! the same message for `lessons` from the other side, one small
//! `assets_management`-downward dependency rather than the reverse.
//!
//! No-ops entirely if `~/Harmonicon` doesn't exist: most players never
//! create it, same "external folder is optional" tolerance the scans in
//! `mod.rs` already have. If the watcher can't be started for some other
//! reason (inotify limits, permissions, unsupported platform), this only
//! logs a warning — the external folder is still scanned once at Startup,
//! just not kept live.

use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use notify_debouncer_full::{Debouncer, DebounceEventResult, RecommendedCache, new_debouncer};
use notify_debouncer_full::notify::event::{EventKind, ModifyKind};
use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A saved file often fires several raw events in quick succession (create +
/// write, or an editor's atomic rename-into-place); this coalesces a burst
/// into one rescan rather than one per event.
const WATCH_DEBOUNCE: Duration = Duration::from_millis(500);

/// Keeps the debouncer (and its background thread) alive; dropping this
/// resource stops watching.
#[derive(Resource)]
struct ExternalFolderWatcher(#[allow(dead_code)] Debouncer<RecommendedWatcher, RecommendedCache>);

/// Receives every debounced event batch from the watcher thread. `pub(super)`
/// only because `add_systems(Update, watch::process_external_folder_events)`
/// in `mod.rs` needs the fn's own signature to be nameable from there —
/// nothing outside `assets_management` touches this type.
#[derive(Resource)]
pub(super) struct ExternalFolderEvents(Receiver<DebounceEventResult>);

/// Fired once per debounced batch of filesystem events under `~/Harmonicon`,
/// naming which immediate subfolders had a change fall under them. A
/// consumer checks `top_level_dirs.contains("songs")` (etc.) for the one it
/// owns and re-scans + fires its own specific `*Rescanned` message — see
/// `mod.rs`'s `SongsRescanned`/`ThemesRescanned` and `lessons::catalog`'s
/// `LessonsRescanned`. Message-driven rather than a bare resource-change
/// poll for the same reason those are: a menu page's own rebuild system only
/// runs while that page is open, so its change-detection tick goes stale
/// while closed and would misfire as "changed" on every re-entry (see
/// `CLAUDE.md`).
#[derive(Message)]
pub struct ExternalFolderChanged {
    pub top_level_dirs: HashSet<String>,
}

/// Starts watching `~/Harmonicon` recursively if it exists — every
/// external-content subfolder (`songs/`, `themes/`, `lessons/`, ...) lives
/// directly under it, so one watcher covers all of them.
pub(super) fn start_watching_external_folder(mut commands: Commands) {
    let Some(root) = dirs::home_dir().map(|h| h.join("Harmonicon")) else {
        return;
    };
    if !root.is_dir() {
        return;
    }

    let (tx, rx): (Sender<DebounceEventResult>, Receiver<DebounceEventResult>) =
        crossbeam_channel::unbounded();
    let mut debouncer = match new_debouncer(WATCH_DEBOUNCE, None, tx) {
        Ok(d) => d,
        Err(err) => {
            warn!("Could not start external-folder watcher: {err}");
            return;
        }
    };
    if let Err(err) = debouncer.watch(&root, RecursiveMode::Recursive) {
        warn!("Could not watch {}: {err}", root.display());
        return;
    }

    info!("Watching {} for content changes", root.display());
    commands.insert_resource(ExternalFolderWatcher(debouncer));
    commands.insert_resource(ExternalFolderEvents(rx));
}

/// Whether an event actually changed file *content* — as opposed to a mere
/// open/read/close or a metadata touch (permissions, atime). `notify`'s
/// inotify backend watches `IN_OPEN` (needed to catch atomic rename-into-place
/// saves), so **our own scan reading the very files it just watched** fires
/// `Access` events right back at the watcher; treating those as "changed"
/// would rescan, which opens the files again, which fires more `Access`
/// events — an unbounded self-triggering feedback loop. Pure so it's
/// unit-testable without a real filesystem watcher.
fn is_content_change(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Name(_))
    )
}

/// The set of `root`'s immediate subfolder names that any of `paths` falls
/// under — pure so it's unit-testable without a real filesystem watcher.
fn changed_top_level_dirs(root: &Path, paths: &[PathBuf]) -> HashSet<String> {
    paths
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok())
        .filter_map(|rel| rel.components().next())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect()
}

/// Drains the watcher's event channel each frame and fires one
/// [`ExternalFolderChanged`] naming every changed top-level subfolder. A
/// no-op system (returns immediately) if the watcher never started.
pub(super) fn process_external_folder_events(
    events: Option<Res<ExternalFolderEvents>>,
    mut changed: MessageWriter<ExternalFolderChanged>,
) {
    let Some(events) = events else { return };
    let Some(root) = dirs::home_dir().map(|h| h.join("Harmonicon")) else {
        return;
    };

    let mut top_level_dirs = HashSet::new();
    for result in events.0.try_iter() {
        match result {
            Ok(batch) => {
                let paths: Vec<PathBuf> = batch
                    .iter()
                    .filter(|e| is_content_change(&e.event.kind))
                    .flat_map(|e| e.event.paths.clone())
                    .collect();
                top_level_dirs.extend(changed_top_level_dirs(&root, &paths));
            }
            Err(errors) => {
                for err in errors {
                    warn!("Filesystem watch error: {err}");
                }
            }
        }
    }

    if !top_level_dirs.is_empty() {
        changed.write(ExternalFolderChanged { top_level_dirs });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify_debouncer_full::notify::event::{AccessKind, AccessMode, CreateKind, RemoveKind};

    #[test]
    fn a_file_open_is_not_a_content_change() {
        // The self-triggering-loop case: our own scan opening a chart file
        // must not read back as "the folder changed".
        assert!(!is_content_change(&EventKind::Access(AccessKind::Open(
            AccessMode::Any
        ))));
    }

    #[test]
    fn a_metadata_only_touch_is_not_a_content_change() {
        assert!(!is_content_change(&EventKind::Modify(
            ModifyKind::Metadata(notify_debouncer_full::notify::event::MetadataKind::Any)
        )));
    }

    #[test]
    fn create_modify_data_rename_and_remove_are_content_changes() {
        assert!(is_content_change(&EventKind::Create(CreateKind::File)));
        assert!(is_content_change(&EventKind::Modify(ModifyKind::Data(
            notify_debouncer_full::notify::event::DataChange::Any
        ))));
        assert!(is_content_change(&EventKind::Modify(ModifyKind::Name(
            notify_debouncer_full::notify::event::RenameMode::Any
        ))));
        assert!(is_content_change(&EventKind::Remove(RemoveKind::File)));
    }

    #[test]
    fn a_path_under_songs_names_the_songs_subfolder() {
        let root = Path::new("/home/x/Harmonicon");
        let paths = vec![root.join("songs/Artist/Song/song/chart.harpchart")];
        let dirs = changed_top_level_dirs(root, &paths);
        assert_eq!(dirs, HashSet::from(["songs".to_string()]));
    }

    #[test]
    fn a_path_under_themes_names_the_themes_subfolder() {
        let root = Path::new("/home/x/Harmonicon");
        let paths = vec![root.join("themes/mytheme/theme.json")];
        let dirs = changed_top_level_dirs(root, &paths);
        assert_eq!(dirs, HashSet::from(["themes".to_string()]));
    }

    #[test]
    fn a_path_under_lessons_names_the_lessons_subfolder() {
        let root = Path::new("/home/x/Harmonicon");
        let paths = vec![root.join("lessons/01_basics/01_first/lesson.json")];
        let dirs = changed_top_level_dirs(root, &paths);
        assert_eq!(dirs, HashSet::from(["lessons".to_string()]));
    }

    #[test]
    fn paths_under_several_subfolders_name_them_all() {
        let root = Path::new("/home/x/Harmonicon");
        let paths = vec![
            root.join("songs/Artist/Song/song/chart.harpchart"),
            root.join("themes/mytheme/theme.json"),
        ];
        let dirs = changed_top_level_dirs(root, &paths);
        assert_eq!(
            dirs,
            HashSet::from(["songs".to_string(), "themes".to_string()])
        );
    }

    #[test]
    fn a_path_outside_root_names_nothing() {
        let root = Path::new("/home/x/Harmonicon");
        let paths = vec![PathBuf::from("/home/x/somewhere-else.txt")];
        assert!(changed_top_level_dirs(root, &paths).is_empty());
    }

    #[test]
    fn no_paths_names_nothing() {
        let root = Path::new("/home/x/Harmonicon");
        assert!(changed_top_level_dirs(root, &[]).is_empty());
    }
}
