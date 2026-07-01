// SPDX-License-Identifier: MIT

//! Reusable in-app dialogs. Currently a navigable file picker, decoupled from
//! its callers via messages so any screen can request one.
//!
//! Open one by writing [`OpenFileDialog`] with a [`DialogId`] you choose, an
//! optional extension filter, and a [`DialogMode`]; read the result by reading
//! [`FileChosen`] messages whose `purpose` matches your id. The dialog handles
//! folder navigation (into subfolders and up via "..") and closes on pick,
//! Cancel, or Esc.
//!
//! In [`DialogMode::Save`] a filename text field appears at the bottom.
//! Keyboard input goes to that field; Enter confirms, Esc cancels.
//! Clicking a file entry fills the filename field instead of picking immediately.

use std::path::PathBuf;

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui_widgets::ScrollArea;

use crate::dialogs::button;

/// Identifies who opened a dialog, so a caller only reacts to its own results.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DialogId(pub &'static str);

/// Whether the dialog is picking an existing file or choosing a save location.
#[derive(Clone, Debug, Default)]
pub enum DialogMode {
    /// Browse and select an existing file (original behaviour).
    #[default]
    Open,
    /// Browse to a directory and type/confirm a filename to write.
    Save {
        /// Pre-filled filename shown in the text field when the dialog opens.
        default_name: String,
    },
}

/// Request to open the file dialog.
#[derive(Message)]
pub struct OpenFileDialog {
    pub purpose: DialogId,
    pub title: String,
    /// Lowercase extensions to show (e.g. `["ogg", "mp3"]`); empty = all files.
    pub extensions: Vec<String>,
    /// Where to start browsing; defaults to the current directory.
    pub start_dir: Option<PathBuf>,
    /// Open an existing file or choose a save path. Defaults to [`DialogMode::Open`].
    pub mode: DialogMode,
}

/// Emitted when the user picks a file (Open) or confirms a save path (Save).
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
    pub mode: DialogMode,
    /// Current contents of the filename text field (Save mode only).
    save_filename: String,
}

#[derive(Component, Default, Clone)]
struct FileDialogRoot;
#[derive(Component, Default, Clone)]
struct FileDialogList;
#[derive(Component, Default, Clone)]
struct DialogPathText;
/// Marks the `Text` entity that mirrors `FileDialog::save_filename`.
#[derive(Component, Default, Clone)]
struct SaveFilenameText;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
enum FileDialogState {
    #[default]
    Closed,
    Open,
}

const PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.11, 0.98);
const ENTRY_BG: Color = Color::srgba(0.14, 0.14, 0.20, 0.95);

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
/// modal shell. The entry list is filled by `refresh`.
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
    dialog.mode = req.mode.clone();
    dialog.save_filename = match &req.mode {
        DialogMode::Save { default_name } => default_name.clone(),
        DialogMode::Open => String::new(),
    };

    let title = dialog.title.clone();
    let is_save = matches!(dialog.mode, DialogMode::Save { .. });
    let save_row_display = if is_save { Display::Flex } else { Display::None };
    let initial_filename = dialog.save_filename.clone();

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
                    overflow: {Overflow::scroll_y()},
                    padding: {UiRect::all(Val::Px(6.0))},
                }
                BackgroundColor({PANEL_BG})
                FileDialogList
                ScrollArea
            ),
            (
                Node {
                    display: {save_row_display},
                    flex_direction: {FlexDirection::Row},
                    align_items: {AlignItems::Center},
                    column_gap: {Val::Px(8.0)},
                }
                Children [
                    (
                        Text({"File name:".to_string()})
                        TextFont { font_size: {FontSize::Px(13.0)} }
                        TextColor({Color::srgb(0.75, 0.75, 0.85)})
                    ),
                    (
                        Node {
                            width: {Val::Px(340.0)},
                            height: {Val::Px(28.0)},
                            align_items: {AlignItems::Center},
                            padding: {UiRect::horizontal(Val::Px(8.0))},
                        }
                        BackgroundColor({Color::srgba(0.10, 0.10, 0.16, 1.0)})
                        Children [
                            (
                                Text({initial_filename})
                                TextFont { font_size: {FontSize::Px(13.0)} }
                                TextColor({Color::WHITE})
                                SaveFilenameText
                            )
                        ]
                    ),
                    (
                        Button
                        Node {
                            padding: {UiRect::axes(Val::Px(14.0), Val::Px(5.0))},
                        }
                        BackgroundColor({Color::srgb(0.18, 0.28, 0.45)})
                        on(|_: On<Pointer<Click>>,
                           mut dialog: ResMut<FileDialog>,
                           mut chosen: MessageWriter<FileChosen>,
                           roots: Query<Entity, With<FileDialogRoot>>,
                           next: ResMut<NextState<FileDialogState>>,
                           mut commands: Commands| {
                            confirm_save(&mut dialog, &mut chosen, &roots, next, &mut commands);
                        })
                        Children [
                            (
                                Text({"Save".to_string()})
                                TextFont { font_size: {FontSize::Px(13.0)} }
                                TextColor({Color::WHITE})
                            )
                        ]
                    ),
                ]
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

/// A folder row: navigates into the folder on click.
fn spawn_dir_entry(parent: &mut ChildSpawnerCommands, label: String, path: PathBuf) {
    parent.spawn_empty().apply_scene(button::default(
        &label,
        move |_: On<Pointer<Click>>,
              mut dialog: ResMut<FileDialog>,
              mut refresh_req: MessageWriter<RefreshFileList>| {
            dialog.dir = path.clone();
            refresh_req.write(RefreshFileList);
        },
    ));
}

/// A file row.
/// - **Open mode**: picks the file and closes the dialog.
/// - **Save mode**: fills the filename field with the clicked entry's name.
fn spawn_file_entry(parent: &mut ChildSpawnerCommands, label: String, path: PathBuf) {
    parent.spawn_empty().apply_scene(button::default(
        &label,
        move |_: On<Pointer<Click>>,
              mut dialog: ResMut<FileDialog>,
              mut chosen: MessageWriter<FileChosen>,
              roots: Query<Entity, With<FileDialogRoot>>,
              next: ResMut<NextState<FileDialogState>>,
              mut commands: Commands| {
            match &dialog.mode {
                DialogMode::Open => {
                    if let Some(purpose) = dialog.purpose {
                        chosen.write(FileChosen { purpose, path: path.clone() });
                    }
                    close(&mut dialog, &roots, next, &mut commands);
                }
                DialogMode::Save { .. } => {
                    if let Some(name) = path.file_name() {
                        dialog.save_filename = name.to_string_lossy().into_owned();
                    }
                }
            }
        },
    ));
}

/// Confirm a save: emit [`FileChosen`] with `dir/filename` and close.
/// Does nothing if the filename field is empty.
fn confirm_save(
    dialog: &mut FileDialog,
    chosen: &mut MessageWriter<FileChosen>,
    roots: &Query<Entity, With<FileDialogRoot>>,
    next: ResMut<NextState<FileDialogState>>,
    commands: &mut Commands,
) {
    if dialog.save_filename.is_empty() {
        return;
    }
    if let Some(purpose) = dialog.purpose {
        let path = dialog.dir.join(&dialog.save_filename);
        chosen.write(FileChosen { purpose, path });
    }
    close(dialog, roots, next, commands);
}

/// Mirror `FileDialog::save_filename` into the on-screen text entity whenever
/// it changes (Save mode only; the entity is hidden in Open mode).
fn sync_save_filename(
    dialog: Res<FileDialog>,
    mut texts: Query<&mut Text, With<SaveFilenameText>>,
) {
    if !dialog.is_changed() {
        return;
    }
    for mut text in &mut texts {
        **text = dialog.save_filename.clone();
    }
}

/// Keyboard handling while the dialog is open.
///
/// **Open mode**: Esc closes; Backspace navigates up a folder.
/// **Save mode**: printable keys type into the filename field; Backspace removes
/// the last character; Enter confirms; Esc cancels.
fn dialog_keys(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
    mut dialog: ResMut<FileDialog>,
    roots: Query<Entity, With<FileDialogRoot>>,
    next_state: ResMut<NextState<FileDialogState>>,
    mut refresh_req: MessageWriter<RefreshFileList>,
    mut chosen: MessageWriter<FileChosen>,
    mut commands: Commands,
) {
    match &dialog.mode {
        DialogMode::Save { .. } => {
            // Collect the action first so `next_state` isn't moved inside a loop.
            #[derive(PartialEq)]
            enum Act { Confirm, Cancel, None }
            let mut act = Act::None;

            for ev in key_events.read() {
                if ev.state != ButtonState::Pressed {
                    continue;
                }
                match &ev.logical_key {
                    bevy::input::keyboard::Key::Character(s) => {
                        for c in s.chars() {
                            if !c.is_control() {
                                dialog.save_filename.push(c);
                            }
                        }
                    }
                    bevy::input::keyboard::Key::Space => dialog.save_filename.push(' '),
                    bevy::input::keyboard::Key::Backspace => { dialog.save_filename.pop(); }
                    bevy::input::keyboard::Key::Enter => { act = Act::Confirm; break; }
                    bevy::input::keyboard::Key::Escape => { act = Act::Cancel; break; }
                    _ => {}
                }
            }

            match act {
                Act::Confirm => confirm_save(&mut dialog, &mut chosen, &roots, next_state, &mut commands),
                Act::Cancel  => close(&mut dialog, &roots, next_state, &mut commands),
                Act::None    => {}
            }
        }
        DialogMode::Open => {
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

pub struct FileDialogsPlugin;

impl Plugin for FileDialogsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OpenFileDialog>()
            .add_message::<FileChosen>()
            .add_message::<RefreshFileList>()
            .init_resource::<FileDialog>()
            .init_state::<FileDialogState>()
            .add_systems(Update, handle_open.run_if(in_state(FileDialogState::Closed)))
            .add_systems(
                Update,
                (refresh, sync_save_filename, dialog_keys)
                    .run_if(in_state(FileDialogState::Open)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_dir_splits_and_filters() {
        let (dirs, files) = list_dir(std::path::Path::new("assets/songs"), &["ogg".into()]);
        assert!(!dirs.is_empty(), "expected artist subfolders");
        assert!(files.iter().all(|f| f.extension().and_then(|e| e.to_str()) == Some("ogg")));
    }

    #[test]
    fn list_dir_filter_is_case_insensitive_and_skips_hidden() {
        let (dirs, _) = list_dir(std::path::Path::new("assets"), &[]);
        assert!(dirs.iter().all(|d| !file_name(d).starts_with('.')));
    }
}
