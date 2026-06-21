// SPDX-License-Identifier: MIT

//! Song authoring tool, launched from the main menu (`AppState::SongEditor`).
//!
//! Step 2 builds the metadata form: artist, song name, a music-file picker,
//! tempo, beats-per-bar, the harmonica key, and a 12-bar blues preview. Text
//! fields are edited in place (click to focus, type, backspace); the music
//! picker is a small in-app browser that scans common folders for ogg/mp3 so we
//! don't depend on a native dialog. Later steps add audio analysis, note
//! editing in the grid, and saving to a `.harpchart`.

use std::path::{Path, PathBuf};

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};

use crate::assets_management::GlobalFonts;
use crate::song::harmonica::twelve_bar;

use super::AppState;

// ── Model ───────────────────────────────────────────────────────────────────

/// The song being authored. Strings are kept as typed text and parsed on save.
#[derive(Resource)]
pub struct SongEditorData {
    pub artist: String,
    pub song_name: String,
    pub music_path: Option<PathBuf>,
    pub tempo_bpm: String,
    pub beats_per_bar: String,
    pub harp_key: String,
}

impl Default for SongEditorData {
    fn default() -> Self {
        Self {
            artist: String::new(),
            song_name: String::new(),
            music_path: None,
            tempo_bpm: "120".into(),
            beats_per_bar: "4".into(),
            harp_key: "C".into(),
        }
    }
}

/// The four free-text fields. (Harp key cycles; music path is picked.)
#[derive(Clone, Copy, PartialEq, Eq)]
enum TextFieldId {
    Artist,
    SongName,
    Tempo,
    BeatsPerBar,
}

impl TextFieldId {
    fn value_mut<'a>(&self, d: &'a mut SongEditorData) -> &'a mut String {
        match self {
            TextFieldId::Artist => &mut d.artist,
            TextFieldId::SongName => &mut d.song_name,
            TextFieldId::Tempo => &mut d.tempo_bpm,
            TextFieldId::BeatsPerBar => &mut d.beats_per_bar,
        }
    }
    fn value<'a>(&self, d: &'a SongEditorData) -> &'a str {
        match self {
            TextFieldId::Artist => &d.artist,
            TextFieldId::SongName => &d.song_name,
            TextFieldId::Tempo => &d.tempo_bpm,
            TextFieldId::BeatsPerBar => &d.beats_per_bar,
        }
    }
}

/// Which text field currently receives typing.
#[derive(Resource, Default)]
struct FocusedField(Option<TextFieldId>);

/// The in-flight tempo-analysis task, if a file is being analysed.
#[derive(Resource, Default)]
struct TempoTask(Option<Task<Option<f32>>>);

/// The 12 chromatic keys, cycled by the harp-key button.
const KEYS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn next_key(current: &str) -> String {
    let i = KEYS.iter().position(|&k| k == current).unwrap_or(0);
    KEYS[(i + 1) % KEYS.len()].to_string()
}

// ── Markers ─────────────────────────────────────────────────────────────────

#[derive(Component)]
struct SongEditorRoot;
#[derive(Component)]
struct TextFieldBox(TextFieldId);
#[derive(Component)]
struct TextFieldText(TextFieldId);
#[derive(Component)]
struct HarpKeyButton;
#[derive(Component)]
struct HarpKeyText;
#[derive(Component)]
struct MusicPickButton;
#[derive(Component)]
struct MusicPathText;
#[derive(Component)]
struct TwelveBarGrid;
#[derive(Component)]
struct FileBrowserRoot;
#[derive(Component)]
struct FileEntryButton(PathBuf);
#[derive(Component)]
struct AnalyzeStatusText;

// ── Colours ─────────────────────────────────────────────────────────────────

const FIELD_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.95);
const FIELD_BG_FOCUS: Color = Color::srgba(0.16, 0.16, 0.24, 1.0);
const BTN_BG: Color = Color::srgba(0.14, 0.14, 0.20, 0.95);
const ACCENT: Color = Color::srgb(0.95, 0.80, 0.35);
const LABEL: Color = Color::srgb(0.75, 0.75, 0.82);

// ── Setup ───────────────────────────────────────────────────────────────────

fn setup(mut commands: Commands, fonts: Res<GlobalFonts>, data: Res<SongEditorData>) {
    let font = fonts.gameplay.clone();

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.06, 0.06, 0.09)),
            SongEditorRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Song Editor"),
                TextFont { font_size: FontSize::Px(34.0), font: font.clone(), ..default() },
                TextColor(Color::WHITE),
            ));

            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                min_width: Val::Px(540.0),
                ..default()
            })
            .with_children(|form| {
                text_field(form, &font, "Artist", TextFieldId::Artist, &data.artist);
                text_field(form, &font, "Song Name", TextFieldId::SongName, &data.song_name);
                music_field(form, &font, &data.music_path);
                text_field(form, &font, "Music Tempo  \u{2669} =", TextFieldId::Tempo, &data.tempo_bpm);
                text_field(form, &font, "Beats per Bar", TextFieldId::BeatsPerBar, &data.beats_per_bar);
                harp_field(form, &font, &data.harp_key);
            });

            root.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
                TwelveBarGrid,
            ))
            .with_children(|grid| {
                build_twelve_bar_cells(grid, &twelve_bar(&data.harp_key), &font);
            });

            root.spawn((
                Text::new(String::new()),
                TextFont { font_size: FontSize::Px(13.0), font: font.clone(), ..default() },
                TextColor(ACCENT),
                AnalyzeStatusText,
            ));

            root.spawn((
                Text::new("Click a field to edit  \u{00B7}  Esc to go back"),
                TextFont { font_size: FontSize::Px(13.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));
        });
}

fn text_field(
    parent: &mut ChildSpawnerCommands,
    font: &FontSource,
    label: &str,
    id: TextFieldId,
    initial: &str,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node { width: Val::Px(160.0), ..default() },
                Text::new(format!("{label}:")),
                TextFont { font_size: FontSize::Px(15.0), font: font.clone(), ..default() },
                TextColor(LABEL),
            ));
            row.spawn((
                Button,
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(260.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(FIELD_BG),
                BorderColor::all(Color::srgb(0.30, 0.30, 0.42)),
                TextFieldBox(id),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(initial.to_string()),
                    TextFont { font_size: FontSize::Px(15.0), font: font.clone(), ..default() },
                    TextColor(Color::WHITE),
                    TextFieldText(id),
                ));
            });
        });
}

fn music_field(parent: &mut ChildSpawnerCommands, font: &FontSource, path: &Option<PathBuf>) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node { width: Val::Px(160.0), ..default() },
                Text::new("Music Background:"),
                TextFont { font_size: FontSize::Px(15.0), font: font.clone(), ..default() },
                TextColor(LABEL),
            ));
            row.spawn((
                Node { flex_grow: 1.0, min_width: Val::Px(180.0), ..default() },
                Text::new(music_label(path)),
                TextFont { font_size: FontSize::Px(14.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.9)),
                MusicPathText,
            ));
            row.spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(BTN_BG),
                BorderColor::all(Color::srgb(0.35, 0.35, 0.50)),
                MusicPickButton,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Browse\u{2026}"),
                    TextFont { font_size: FontSize::Px(14.0), font: font.clone(), ..default() },
                    TextColor(ACCENT),
                ));
            });
        });
}

fn harp_field(parent: &mut ChildSpawnerCommands, font: &FontSource, key: &str) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node { width: Val::Px(160.0), ..default() },
                Text::new("Harmonica Key:"),
                TextFont { font_size: FontSize::Px(15.0), font: font.clone(), ..default() },
                TextColor(LABEL),
            ));
            row.spawn((
                Button,
                Node {
                    width: Val::Px(70.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(BTN_BG),
                BorderColor::all(Color::srgb(0.35, 0.35, 0.50)),
                HarpKeyButton,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(key.to_string()),
                    TextFont { font_size: FontSize::Px(16.0), font: font.clone(), ..default() },
                    TextColor(ACCENT),
                    HarpKeyText,
                ));
            });
            row.spawn((
                Text::new("(click to cycle)"),
                TextFont { font_size: FontSize::Px(12.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.6)),
            ));
        });
}

/// Spawn the 12 bar cells with their I/IV/V chord labels.
fn build_twelve_bar_cells(parent: &mut ChildSpawnerCommands, chords: &[String], font: &FontSource) {
    for (i, chord) in chords.iter().enumerate() {
        parent
            .spawn((
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(56.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.11, 0.15, 0.95)),
                BorderColor::all(Color::srgb(0.30, 0.32, 0.42)),
            ))
            .with_children(|cell| {
                cell.spawn((
                    Text::new(format!("{}", i + 1)),
                    TextFont { font_size: FontSize::Px(9.0), font: font.clone(), ..default() },
                    TextColor(Color::srgb(0.45, 0.45, 0.55)),
                ));
                cell.spawn((
                    Text::new(chord.clone()),
                    TextFont { font_size: FontSize::Px(17.0), font: font.clone(), ..default() },
                    TextColor(Color::WHITE),
                ));
            });
    }
}

// ── Tempo analysis ────────────────────────────────────────────────────────────

/// Decode `path` (ogg) to mono and estimate its tempo in BPM. Returns `None` if
/// the file can't be decoded or no clear tempo is found, so the caller can fall
/// back to manual entry. Runs on a background task — it decodes the whole file.
fn analyze_tempo(path: &Path) -> Option<f32> {
    use rodio::Source;
    let file = std::fs::File::open(path).ok()?;
    let decoder = rodio::Decoder::try_from(file).ok()?;
    let sample_rate = decoder.sample_rate().get() as f32;
    let channels = decoder.channels().get() as usize;
    if channels == 0 {
        return None;
    }

    // Downmix to mono, capped to ~90s — plenty for a steady tempo.
    let cap = (sample_rate as usize) * channels * 90;
    let mut mono = Vec::new();
    let mut acc = 0.0f32;
    let mut c = 0usize;
    for (i, s) in decoder.enumerate() {
        if i >= cap {
            break;
        }
        acc += s;
        c += 1;
        if c == channels {
            mono.push(acc / channels as f32);
            acc = 0.0;
            c = 0;
        }
    }
    estimate_bpm(&mono, sample_rate)
}

/// Autocorrelation tempo estimate over an onset-energy envelope. Pure, so it can
/// be unit-tested on synthetic signals.
fn estimate_bpm(mono: &[f32], sample_rate: f32) -> Option<f32> {
    const HOP: usize = 512;
    let n_frames = mono.len() / HOP;
    if n_frames < 32 || sample_rate <= 0.0 {
        return None;
    }

    // Per-hop energy, then a half-wave-rectified difference = onset envelope.
    let energy: Vec<f32> = (0..n_frames)
        .map(|f| mono[f * HOP..(f + 1) * HOP].iter().map(|x| x * x).sum())
        .collect();
    let mut onset: Vec<f32> = std::iter::once(0.0)
        .chain((1..n_frames).map(|i| (energy[i] - energy[i - 1]).max(0.0)))
        .collect();
    let mean = onset.iter().sum::<f32>() / onset.len() as f32;
    if mean <= f32::EPSILON {
        return None; // silence / no onsets
    }
    for v in &mut onset {
        *v -= mean;
    }

    let frame_rate = sample_rate / HOP as f32; // envelope frames per second
    let lag_min = (frame_rate * 60.0 / 200.0).floor().max(1.0) as usize;
    let lag_max = ((frame_rate * 60.0 / 50.0).ceil() as usize).min(onset.len() / 2);
    if lag_min >= lag_max {
        return None;
    }

    let mut best_lag = 0usize;
    let mut best = f32::MIN;
    let mut total = 0.0f32;
    let mut count = 0u32;
    for lag in lag_min..=lag_max {
        let sum: f32 = (lag..onset.len()).map(|i| onset[i] * onset[i - lag]).sum();
        total += sum;
        count += 1;
        if sum > best {
            best = sum;
            best_lag = lag;
        }
    }
    // Require a peak clearly above the average autocorrelation, else "no tempo".
    let avg = total / count.max(1) as f32;
    if best_lag == 0 || best <= avg * 1.5 {
        return None;
    }

    let mut bpm = 60.0 * frame_rate / best_lag as f32;
    while bpm < 70.0 {
        bpm *= 2.0;
    }
    while bpm > 180.0 {
        bpm /= 2.0;
    }
    Some(bpm)
}

fn music_label(path: &Option<PathBuf>) -> String {
    match path {
        Some(p) => p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.to_string_lossy().to_string()),
        None => "(none selected)".to_string(),
    }
}

// ── Interaction: focus + typing ───────────────────────────────────────────────

/// Clicking a text field focuses it for typing.
fn focus_clicks(
    boxes: Query<(&Interaction, &TextFieldBox), Changed<Interaction>>,
    mut focused: ResMut<FocusedField>,
) {
    for (interaction, field) in &boxes {
        if *interaction == Interaction::Pressed {
            focused.0 = Some(field.0);
        }
    }
}

/// Route typed characters into the focused field. No-op while a field isn't
/// focused (e.g. the file browser is open and clears focus).
fn type_into_focused(
    mut keys: MessageReader<KeyboardInput>,
    focused: Res<FocusedField>,
    mut data: ResMut<SongEditorData>,
) {
    let Some(field) = focused.0 else {
        keys.clear();
        return;
    };
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let value = field.value_mut(&mut data);
        match &ev.logical_key {
            Key::Backspace => {
                value.pop();
            }
            Key::Space => value.push(' '),
            Key::Character(s) => {
                for c in s.chars() {
                    if !c.is_control() {
                        value.push(c);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Mirror the model into the field texts (with a caret on the focused one) and
/// highlight the focused box.
fn update_field_views(
    data: Res<SongEditorData>,
    focused: Res<FocusedField>,
    mut texts: Query<(&TextFieldText, &mut Text)>,
    mut boxes: Query<(&TextFieldBox, &mut BackgroundColor)>,
    mut music: Query<&mut Text, (With<MusicPathText>, Without<TextFieldText>)>,
) {
    for (field, mut text) in &mut texts {
        let mut s = field.0.value(&data).to_string();
        if focused.0 == Some(field.0) {
            s.push('_');
        }
        **text = s;
    }
    for (field, mut bg) in &mut boxes {
        bg.0 = if focused.0 == Some(field.0) { FIELD_BG_FOCUS } else { FIELD_BG };
    }
    if let Ok(mut text) = music.single_mut() {
        **text = music_label(&data.music_path);
    }
}

// ── Interaction: harmonica key ────────────────────────────────────────────────

/// Cycle the harp key on click, update its label, and rebuild the 12-bar grid.
fn harp_key_clicks(
    interactions: Query<&Interaction, (Changed<Interaction>, With<HarpKeyButton>)>,
    mut data: ResMut<SongEditorData>,
    fonts: Res<GlobalFonts>,
    mut key_texts: Query<&mut Text, With<HarpKeyText>>,
    grids: Query<(Entity, Option<&Children>), With<TwelveBarGrid>>,
    mut commands: Commands,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    data.harp_key = next_key(&data.harp_key);
    for mut t in &mut key_texts {
        **t = data.harp_key.clone();
    }
    let chords = twelve_bar(&data.harp_key);
    for (grid, children) in &grids {
        if let Some(children) = children {
            for &c in children {
                commands.entity(c).despawn();
            }
        }
        commands.entity(grid).with_children(|g| {
            build_twelve_bar_cells(g, &chords, &fonts.gameplay);
        });
    }
}

// ── File browser ──────────────────────────────────────────────────────────────

/// Recursively collect up to `limit` ogg/mp3 files under `dir` (depth-bounded,
/// skipping hidden folders), appending to `out`.
fn collect_audio(dir: &std::path::Path, depth: u8, limit: usize, out: &mut Vec<PathBuf>) {
    if out.len() >= limit || depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= limit {
            return;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_audio(&path, depth - 1, limit, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("ogg" | "mp3")
        ) {
            out.push(path);
        }
    }
}

/// Scan a few common locations for audio files to offer in the picker.
fn scan_audio_files() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = vec![PathBuf::from("assets"), PathBuf::from(".")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Music"));
        roots.push(home);
    }
    let mut found = Vec::new();
    for root in roots {
        collect_audio(&root, 3, 120, &mut found);
    }
    found.sort();
    found.dedup();
    found
}

/// Open the file browser overlay when Browse is clicked (if not already open).
fn open_browser(
    interactions: Query<&Interaction, (Changed<Interaction>, With<MusicPickButton>)>,
    open: Query<Entity, With<FileBrowserRoot>>,
    mut focused: ResMut<FocusedField>,
    fonts: Res<GlobalFonts>,
    mut commands: Commands,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) || !open.is_empty() {
        return;
    }
    focused.0 = None;
    let font = fonts.gameplay.clone();
    let files = scan_audio_files();

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.82)),
            GlobalZIndex(200),
            FileBrowserRoot,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(if files.is_empty() {
                    "No .ogg/.mp3 files found in assets, ., or your Music folder"
                } else {
                    "Select a music file  (Esc to cancel)"
                }),
                TextFont { font_size: FontSize::Px(16.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.9)),
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    max_height: Val::Percent(75.0),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|list| {
                    for path in files {
                        list.spawn((
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(BTN_BG),
                            FileEntryButton(path.clone()),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new(path.to_string_lossy().to_string()),
                                TextFont { font_size: FontSize::Px(13.0), font: font.clone(), ..default() },
                                TextColor(Color::srgb(0.8, 0.85, 0.95)),
                            ));
                        });
                    }
                });
        });
}

/// Pick a file from the browser: set the path and close the overlay.
fn pick_file(
    entries: Query<(&Interaction, &FileEntryButton), Changed<Interaction>>,
    browser: Query<Entity, With<FileBrowserRoot>>,
    mut data: ResMut<SongEditorData>,
    mut task: ResMut<TempoTask>,
    mut status: Query<&mut Text, With<AnalyzeStatusText>>,
    mut commands: Commands,
) {
    for (interaction, entry) in &entries {
        if *interaction == Interaction::Pressed {
            data.music_path = Some(entry.0.clone());
            for e in &browser {
                commands.entity(e).despawn();
            }
            // Kick off background tempo analysis (Step 3).
            let path = entry.0.clone();
            let pool = AsyncComputeTaskPool::get();
            task.0 = Some(pool.spawn(async move { analyze_tempo(&path) }));
            if let Ok(mut text) = status.single_mut() {
                **text = "Analyzing tempo\u{2026}".to_string();
            }
            return;
        }
    }
}

/// Poll the background tempo analysis; on success fill the tempo field, else
/// leave it for manual entry. The status line reflects the outcome.
fn poll_tempo(
    mut task: ResMut<TempoTask>,
    mut data: ResMut<SongEditorData>,
    mut status: Query<&mut Text, With<AnalyzeStatusText>>,
) {
    let Some(t) = task.0.as_mut() else {
        return;
    };
    let Some(result) = future::block_on(future::poll_once(t)) else {
        return; // still running
    };
    task.0 = None;
    let msg = match result {
        Some(bpm) => {
            let bpm = bpm.round() as u32;
            data.tempo_bpm = bpm.to_string();
            format!("Tempo auto-detected: {bpm} BPM (edit if wrong)")
        }
        None => "Couldn't detect tempo \u{2014} enter it manually".to_string(),
    };
    if let Ok(mut text) = status.single_mut() {
        **text = msg;
    }
}

// ── Escape / lifecycle ─────────────────────────────────────────────────────────

/// Esc: close the browser if open, else blur a focused field, else go back.
fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    browser: Query<Entity, With<FileBrowserRoot>>,
    mut focused: ResMut<FocusedField>,
    mut next_state: ResMut<NextState<AppState>>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    if let Some(e) = browser.iter().next() {
        commands.entity(e).despawn();
    } else if focused.0.is_some() {
        focused.0 = None;
    } else {
        next_state.set(AppState::Menu);
    }
}

fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, Or<(With<SongEditorRoot>, With<FileBrowserRoot>)>>,
    mut focused: ResMut<FocusedField>,
    mut task: ResMut<TempoTask>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    focused.0 = None;
    task.0 = None;
}

pub struct SongEditorPlugin;

impl Plugin for SongEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SongEditorData>()
            .init_resource::<FocusedField>()
            .init_resource::<TempoTask>()
            .add_systems(OnEnter(AppState::SongEditor), setup)
            .add_systems(OnExit(AppState::SongEditor), cleanup)
            .add_systems(
                Update,
                (
                    handle_escape,
                    focus_clicks,
                    type_into_focused,
                    harp_key_clicks,
                    open_browser,
                    pick_file,
                    poll_tempo,
                    update_field_views,
                )
                    .run_if(in_state(AppState::SongEditor)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_key_cycles_through_all_twelve() {
        assert_eq!(next_key("C"), "C#");
        assert_eq!(next_key("B"), "C");
        // Cycling 12 times returns to start.
        let mut k = "C".to_string();
        for _ in 0..12 {
            k = next_key(&k);
        }
        assert_eq!(k, "C");
    }

    #[test]
    fn music_label_shows_file_name_or_placeholder() {
        assert_eq!(music_label(&None), "(none selected)");
        assert_eq!(
            music_label(&Some(PathBuf::from("/a/b/song.ogg"))),
            "song.ogg"
        );
    }

    #[test]
    fn estimate_bpm_finds_a_click_train_tempo() {
        let sr = 44100.0;
        let period = (60.0 / 120.0 * sr) as usize; // 120 BPM → 22050 samples
        let n = period * 20;
        let mut sig = vec![0.0f32; n];
        for beat in 0..20 {
            for k in 0..256 {
                if beat * period + k < n {
                    sig[beat * period + k] = 1.0;
                }
            }
        }
        let est = estimate_bpm(&sig, sr).expect("a steady click train has a tempo");
        assert!((est - 120.0).abs() < 6.0, "got {est}");
    }

    #[test]
    fn estimate_bpm_rejects_silence() {
        assert!(estimate_bpm(&vec![0.0f32; 44100 * 2], 44100.0).is_none());
    }

    #[test]
    fn collect_audio_finds_only_ogg_mp3() {
        // The shipped songs include music.ogg files; scanning assets finds some.
        let mut found = Vec::new();
        collect_audio(std::path::Path::new("assets/songs"), 4, 50, &mut found);
        assert!(found.iter().all(|p| {
            matches!(p.extension().and_then(|e| e.to_str()), Some("ogg" | "mp3"))
        }));
    }
}
