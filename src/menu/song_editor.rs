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
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};

use crate::dialogs::file_dialog::{DialogId, FileChosen, FileDialog, OpenFileDialog};
use crate::gameplay::twelve_bar_blues_overlay::bar_bg;
use crate::song::chart::{
    Action, BendingProfile, DiatonicLayout, Difficulty, HarpChart, Modifier, NoteEvent, Scoring,
    Song, TempoPoint, Timing, TrackItem,
};
use crate::song::harmonica::{Harmonica, twelve_bar};

use super::AppState;

// ── Model ───────────────────────────────────────────────────────────────────

/// One authored event: either a hole played blow (exhale) or draw (inhale), or a
/// silence (`rest`). Duration is in beats (quarter = 1.0). For a rest, `hole`
/// and `is_blow` are unused. `mods` is a bitmask of [`NoteMod`]s.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EditorNote {
    pub hole: u8,
    pub is_blow: bool,
    pub beats: f32,
    pub rest: bool,
    pub mods: u8,
}

/// A technique applied to a note. Stored as bits in [`EditorNote::mods`]. (There
/// is no "slide" — that's a chromatic-harp control with no chart modifier.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NoteMod {
    Bend,
    Overblow,
    Overdraw,
    Vibrato,
    WahWah,
}

impl NoteMod {
    const ALL: [NoteMod; 5] = [
        NoteMod::Bend,
        NoteMod::Overblow,
        NoteMod::Overdraw,
        NoteMod::Vibrato,
        NoteMod::WahWah,
    ];

    fn bit(self) -> u8 {
        match self {
            NoteMod::Bend => 1,
            NoteMod::Overblow => 2,
            NoteMod::Overdraw => 4,
            NoteMod::Vibrato => 8,
            NoteMod::WahWah => 16,
        }
    }

    fn label(self) -> &'static str {
        match self {
            NoteMod::Bend => "Bend",
            NoteMod::Overblow => "Overblow",
            NoteMod::Overdraw => "Overdraw",
            NoteMod::Vibrato => "Vibrato",
            NoteMod::WahWah => "Wah",
        }
    }

    /// Short tag shown on a note that carries this modifier.
    fn tag(self) -> &'static str {
        match self {
            NoteMod::Bend => "b",
            NoteMod::Overblow => "o",
            NoteMod::Overdraw => "O",
            NoteMod::Vibrato => "v",
            NoteMod::WahWah => "w",
        }
    }

    /// The chart-format modifier for this technique (sensible default params).
    fn to_modifier(self) -> Modifier {
        match self {
            NoteMod::Bend => Modifier::Bend { semitones: -1.0, intensity: None },
            NoteMod::Overblow => Modifier::Overblow,
            NoteMod::Overdraw => Modifier::Overdraw,
            NoteMod::Vibrato => Modifier::Vibrato { oscillation_hz: 5.0, intensity: None },
            NoteMod::WahWah => Modifier::WahWah { oscillation_hz: 4.0, intensity: None },
        }
    }
}

/// Whether `m` is physically possible on `note` ("where possible"): draw bends
/// on holes 1-6 and blow bends on 7-10; overblow on blow holes 1-6; overdraw on
/// draw holes 7-10; vibrato/wah on any sounded note; nothing on a rest.
fn mod_valid(note: &EditorNote, m: NoteMod) -> bool {
    if note.rest {
        return false;
    }
    match m {
        NoteMod::Bend => (note.is_blow && note.hole >= 7) || (!note.is_blow && note.hole <= 6),
        NoteMod::Overblow => note.is_blow && note.hole <= 6,
        NoteMod::Overdraw => !note.is_blow && note.hole >= 7,
        NoteMod::Vibrato | NoteMod::WahWah => true,
    }
}

/// The modifier tags carried by a note, concatenated (e.g. `"bv"`), or empty.
fn mods_tag(mods: u8) -> String {
    NoteMod::ALL
        .iter()
        .filter(|m| mods & m.bit() != 0)
        .map(|m| m.tag())
        .collect()
}

/// The song being authored. Strings are kept as typed text and parsed on save.
#[derive(Resource)]
pub struct SongEditorData {
    pub artist: String,
    pub song_name: String,
    pub music_path: Option<PathBuf>,
    pub tempo_bpm: String,
    pub beats_per_bar: String,
    pub harp_key: String,
    /// Authored notes, in play order.
    pub notes: Vec<EditorNote>,
    /// Selected note indices (sorted), for batch edit/delete/modifier ops.
    pub selected: Vec<usize>,
    /// The focused note — the moving end of a shift range and the one Enter
    /// edits / whose spec is loaded into `note_input`.
    pub cursor: Option<usize>,
    /// The fixed end of a shift-extended range.
    pub anchor: Option<usize>,
    /// The note being typed, e.g. `"-4 q"`.
    pub note_input: String,
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
            notes: Vec::new(),
            selected: Vec::new(),
            cursor: None,
            anchor: None,
            note_input: String::new(),
        }
    }
}

// ── Note parsing / durations ────────────────────────────────────────────────

/// Beats for a duration letter: whole/half/quarter/eighth/sixteenth.
fn dur_beats(letter: char) -> Option<f32> {
    match letter {
        'w' => Some(4.0),
        'h' => Some(2.0),
        'q' => Some(1.0),
        'e' => Some(0.5),
        's' => Some(0.25),
        _ => None,
    }
}

/// The duration letter closest to a beat count (for round-tripping/formatting).
fn beats_letter(beats: f32) -> char {
    match beats {
        b if b >= 4.0 => 'w',
        b if b >= 2.0 => 'h',
        b if b >= 1.0 => 'q',
        b if b >= 0.5 => 'e',
        _ => 's',
    }
}

/// Note glyph for a duration in beats. Quarter/eighth/sixteenth use the BMP note
/// symbols the sans default font renders; whole/half have no glyph in any sans
/// font (they live in the Musical Symbols block), so they show a short word.
fn dur_symbol(beats: f32) -> &'static str {
    match beats_letter(beats) {
        'w' => "whole",
        'h' => "half",
        'q' => "\u{2669}", // ♩ quarter note
        'e' => "\u{266A}", // ♪ eighth note
        _ => "\u{266C}",   // ♬ sixteenth (beamed)
    }
}

/// Parse a note spec like `"-4 q"`, `"4 e"`, or `"3"` (defaults to a quarter).
/// `-` prefix = draw (inhale); otherwise blow (exhale). Hole 1..=10. A spec
/// starting with `r` is a silence/rest, e.g. `"r q"` or `"r"`.
fn parse_note(input: &str) -> Option<EditorNote> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    // Rest: "r" + optional duration letter.
    if matches!(s.chars().next(), Some('r' | 'R')) {
        let beats = match s[1..].trim_start().chars().next() {
            Some(c) => dur_beats(c.to_ascii_lowercase())?,
            None => 1.0,
        };
        return Some(EditorNote { hole: 0, is_blow: true, beats, rest: true, mods: 0 });
    }
    let (is_blow, rest) = match s.strip_prefix('-') {
        Some(r) => (false, r.trim_start()),
        None => (true, s),
    };
    let mut chars = rest.trim_start();
    // Leading digits → hole.
    let digits: String = chars.chars().take_while(|c| c.is_ascii_digit()).collect();
    let hole: u8 = digits.parse().ok()?;
    if !(1..=10).contains(&hole) {
        return None;
    }
    chars = chars[digits.len()..].trim_start();
    // Optional duration letter (defaults to a quarter note).
    let beats = match chars.chars().next() {
        Some(c) => dur_beats(c.to_ascii_lowercase())?,
        None => 1.0,
    };
    Some(EditorNote { hole, is_blow, beats, rest: false, mods: 0 })
}

/// Format a note back to its spec text, e.g. `"-4 q"` or `"r h"` for a rest.
fn format_note_spec(n: &EditorNote) -> String {
    if n.rest {
        return format!("r {}", beats_letter(n.beats));
    }
    let sign = if n.is_blow { "" } else { "-" };
    format!("{sign}{} {}", n.hole, beats_letter(n.beats))
}

/// Parsed beats-per-bar (the bar capacity), at least 1.
fn beats_per_bar_of(data: &SongEditorData) -> usize {
    data.beats_per_bar.trim().parse::<usize>().unwrap_or(4).max(1)
}

/// Parsed tempo in BPM (for the seconds readout), at least 1.
fn bpm_of(data: &SongEditorData) -> f32 {
    data.tempo_bpm.trim().parse::<f32>().unwrap_or(120.0).max(1.0)
}

/// The song's key in 2nd position (cross harp): a perfect fifth above the harp
/// key — e.g. a C harp plays in G.
fn song_key_of(harp_key: &str) -> String {
    crate::song::harmonica::semitone(harp_key, 7)
}

/// Which bar each note falls in: notes fill a bar's beats, then spill to the
/// next. Returns one bucket of note indices per used bar (at least the notes
/// given; empty when there are none).
fn notes_by_bar(notes: &[EditorNote], beats_per_bar: usize) -> Vec<Vec<usize>> {
    let cap = beats_per_bar as f32;
    let mut bars: Vec<Vec<usize>> = Vec::new();
    let mut used = 0.0f32;
    for (i, n) in notes.iter().enumerate() {
        if bars.is_empty() {
            bars.push(Vec::new());
        }
        // If this note won't fit the remaining space and the bar already has
        // something, start a new bar.
        if used > 0.0 && used + n.beats > cap {
            bars.push(Vec::new());
            used = 0.0;
        }
        bars.last_mut().unwrap().push(i);
        used += n.beats;
    }
    bars
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

/// What currently receives keyboard input.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Focus {
    #[default]
    None,
    Field(TextFieldId),
    Note,
}

/// Which input target currently has focus.
#[derive(Resource, Default)]
struct FocusedField(Focus);

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
struct HarpPlaysText;
#[derive(Component)]
struct MusicPickButton;
#[derive(Component)]
struct MusicPathText;
#[derive(Component)]
struct TwelveBarGrid;
#[derive(Component)]
struct NoteEntryBox;
#[derive(Component)]
struct NoteEntryText;
#[derive(Component)]
struct NoteDurationText;
#[derive(Component)]
struct NoteWidget(usize);
#[derive(Component)]
struct ModButton(NoteMod);
#[derive(Component)]
struct SaveButton;
#[derive(Component)]
struct AnalyzeStatusText;

// ── Colours ─────────────────────────────────────────────────────────────────

const FIELD_BG: Color = Color::srgba(0.10, 0.10, 0.14, 0.95);
const FIELD_BG_FOCUS: Color = Color::srgba(0.16, 0.16, 0.24, 1.0);
const BTN_BG: Color = Color::srgba(0.14, 0.14, 0.20, 0.95);
const ACCENT: Color = Color::srgb(0.95, 0.80, 0.35);
const LABEL: Color = Color::srgb(0.75, 0.75, 0.82);

// ── Setup ───────────────────────────────────────────────────────────────────

fn setup(mut commands: Commands, data: Res<SongEditorData>) {

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
                TextFont { font_size: FontSize::Px(34.0), ..default() },
                TextColor(Color::WHITE),
            ));

            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                min_width: Val::Px(540.0),
                ..default()
            })
            .with_children(|form| {
                text_field(form, "Artist", TextFieldId::Artist, &data.artist);
                text_field(form, "Song Name", TextFieldId::SongName, &data.song_name);
                music_field(form, &data.music_path);
                text_field(form, "Music Tempo  \u{2669} =", TextFieldId::Tempo, &data.tempo_bpm);
                text_field(form, "Beats per Bar", TextFieldId::BeatsPerBar, &data.beats_per_bar);
                harp_field(form, &data.harp_key);
                note_field(form, &data.note_input);
            });

            root.spawn((
                Text::new(String::new()),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.7, 0.85, 1.0)),
                NoteDurationText,
            ));
            root.spawn((
                Text::new("Note: \"-4 q\" (draw 4, quarter), \"4 e\" (blow 4, eighth), \"r h\" (half-rest silence)  \u{00B7}  Enter add/edit  \u{00B7}  \u{2190}/\u{2192} select (Shift=range, Ctrl=add)  \u{00B7}  Backspace delete"),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));

            // Modifier toolbar — applies to the current selection where possible.
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Text::new("Mark selected:"),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(LABEL),
                ));
                for m in NoteMod::ALL {
                    mod_button(row, m);
                }
            });

            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
                TwelveBarGrid,
            ))
            .with_children(|grid| {
                build_grid(grid, &data);
            });

            root.spawn((
                Button,
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.16, 0.22, 0.16, 0.95)),
                BorderColor::all(Color::srgb(0.40, 0.65, 0.40)),
                SaveButton,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Save Song"),
                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                    TextColor(Color::srgb(0.7, 0.95, 0.7)),
                ));
            })
            .observe(save_song_click);

            root.spawn((
                Text::new(String::new()),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(ACCENT),
                AnalyzeStatusText,
            ));

            root.spawn((
                Text::new("Click a field to edit  \u{00B7}  Esc to go back"),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));
        });
}

/// The note-entry row: a label and a click-to-focus box for the note spec.
/// A modifier toolbar button that toggles `m` on the selection.
fn mod_button(parent: &mut ChildSpawnerCommands, m: NoteMod) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(Color::srgb(0.35, 0.35, 0.50)),
            ModButton(m),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(m.label()),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.95)),
            ));
        })
        .observe(
            move |_: On<Pointer<Click>>,
                  mut data: ResMut<SongEditorData>,
                  mut status: Query<&mut Text, With<AnalyzeStatusText>>| {
                apply_modifier(m, &mut data, &mut status);
            },
        );
}

fn note_field(parent: &mut ChildSpawnerCommands, initial: &str) {
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
                Text::new("Add / Edit Note:"),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(LABEL),
            ));
            row.spawn((
                Button,
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(260.0),
                    height: Val::Px(30.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(FIELD_BG),
                BorderColor::all(Color::srgb(0.30, 0.30, 0.42)),
                NoteEntryBox,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(initial.to_string()),
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(Color::WHITE),
                    NoteEntryText,
                ));
            })
            .observe(focus_note);
        });
}

fn text_field(
    parent: &mut ChildSpawnerCommands,
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
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(LABEL),
            ));
            row.spawn((
                Button,
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(260.0),
                    height: Val::Px(30.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
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
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(Color::WHITE),
                    TextFieldText(id),
                ));
            })
            .observe(move |_: On<Pointer<Click>>, mut focused: ResMut<FocusedField>| {
                focused.0 = Focus::Field(id);
            });
        });
}

fn music_field(parent: &mut ChildSpawnerCommands, path: &Option<PathBuf>) {
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
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(LABEL),
            ));
            row.spawn((
                Node { flex_grow: 1.0, min_width: Val::Px(180.0), ..default() },
                Text::new(music_label(path)),
                TextFont { font_size: FontSize::Px(14.0), ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.9)),
                MusicPathText,
            ));
            row.spawn((
                Button,
                Node {
                    height: Val::Px(30.0),
                    padding: UiRect::horizontal(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(BTN_BG),
                BorderColor::all(Color::srgb(0.35, 0.35, 0.50)),
                MusicPickButton,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Browse\u{2026}"),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(ACCENT),
                ));
            })
            .observe(browse_music);
        });
}

fn harp_field(parent: &mut ChildSpawnerCommands, key: &str) {
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
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(LABEL),
            ));
            row.spawn((
                Button,
                Node {
                    width: Val::Px(70.0),
                    height: Val::Px(30.0),
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    align_items: AlignItems::Center,
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
                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                    TextColor(ACCENT),
                    HarpKeyText,
                ));
            })
            .observe(cycle_harp_key);
            row.spawn((
                Text::new(format!("click to cycle  \u{00B7}  plays in {} (2nd position)", song_key_of(key))),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.6)),
                HarpPlaysText,
            ));
        });
}

/// Build the 12-bar grid as one or more pages (rows of 12 bars), each bar
/// showing its chord and the notes that fall in it (arrows sized by duration).
fn build_grid(parent: &mut ChildSpawnerCommands, data: &SongEditorData) {
    let bpb = beats_per_bar_of(data);
    // 2nd position: the song's key (and its I/IV/V chords) is a fifth above the
    // harp key, so the preview shows the key you actually play in.
    let song_key = song_key_of(&data.harp_key);
    let chords = twelve_bar(&song_key);
    let bars = notes_by_bar(&data.notes, bpb);
    let pages = (bars.len().div_ceil(12)).max(1);

    for page in 0..pages {
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|row| {
                for col in 0..12 {
                    let bar_global = page * 12 + col;
                    let empty = Vec::new();
                    let bar_notes = bars.get(bar_global).unwrap_or(&empty);
                    // Reuse the gameplay grid's I/IV/V chord colouring so the
                    // editor preview matches the real 12-bar blues view.
                    let bg = bar_bg(col, &song_key);
                    build_bar_cell(row, col + 1, &chords[col], bg, bar_notes, data, bpb);
                }
            });
    }
}

/// One bar cell: chord/bar-number header plus its notes as arrow widgets.
fn build_bar_cell(
    parent: &mut ChildSpawnerCommands,
    bar_num: usize,
    chord: &str,
    bg: Color,
    note_indices: &[usize],
    data: &SongEditorData,
    beats_per_bar: usize,
) {
    parent
        .spawn((
            Node {
                width: Val::Px(88.0),
                height: Val::Px(72.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgb(0.30, 0.32, 0.42)),
        ))
        .with_children(|cell| {
            cell.spawn((
                Text::new(format!("{bar_num}  {chord}")),
                TextFont { font_size: FontSize::Px(10.0), ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));
            // Row of note arrows, widths proportional to each note's beats.
            cell.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::Center,
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                column_gap: Val::Px(1.0),
                ..default()
            })
            .with_children(|notes_row| {
                for &ni in note_indices {
                    let n = data.notes[ni];
                    let selected = data.selected.contains(&ni);
                    let is_cursor = data.cursor == Some(ni);
                    let frac = (n.beats / beats_per_bar as f32).clamp(0.12, 1.0);
                    notes_row
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(frac * 100.0),
                                height: Val::Percent(100.0),
                                min_width: Val::Px(14.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(if is_cursor { 2.0 } else { 1.0 })),
                                ..default()
                            },
                            BackgroundColor(if selected {
                                Color::srgba(0.30, 0.30, 0.12, 0.95)
                            } else if n.rest {
                                Color::srgba(0.10, 0.10, 0.12, 0.95)
                            } else {
                                Color::srgba(0.16, 0.16, 0.20, 0.95)
                            }),
                            BorderColor::all(if is_cursor {
                                ACCENT
                            } else if selected {
                                Color::srgb(0.70, 0.62, 0.30)
                            } else {
                                Color::srgb(0.30, 0.30, 0.40)
                            }),
                            NoteWidget(ni),
                        ))
                        .with_children(|w| {
                            if n.rest {
                                // A silence: a centered dim bar, no arrow/number.
                                w.spawn((
                                    Node {
                                        width: Val::Percent(55.0),
                                        height: Val::Px(2.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.5, 0.5, 0.55)),
                                ));
                            } else {
                                let (arrow, color) = if n.is_blow {
                                    ("\u{2191}", Color::srgb(0.30, 0.60, 0.95))
                                } else {
                                    ("\u{2193}", Color::srgb(0.95, 0.45, 0.20))
                                };
                                w.spawn((
                                    Text::new(arrow),
                                    TextFont { font_size: FontSize::Px(16.0), ..default() },
                                    TextColor(color),
                                ));
                                w.spawn((
                                    Text::new(n.hole.to_string()),
                                    TextFont { font_size: FontSize::Px(12.0), ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            }
                            // Modifier tags (e.g. "bv"), shown small at the bottom.
                            if n.mods != 0 {
                                w.spawn((
                                    Text::new(mods_tag(n.mods)),
                                    TextFont { font_size: FontSize::Px(9.0), ..default() },
                                    TextColor(Color::srgb(0.95, 0.80, 0.35)),
                                ));
                            }
                        })
                        .observe(
                            move |_: On<Pointer<Click>>,
                                  keyboard: Res<ButtonInput<KeyCode>>,
                                  mut data: ResMut<SongEditorData>,
                                  mut focused: ResMut<FocusedField>| {
                                select_note(ni, &keyboard, &mut data, &mut focused);
                            },
                        );
                }
            });
        });
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

/// Clicking the note box focuses it for typing. (Metadata fields focus via a
/// per-field closure that captures their id; see `text_field`.)
fn focus_note(_: On<Pointer<Click>>, mut focused: ResMut<FocusedField>) {
    focused.0 = Focus::Note;
}

/// Route typed characters into the focused field. No-op while a field isn't
/// focused (e.g. the file browser is open and clears focus).
fn type_into_focused(
    mut keys: MessageReader<KeyboardInput>,
    focused: Res<FocusedField>,
    mut data: ResMut<SongEditorData>,
) {
    let Focus::Field(field) = focused.0 else {
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
        if focused.0 == Focus::Field(field.0) {
            s.push('_');
        }
        **text = s;
    }
    for (field, mut bg) in &mut boxes {
        bg.0 = if focused.0 == Focus::Field(field.0) { FIELD_BG_FOCUS } else { FIELD_BG };
    }
    if let Ok(mut text) = music.single_mut() {
        **text = music_label(&data.music_path);
    }
}

/// Mirror the note buffer into its box (caret when focused) and show the parsed
/// note's musical + seconds duration.
fn update_note_views(
    data: Res<SongEditorData>,
    focused: Res<FocusedField>,
    mut entry: Query<
        &mut Text,
        (With<NoteEntryText>, Without<TextFieldText>, Without<MusicPathText>),
    >,
    mut duration: Query<
        &mut Text,
        (
            With<NoteDurationText>,
            Without<NoteEntryText>,
            Without<TextFieldText>,
            Without<MusicPathText>,
        ),
    >,
    mut boxes: Query<&mut BackgroundColor, With<NoteEntryBox>>,
) {
    let focused_note = focused.0 == Focus::Note;
    if let Ok(mut text) = entry.single_mut() {
        let mut s = data.note_input.clone();
        if focused_note {
            s.push('_');
        }
        **text = s;
    }
    if let Ok(mut bg) = boxes.single_mut() {
        bg.0 = if focused_note { FIELD_BG_FOCUS } else { FIELD_BG };
    }
    if let Ok(mut text) = duration.single_mut() {
        **text = match parse_note(&data.note_input) {
            Some(n) => {
                let secs = n.beats * 60.0 / bpm_of(&data);
                if n.rest {
                    format!(
                        "\u{2192} silence  \u{00B7}  {}  \u{00B7}  {secs:.2}s",
                        dur_symbol(n.beats),
                    )
                } else {
                    let dir = if n.is_blow { "blow" } else { "draw" };
                    format!(
                        "\u{2192} {dir} hole {}  \u{00B7}  {}  \u{00B7}  {secs:.2}s",
                        n.hole,
                        dur_symbol(n.beats),
                    )
                }
            }
            None => String::new(),
        };
    }
}

// ── Interaction: harmonica key ────────────────────────────────────────────────

/// Cycle the harp key on click and update its label. The grid is rebuilt by
/// `rebuild_grid` (the key change marks the data as changed).
fn cycle_harp_key(
    _: On<Pointer<Click>>,
    mut data: ResMut<SongEditorData>,
    mut key_texts: Query<&mut Text, (With<HarpKeyText>, Without<HarpPlaysText>)>,
    mut plays_texts: Query<&mut Text, (With<HarpPlaysText>, Without<HarpKeyText>)>,
) {
    data.harp_key = next_key(&data.harp_key);
    for mut t in &mut key_texts {
        **t = data.harp_key.clone();
    }
    for mut t in &mut plays_texts {
        **t = format!("click to cycle  \u{00B7}  plays in {} (2nd position)", song_key_of(&data.harp_key));
    }
}

// ── Interaction: notes ──────────────────────────────────────────────────────

/// Keyboard handling while the note box is focused: type the spec, Enter to
/// add/edit, Backspace to erase a char (or delete the selection when empty),
/// arrows to move the cursor. Shift+arrow extends a contiguous selection;
/// Ctrl+arrow adds the focused note to a non-contiguous selection.
fn note_input_keys(
    mut keys: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    focused: Res<FocusedField>,
    mut data: ResMut<SongEditorData>,
) {
    if focused.0 != Focus::Note {
        keys.clear();
        return;
    }
    let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Character(s) => {
                for c in s.chars() {
                    if !c.is_control() {
                        data.note_input.push(c);
                    }
                }
            }
            Key::Space => data.note_input.push(' '),
            Key::Backspace => {
                if data.note_input.pop().is_none() {
                    delete_selected_notes(&mut data);
                }
            }
            Key::Enter => commit_note(&mut data),
            Key::ArrowLeft => move_cursor(&mut data, -1, shift, ctrl),
            Key::ArrowRight => move_cursor(&mut data, 1, shift, ctrl),
            _ => {}
        }
    }
}

/// Commit the typed spec: replace the focused note (keeping its modifiers), or
/// append a new one.
fn commit_note(data: &mut SongEditorData) {
    let Some(mut note) = parse_note(&data.note_input) else {
        return;
    };
    match data.cursor {
        Some(i) if i < data.notes.len() => {
            note.mods = data.notes[i].mods; // editing the spec keeps modifiers
            data.notes[i] = note;
        }
        _ => {
            data.notes.push(note);
            data.cursor = None;
            data.selected.clear();
            data.anchor = None;
        }
    }
    data.note_input.clear();
}

/// Delete every selected note (or the focused/last one when nothing is
/// selected), then settle the cursor.
fn delete_selected_notes(data: &mut SongEditorData) {
    let mut targets = if data.selected.is_empty() {
        match data.cursor.or_else(|| data.notes.len().checked_sub(1)) {
            Some(i) => vec![i],
            None => return,
        }
    } else {
        data.selected.clone()
    };
    targets.sort_unstable();
    targets.dedup();
    for &i in targets.iter().rev() {
        if i < data.notes.len() {
            data.notes.remove(i);
        }
    }
    data.selected.clear();
    data.anchor = None;
    data.cursor = if data.notes.is_empty() {
        None
    } else {
        Some(targets[0].min(data.notes.len() - 1))
    };
    data.note_input = match data.cursor {
        Some(c) => format_note_spec(&data.notes[c]),
        None => String::new(),
    };
}

/// Move the focused note by `dir`. `shift` extends a contiguous range from the
/// anchor; `ctrl` adds the new focus to the existing selection.
fn move_cursor(data: &mut SongEditorData, dir: i32, shift: bool, ctrl: bool) {
    if data.notes.is_empty() {
        return;
    }
    let last = data.notes.len() - 1;
    let next = match data.cursor {
        None => if dir > 0 { 0 } else { last },
        Some(i) if dir < 0 => i.saturating_sub(1),
        Some(i) => (i + 1).min(last),
    };
    data.cursor = Some(next);
    if shift {
        let a = data.anchor.unwrap_or(next);
        data.anchor = Some(a);
        data.selected = (a.min(next)..=a.max(next)).collect();
    } else if ctrl {
        if !data.selected.contains(&next) {
            data.selected.push(next);
            data.selected.sort_unstable();
        }
    } else {
        data.selected = vec![next];
        data.anchor = Some(next);
    }
    data.note_input = format_note_spec(&data.notes[next]);
}

/// Clicking a note selects it; Ctrl+click toggles it; Shift+click extends a
/// range from the anchor. Each note widget carries this as a dedicated `on()`
/// closure capturing its index (see `build_bar_cell`).
fn select_note(
    i: usize,
    keyboard: &ButtonInput<KeyCode>,
    data: &mut SongEditorData,
    focused: &mut FocusedField,
) {
    let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if i >= data.notes.len() {
        return;
    }
    {
        if ctrl {
            match data.selected.iter().position(|&x| x == i) {
                Some(pos) => {
                    data.selected.remove(pos);
                }
                None => {
                    data.selected.push(i);
                    data.selected.sort_unstable();
                }
            }
        } else if shift {
            let a = data.anchor.unwrap_or(i);
            data.anchor = Some(a);
            data.selected = (a.min(i)..=a.max(i)).collect();
        } else {
            data.selected = vec![i];
            data.anchor = Some(i);
        }
        data.cursor = Some(i);
        data.note_input = format_note_spec(&data.notes[i]);
        focused.0 = Focus::Note;
    }
}

/// Apply (toggle) a modifier on every selected note where it's physically
/// possible. Reports the result in the status line. Each toolbar button carries
/// this as a dedicated `on()` closure capturing its modifier (see `mod_button`).
fn apply_modifier(
    m: NoteMod,
    data: &mut SongEditorData,
    status: &mut Query<&mut Text, With<AnalyzeStatusText>>,
) {
    let targets: Vec<usize> = data
        .selected
        .iter()
        .copied()
        .filter(|&i| i < data.notes.len() && mod_valid(&data.notes[i], m))
        .collect();
    let msg = if targets.is_empty() {
        format!("No selected note can take {}", m.label())
    } else {
        let all_have = targets.iter().all(|&i| data.notes[i].mods & m.bit() != 0);
        for &i in &targets {
            if all_have {
                data.notes[i].mods &= !m.bit();
            } else {
                data.notes[i].mods |= m.bit();
            }
        }
        let verb = if all_have { "Removed" } else { "Applied" };
        format!("{verb} {} on {} note(s)", m.label(), targets.len())
    };
    if let Ok(mut text) = status.single_mut() {
        **text = msg;
    }
}

/// Rebuild the 12-bar grid whenever the song data changes (notes, selection,
/// key, or beats-per-bar).
fn rebuild_grid(
    data: Res<SongEditorData>,
    grids: Query<(Entity, Option<&Children>), With<TwelveBarGrid>>,
    mut commands: Commands,
) {
    if !data.is_changed() {
        return;
    }
    for (grid, children) in &grids {
        if let Some(children) = children {
            for &c in children {
                commands.entity(c).despawn();
            }
        }
        commands.entity(grid).with_children(|g| build_grid(g, &data));
    }
}

// ── Music file picking (via the reusable file dialog) ──────────────────────────

/// Identifies this screen's requests to the shared file dialog.
const MUSIC_DIALOG: DialogId = DialogId("song_editor_music");

/// Open the navigable file dialog when Browse is clicked.
fn browse_music(
    _: On<Pointer<Click>>,
    mut focused: ResMut<FocusedField>,
    mut open: MessageWriter<OpenFileDialog>,
) {
    focused.0 = Focus::None;
    open.write(OpenFileDialog {
        purpose: MUSIC_DIALOG,
        title: "Select a music file (ogg/mp3)".to_string(),
        extensions: vec!["ogg".into(), "mp3".into()],
        start_dir: dirs::home_dir(),
    });
}

/// Receive the dialog's chosen file: set the path and kick off tempo analysis.
fn pick_file(
    mut chosen: MessageReader<FileChosen>,
    mut data: ResMut<SongEditorData>,
    mut task: ResMut<TempoTask>,
    mut status: Query<&mut Text, With<AnalyzeStatusText>>,
) {
    for ev in chosen.read() {
        if ev.purpose != MUSIC_DIALOG {
            continue;
        }
        data.music_path = Some(ev.path.clone());
        let path = ev.path.clone();
        let pool = AsyncComputeTaskPool::get();
        task.0 = Some(pool.spawn(async move { analyze_tempo(&path) }));
        if let Ok(mut text) = status.single_mut() {
            **text = "Analyzing tempo\u{2026}".to_string();
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

// ── Saving to a .harpchart ────────────────────────────────────────────────────

/// The standard Richter C-harp layout, transposed for other keys.
const C_BLOW: [&str; 10] = ["C4", "E4", "G4", "C5", "E5", "G5", "C6", "E6", "G6", "C7"];
const C_DRAW: [&str; 10] = ["D4", "G4", "B4", "D5", "F5", "A5", "B5", "D6", "F6", "A6"];

/// Blow/draw note layout for a harp in `key`, transposed from the C reference.
fn harp_layout(key: &str) -> (Vec<String>, Vec<String>) {
    use crate::audio_system::midi::{midi_to_note, note_to_midi};
    let off = KEYS.iter().position(|&k| k == key).unwrap_or(0) as i32;
    let tr = |n: &str| midi_to_note(note_to_midi(n).unwrap_or(60) + off);
    (
        C_BLOW.iter().map(|n| tr(n)).collect(),
        C_DRAW.iter().map(|n| tr(n)).collect(),
    )
}

/// Replace characters awkward in a folder name with underscores.
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_".contains(c) { c } else { '_' })
        .collect();
    if cleaned.is_empty() {
        "Untitled".to_string()
    } else {
        cleaned
    }
}

/// Build a typed [`HarpChart`] from the editor state, reusing the `song::chart`
/// definitions instead of hand-rolled JSON.
fn build_chart(data: &SongEditorData) -> HarpChart {
    let bpm = bpm_of(data);
    let bpb = beats_per_bar_of(data);
    let beat_secs = 60.0 / bpm as f64;
    let (blow, draw) = harp_layout(&data.harp_key);

    let mut track = Vec::new();
    let mut t = 0.0f64;
    for n in &data.notes {
        let dur = n.beats as f64 * beat_secs;
        // A rest is just a gap: advance the clock, emit no track item.
        if !n.rest {
            let modifiers: Vec<Modifier> = NoteMod::ALL
                .iter()
                .filter(|m| n.mods & m.bit() != 0)
                .map(|m| m.to_modifier())
                .collect();
            track.push(TrackItem {
                id: None,
                time: Some(t),
                tick: None,
                duration: dur,
                phrase: None,
                groove: None,
                play_mode: None,
                events: vec![NoteEvent {
                    hole: n.hole,
                    action: if n.is_blow { Action::Blow } else { Action::Draw },
                    note: None,
                    modifiers: (!modifiers.is_empty()).then_some(modifiers),
                }],
            });
        }
        t += dur;
    }

    let title = if data.song_name.trim().is_empty() { "Untitled" } else { data.song_name.trim() };
    let artist = if data.artist.trim().is_empty() { "Unknown" } else { data.artist.trim() };

    HarpChart {
        metadata: None,
        song: Song {
            title: title.to_string(),
            artist: artist.to_string(),
            tempo_bpm: bpm,
            key: song_key_of(&data.harp_key),
            time_signature: Some(format!("{bpb}/4")),
            difficulty: Difficulty::Easy,
        },
        timing: Timing {
            resolution: 480,
            tempo_map: vec![TempoPoint { tick: 0, bpm }],
            time_signature_map: None,
        },
        harmonica: Harmonica::Diatonic {
            holes: 10,
            bending_profile: BendingProfile::RichterStandard,
            position: Some("2nd".to_string()),
            layout: Some(DiatonicLayout { blow: Some(blow), draw: Some(draw) }),
        },
        track,
        loop_section: None,
        scoring: Scoring {
            perfect_window_ms: 50,
            good_window_ms: 100,
            miss_window_ms: 130,
            combo: None,
            style_bonus: None,
        },
        fx_mapping: None,
    }
}

/// The chart as schema-clean JSON: serialize the typed chart, then drop `null`
/// fields (unset optionals) so the output matches the song schema.
fn build_chart_json(data: &SongEditorData) -> serde_json::Value {
    let mut value = serde_json::to_value(build_chart(data)).unwrap_or(serde_json::Value::Null);
    strip_nulls(&mut value);
    value
}

/// Recursively remove `null`-valued object entries.
fn strip_nulls(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for v in map.values_mut() {
                strip_nulls(v);
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_nulls),
        _ => {}
    }
}

/// Write the song to `assets/songs/<artist>/<song>/`: the chart, a copy of the
/// chosen music as `song/music.ogg`, and placeholder background/elements images
/// so the song is immediately loadable. Returns a status message.
fn save_song(data: &SongEditorData) -> String {
    let Some(music) = data.music_path.clone() else {
        return "Pick a music file before saving".to_string();
    };
    let chart = build_chart_json(data);

    let dir = std::path::Path::new("assets/songs")
        .join(sanitize(&data.artist))
        .join(sanitize(&data.song_name));
    let song_dir = dir.join("song");
    if let Err(e) = std::fs::create_dir_all(&song_dir) {
        return format!("Couldn't create folder: {e}");
    }

    let chart_json = match serde_json::to_string_pretty(&chart) {
        Ok(s) => s,
        Err(e) => return format!("Couldn't serialize chart: {e}"),
    };
    if let Err(e) = std::fs::write(song_dir.join("chart.harpchart"), chart_json) {
        return format!("Couldn't write chart: {e}");
    }
    if let Err(e) = std::fs::copy(&music, song_dir.join("music.ogg")) {
        return format!("Couldn't copy music: {e}");
    }

    // Placeholder art so the loader's required images resolve.
    let bg = image::RgbaImage::from_pixel(64, 64, image::Rgba([18, 18, 26, 255]));
    let _ = bg.save(dir.join("background.png"));
    let elements = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 0]));
    let _ = elements.save(dir.join("elements.png"));

    format!("Saved to {}", dir.display())
}

/// Save the song when the Save button is clicked, reporting the result.
fn save_song_click(
    _: On<Pointer<Click>>,
    data: Res<SongEditorData>,
    mut status: Query<&mut Text, With<AnalyzeStatusText>>,
) {
    let msg = save_song(&data);
    if let Ok(mut text) = status.single_mut() {
        **text = msg;
    }
}

// ── Escape / lifecycle ─────────────────────────────────────────────────────────

/// Esc: blur a focused field, else go back. While the file dialog is open it
/// owns Esc (and consumes it), so we stay out of its way.
fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    dialog: Res<FileDialog>,
    mut focused: ResMut<FocusedField>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if dialog.open || !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    if focused.0 != Focus::None {
        focused.0 = Focus::None;
    } else {
        next_state.set(AppState::Menu);
    }
}

fn cleanup(
    mut commands: Commands,
    roots: Query<Entity, With<SongEditorRoot>>,
    mut focused: ResMut<FocusedField>,
    mut task: ResMut<TempoTask>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    focused.0 = Focus::None;
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
                    // Widget clicks (focus, harp cycle, note select, modifiers,
                    // browse, save) ride along as inline on(...) observers wired
                    // at spawn — see the spawn helpers and build_grid.
                    handle_escape,
                    type_into_focused,
                    note_input_keys,
                    pick_file,
                    poll_tempo,
                    update_field_views,
                    update_note_views,
                    rebuild_grid,
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
    fn parse_note_handles_draw_blow_and_durations() {
        let draw = parse_note("-4 q").unwrap();
        assert_eq!((draw.hole, draw.is_blow, draw.beats), (4, false, 1.0));
        let blow = parse_note("4 e").unwrap();
        assert_eq!((blow.hole, blow.is_blow, blow.beats), (4, true, 0.5));
        // No duration letter defaults to a quarter.
        assert_eq!(parse_note("3").unwrap().beats, 1.0);
        // Out-of-range hole / empty are rejected.
        assert!(parse_note("0 q").is_none());
        assert!(parse_note("11 q").is_none());
        assert!(parse_note("").is_none());
    }

    #[test]
    fn parse_note_handles_rests() {
        let r = parse_note("r h").unwrap();
        assert!(r.rest && r.beats == 2.0);
        // "r" alone defaults to a quarter rest.
        assert_eq!(parse_note("r").unwrap().beats, 1.0);
        assert!(parse_note("R q").unwrap().rest);
    }

    #[test]
    fn note_spec_round_trips() {
        for spec in ["-4 q", "4 e", "2 h", "-1 w", "10 s", "r q", "r h"] {
            let n = parse_note(spec).unwrap();
            assert_eq!(format_note_spec(&n), spec, "round-trip {spec}");
        }
    }

    #[test]
    fn rests_create_gaps_without_track_items() {
        // quarter note, half rest, quarter note at 120 BPM (beat = 0.5s).
        let data = SongEditorData {
            notes: vec![
                EditorNote { hole: 4, is_blow: true, beats: 1.0, rest: false, mods: 0 },
                EditorNote { hole: 0, is_blow: true, beats: 2.0, rest: true, mods: 0 },
                EditorNote { hole: 4, is_blow: false, beats: 1.0, rest: false, mods: 0 },
            ],
            ..Default::default()
        };
        let chart = build_chart_json(&data);
        let track = chart["track"].as_array().unwrap();
        // The rest emits no item — only the two real notes.
        assert_eq!(track.len(), 2);
        // Second note starts after note(0.5s) + rest(1.0s) = 1.5s.
        assert_eq!(track[1]["time"].as_f64().unwrap(), 1.5);
    }

    #[test]
    fn notes_flow_into_bars_by_beats() {
        let q = |hole, blow| EditorNote { hole, is_blow: blow, beats: 1.0, rest: false, mods: 0 };
        // Five quarter notes in 4/4 → first bar holds 4, the fifth spills over.
        let notes: Vec<_> = (0..5).map(|i| q(i + 1, true)).collect();
        let bars = notes_by_bar(&notes, 4);
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0], vec![0, 1, 2, 3]);
        assert_eq!(bars[1], vec![4]);
    }

    #[test]
    fn a_half_note_uses_two_of_four_beats() {
        let half = EditorNote { hole: 2, is_blow: false, beats: 2.0, rest: false, mods: 0 };
        let q = EditorNote { hole: 3, is_blow: true, beats: 1.0, rest: false, mods: 0 };
        // half + half + quarter → bar1 holds both halves (4 beats), quarter spills.
        let bars = notes_by_bar(&[half, half, q], 4);
        assert_eq!(bars[0], vec![0, 1]);
        assert_eq!(bars[1], vec![2]);
    }

    #[test]
    fn harp_layout_transposes_from_c() {
        assert_eq!(harp_layout("C").0[0], "C4");
        assert_eq!(harp_layout("G").0[0], "G4"); // C4 + 7 semitones
        assert_eq!(harp_layout("A").0[0], "A4");
    }

    #[test]
    fn sanitize_strips_path_separators() {
        assert_eq!(sanitize("AC/DC"), "AC_DC");
        assert_eq!(sanitize("  "), "Untitled");
        assert_eq!(sanitize("Blues 1-2_3"), "Blues 1-2_3");
    }

    #[test]
    fn built_chart_validates_and_deserializes() {
        use crate::song::chart::HarpChart;
        let data = SongEditorData {
            artist: "Test".into(),
            song_name: "Song".into(),
            notes: vec![
                EditorNote { hole: 4, is_blow: false, beats: 1.0, rest: false, mods: NoteMod::Bend.bit() },
                EditorNote { hole: 4, is_blow: true, beats: 0.5, rest: false, mods: 0 },
            ],
            ..Default::default()
        };
        let chart = build_chart_json(&data);

        // Matches the shipped schema the loader validates against.
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../assets/song_schema.dtd.json")).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let errors: Vec<String> = validator.iter_errors(&chart).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "schema errors: {errors:?}");

        // And deserializes into the real chart type, modifiers included.
        let parsed: HarpChart = serde_json::from_value(chart).unwrap();
        assert_eq!(parsed.track.len(), 2);
        assert_eq!(parsed.song.artist, "Test");
        assert!(parsed.track[0].events[0].modifiers.is_some(), "bend should serialize");
        // 2nd position: a C harp (default) plays the song in G.
        assert_eq!(parsed.song.key, "G");
        assert_eq!(parsed.harmonica.position(), Some("2nd"));
    }

    #[test]
    fn song_key_is_a_fifth_above_the_harp() {
        assert_eq!(song_key_of("C"), "G");
        assert_eq!(song_key_of("A"), "E");
        assert_eq!(song_key_of("G"), "D");
    }

    #[test]
    fn mod_valid_follows_harp_physics() {
        let mk = |hole, is_blow| EditorNote { hole, is_blow, beats: 1.0, rest: false, mods: 0 };
        let draw4 = mk(4, false);
        assert!(mod_valid(&draw4, NoteMod::Bend)); // draw bends on 1-6
        assert!(!mod_valid(&draw4, NoteMod::Overblow));
        let blow3 = mk(3, true);
        assert!(mod_valid(&blow3, NoteMod::Overblow)); // overblow on blow 1-6
        assert!(!mod_valid(&blow3, NoteMod::Bend)); // blow bends are 7-10
        let draw8 = mk(8, false);
        assert!(mod_valid(&draw8, NoteMod::Overdraw)); // overdraw on draw 7-10
        assert!(!mod_valid(&draw8, NoteMod::Bend));
        assert!(mod_valid(&draw8, NoteMod::Vibrato)); // vibrato anywhere
        // Rests take nothing.
        let rest = EditorNote { hole: 0, is_blow: true, beats: 1.0, rest: true, mods: 0 };
        assert!(NoteMod::ALL.iter().all(|&m| !mod_valid(&rest, m)));
    }

    #[test]
    fn mods_tag_lists_applied_techniques() {
        let mods = NoteMod::Bend.bit() | NoteMod::Vibrato.bit();
        assert_eq!(mods_tag(mods), "bv");
        assert_eq!(mods_tag(0), "");
    }

    #[test]
    fn estimate_bpm_rejects_silence() {
        assert!(estimate_bpm(&vec![0.0f32; 44100 * 2], 44100.0).is_none());
    }
}
