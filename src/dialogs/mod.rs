// SPDX-License-Identifier: MIT

//! Reusable in-app dialogs. Currently a navigable file picker, decoupled from
//! its callers via messages so any screen can request one.
//!
//! Open one by writing [`OpenFileDialog`] (with a [`DialogId`] you choose and an
//! optional extension filter); read the result by reading [`FileChosen`]
//! messages whose `purpose` matches your id. The dialog handles folder
//! navigation (into subfolders and up via "..") and closes on pick, Cancel, or
//! Esc.
//!
//! The UI is authored with the `bsn!` macro, and each clickable row carries its
//! behaviour as an `on(...)` / `observe(...)` callback rather than a polling
//! system.

use std::path::PathBuf;

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

/// Identifies who opened a dialog, so a caller only reacts to its own results.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DialogId(pub &'static str);

/// Request to open the file dialog.
#[derive(Message)]
pub struct OpenFileDialog {
    pub purpose: DialogId,
    pub title: String,
    /// Lowercase extensions to show (e.g. `["ogg", "mp3"]`); empty = all files.
    pub extensions: Vec<String>,
    /// Where to start browsing; defaults to the current directory.
    pub start_dir: Option<PathBuf>,
}

/// Emitted when the user picks a file.
#[derive(Message)]
pub struct FileChosen {
    pub purpose: DialogId,
    pub path: PathBuf,
}

/// Internal: request a rebuild of the entry list (on open or after navigating).
#[derive(Message)]
struct RefreshFileList;

/// Live dialog state. `open` lets callers suppress their own input (e.g. Esc)
/// while the modal is up.
#[derive(Resource, Default)]
pub struct FileDialog {
    pub open: bool,
    dir: PathBuf,
    extensions: Vec<String>,
    purpose: Option<DialogId>,
    title: String,
}

#[derive(Component, Default, Clone)]
struct FileDialogRoot;
#[derive(Component, Default, Clone)]
struct FileDialogList;
#[derive(Component, Default, Clone)]
struct DialogPathText;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
enum FileDialogState {
    #[default]
    Closed,
    Open,
}

const PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.11, 0.98);
const ENTRY_BG: Color = Color::srgba(0.14, 0.14, 0.20, 0.95);
const DIR_COLOR: Color = Color::srgb(0.55, 0.78, 1.0);
const FILE_COLOR: Color = Color::srgb(0.85, 0.85, 0.92);

/// Directories then matching files in `dir`, sorted, hidden entries skipped.
fn list_dir(dir: &std::path::Path, extensions: &[String]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                dirs.push(path);
            } else if extensions.is_empty()
                || path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
                    .unwrap_or(false)
            {
                files.push(path);
            }
        }
    }
    dirs.sort();
    files.sort();
    (dirs, files)
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Open the dialog when an [`OpenFileDialog`] arrives: set state and spawn the
/// `bsn!` modal shell. The entry list is filled by `refresh`.
fn handle_open(
    mut requests: MessageReader<OpenFileDialog>,
    mut dialog: ResMut<FileDialog>,
    mut next_state: ResMut<NextState<FileDialogState>>,
    mut refresh_req: MessageWriter<RefreshFileList>,
    mut commands: Commands,
) {
    let Some(req) = requests.read().last() else {
        return;
    };
    let start = req
        .start_dir
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("/"));
    dialog.open = true;
    dialog.dir = std::fs::canonicalize(&start).unwrap_or(start);
    dialog.extensions = req.extensions.clone();
    dialog.purpose = Some(req.purpose);
    dialog.title = req.title.clone();

    let title = dialog.title.clone();
    commands.spawn_scene(bsn! {
        Node {
            position_type: {PositionType::Absolute},
            left: {Val::Px(0.0)},
            top: {Val::Px(0.0)},
            width: {Val::Percent(100.0)},
            height: {Val::Percent(100.0)},
            flex_direction: {FlexDirection::Column},
            align_items: {AlignItems::Center},
            justify_content: {JustifyContent::Center},
            row_gap: {Val::Px(6.0)},
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.78)})
        GlobalZIndex(300)
        FileDialogRoot
        Children [
            (
                Text({title})
                TextFont { font_size: {FontSize::Px(18.0)} }
                TextColor({Color::WHITE})
            ),
            (
                Text({String::new()})
                TextFont { font_size: {FontSize::Px(12.0)} }
                TextColor({Color::srgb(0.6, 0.6, 0.7)})
                DialogPathText
            ),
            (
                Node {
                    flex_direction: {FlexDirection::Column},
                    row_gap: {Val::Px(2.0)},
                    width: {Val::Px(640.0)},
                    max_height: {Val::Percent(64.0)},
                    overflow: {Overflow::clip()},
                    padding: {UiRect::all(Val::Px(6.0))},
                }
                BackgroundColor({PANEL_BG})
                FileDialogList
            ),
            (
                Button
                Node {
                    padding: {UiRect::axes(Val::Px(14.0), Val::Px(5.0))},
                    margin: {UiRect::top(Val::Px(4.0))},
                }
                BackgroundColor({ENTRY_BG})
                on(|_: On<Pointer<Click>>,
                   mut dialog: ResMut<FileDialog>,
                   roots: Query<Entity, With<FileDialogRoot>>,
                   next: ResMut<NextState<FileDialogState>>,
                   mut commands: Commands| {
                    close(&mut dialog, &roots, next, &mut commands);
                })
                Children [
                    (
                        Text({"Cancel  (Esc)".to_string()})
                        TextFont { font_size: {FontSize::Px(13.0)} }
                        TextColor({Color::srgb(0.85, 0.7, 0.7)})
                    )
                ]
            ),
        ]
    });
    next_state.set(FileDialogState::Open);
    refresh_req.write(RefreshFileList);
}

/// Rebuild the entry list and path label, only when a [`RefreshFileList`] is
/// requested (on open or after navigating) — not every frame.
fn refresh(
    mut requests: MessageReader<RefreshFileList>,
    dialog: Res<FileDialog>,
    lists: Query<(Entity, Option<&Children>), With<FileDialogList>>,
    mut path_text: Query<&mut Text, With<DialogPathText>>,
    mut commands: Commands,
) {
    if requests.is_empty() {
        return;
    }
    requests.clear();

    if let Ok(mut text) = path_text.single_mut() {
        **text = dialog.dir.display().to_string();
    }

    let (dirs, files) = list_dir(&dialog.dir, &dialog.extensions);
    for (list, children) in &lists {
        if let Some(children) = children {
            for &c in children {
                commands.entity(c).despawn();
            }
        }
        commands.entity(list).with_children(|l| {
            if let Some(parent) = dialog.dir.parent() {
                spawn_dir_entry(l, "\u{1F4C1} ..".to_string(), parent.to_path_buf());
            }
            for d in &dirs {
                spawn_dir_entry(l, format!("\u{1F4C1} {}", file_name(d)), d.clone());
            }
            for f in &files {
                spawn_file_entry(l, file_name(f), f.clone());
            }
        });
    }
}

/// A folder row: `bsn!` widget + an `observe` callback that navigates into it.
fn spawn_dir_entry(parent: &mut ChildSpawnerCommands, label: String, path: PathBuf) {
    parent
        .spawn_empty()
        .apply_scene(entry_scene(label, DIR_COLOR))
        .observe(
            move |_: On<Pointer<Click>>,
                  mut dialog: ResMut<FileDialog>,
                  mut refresh_req: MessageWriter<RefreshFileList>| {
                dialog.dir = path.clone();
                refresh_req.write(RefreshFileList);
            },
        );
}

/// A file row: `bsn!` widget + an `observe` callback that picks it and closes.
fn spawn_file_entry(parent: &mut ChildSpawnerCommands, label: String, path: PathBuf) {
    parent
        .spawn_empty()
        .apply_scene(entry_scene(label, FILE_COLOR))
        .observe(
            move |_: On<Pointer<Click>>,
                  mut dialog: ResMut<FileDialog>,
                  mut chosen: MessageWriter<FileChosen>,
                  roots: Query<Entity, With<FileDialogRoot>>,
                  next: ResMut<NextState<FileDialogState>>,
                  mut commands: Commands| {
                if let Some(purpose) = dialog.purpose {
                    chosen.write(FileChosen { purpose, path: path.clone() });
                }
                close(&mut dialog, &roots, next, &mut commands);
            },
        );
}

/// The shared `bsn!` scene for one clickable entry row. (Uses the default font:
/// `TextFont.font` can't be set through `bsn!` in 0.19.)
fn entry_scene(label: String, color: Color) -> impl Scene {
    bsn! {
        Button
        Node {
            width: {Val::Percent(100.0)},
            padding: {UiRect::axes(Val::Px(8.0), Val::Px(3.0))},
        }
        BackgroundColor({ENTRY_BG})
        Children [
            (
                Text({label})
                TextFont { font_size: {FontSize::Px(14.0)} }
                TextColor({color})
            )
        ]
    }
}

/// Esc cancels; Backspace goes up a folder. Esc is consumed so callers behind
/// the modal don't also act on it.
fn dialog_keys(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut dialog: ResMut<FileDialog>,
    roots: Query<Entity, With<FileDialogRoot>>,
    next_state: ResMut<NextState<FileDialogState>>,
    mut refresh_req: MessageWriter<RefreshFileList>,
    mut commands: Commands,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        keyboard.clear_just_pressed(KeyCode::Escape);
        close(&mut dialog, &roots, next_state, &mut commands);
    } else if keyboard.just_pressed(KeyCode::Backspace) {
        if let Some(parent) = dialog.dir.parent() {
            dialog.dir = parent.to_path_buf();
            refresh_req.write(RefreshFileList);
        }
    }
}

fn close(
    dialog: &mut FileDialog,
    roots: &Query<Entity, With<FileDialogRoot>>,
    mut next_state: ResMut<NextState<FileDialogState>>,
    commands: &mut Commands,
) {
    dialog.open = false;
    dialog.purpose = None;
    for e in roots {
        commands.entity(e).despawn();
    }
    next_state.set(FileDialogState::Closed);
}

pub struct DialogsPlugin;

impl Plugin for DialogsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OpenFileDialog>()
            .add_message::<FileChosen>()
            .add_message::<RefreshFileList>()
            .init_resource::<FileDialog>()
            .init_state::<FileDialogState>()
            .add_systems(Update, handle_open.run_if(in_state(FileDialogState::Closed)))
            .add_systems(
                Update,
                (refresh, dialog_keys).run_if(in_state(FileDialogState::Open)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_dir_splits_and_filters() {
        // The repo's assets/songs has subfolders and (deeper) ogg files.
        let (dirs, files) = list_dir(std::path::Path::new("assets/songs"), &["ogg".into()]);
        assert!(!dirs.is_empty(), "expected artist subfolders");
        // No ogg directly in assets/songs (they're under <song>/song/).
        assert!(files.iter().all(|f| f.extension().and_then(|e| e.to_str()) == Some("ogg")));
    }

    #[test]
    fn list_dir_filter_is_case_insensitive_and_skips_hidden() {
        let (dirs, _) = list_dir(std::path::Path::new("assets"), &[]);
        assert!(dirs.iter().all(|d| !file_name(d).starts_with('.')));
    }
}
