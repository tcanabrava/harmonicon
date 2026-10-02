// SPDX-License-Identifier: MIT

//! Unified song catalog with an inline 2D/3D toggle, sorting and filtering.

use bevy::input::keyboard::Key;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AutoFocus, FocusCause, InputFocus, InputFocusVisible};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::{ComputedNode, ScrollPosition};
use bevy::ui_widgets::{Activate, Button as WidgetButton};
use harmonicon_ui::dialogs::scroll_area::spawn_scroll_area;

use harmonicon_app::app::{GameplayMode, SelectedSong};
use harmonicon_platform::assets_management::{AvailableSongs, SongEntry, SongsRescanned};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::button;
use harmonicon_ui::dialogs::button::{BaseButtonColor, CHOICE_SELECTED, make_interactive};
use harmonicon_ui::dialogs::text_input::spawn_text_input;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_menu_root_plain};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SongSort {
    #[default]
    Band,
    Difficulty,
    Song,
    Genre,
}

#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct SongPickerState {
    query: String,
    sort: SongSort,
    selected: Option<String>,
    descending: bool,
}

#[derive(Component)]
pub(crate) struct SortChoice {
    sort: SongSort,
    arrow: Entity,
}

#[derive(Component)]
pub(crate) struct SongRow {
    path: String,
    index: usize,
}

#[derive(Component)]
pub(crate) struct SongPickerRows;

#[derive(Component)]
pub(crate) struct SongPickerSearch;

#[derive(Component)]
pub(crate) enum PickerSummary {
    Count,
    Title,
    Metadata,
    Empty,
}

#[derive(Component)]
pub(crate) struct PickerPlay;

#[derive(Component)]
pub(crate) struct ModeLabel(bool);

pub(crate) fn setup_artist_list(
    mut commands: Commands,
    songs: Res<AvailableSongs>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    mode: Res<GameplayMode>,
    state: Res<SongPickerState>,
) {
    let (content, header, _) = spawn_menu_root_plain(
        &mut commands,
        &loc.msg("select-song"),
        None,
        &theme,
        "SongPicker",
    );
    commands.entity(content).insert(Node {
        width: Val::Percent(90.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(8.0),
        ..default()
    });
    let sort_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            border: UiRect {
                left: Val::Px(4.0),
                ..UiRect::all(Val::Px(1.0))
            },
            ..default()
        })
        .id();
    for (sort, width, label) in [
        (SongSort::Song, 40.0, loc.msg("song-sort-name").to_string()),
        (SongSort::Band, 25.0, loc.msg("song-sort-band").to_string()),
        (
            SongSort::Genre,
            20.0,
            loc.msg("song-sort-genre").to_string(),
        ),
        (
            SongSort::Difficulty,
            15.0,
            loc.msg("song-sort-difficulty").to_string(),
        ),
    ] {
        let arrow = commands
            .spawn((
                Text::new("↑"),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Visibility::Hidden,
                bevy::picking::Pickable::IGNORE,
            ))
            .id();
        let label = commands
            .spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                bevy::picking::Pickable::IGNORE,
            ))
            .id();
        let entity = commands
            .spawn((
                WidgetButton,
                TabIndex(0),
                SortChoice { sort, arrow },
                Node {
                    width: Val::Percent(width),
                    min_width: Val::Px(0.0),
                    flex_shrink: 0.0,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            ))
            .id();
        commands.entity(entity).add_children(&[label, arrow]);
        make_interactive(&mut commands.entity(entity), Color::srgb(0.14, 0.14, 0.22));
        commands.entity(entity).observe(
            move |_: On<Activate>, mut state: ResMut<SongPickerState>| {
                if state.sort == sort {
                    state.descending = !state.descending;
                } else {
                    state.sort = sort;
                    state.descending = false;
                }
            },
        );
        commands.entity(sort_row).add_child(entity);
    }
    let search_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    commands.entity(content).add_child(search_row);
    let label = commands
        .spawn((
            Text::new(loc.msg("song-search")),
            TextFont {
                font_size: FontSize::Px(16.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();
    commands.entity(search_row).add_child(label);
    let search = spawn_text_input(
        &mut commands,
        search_row,
        &state.query,
        420.0,
        Color::srgb(0.12, 0.12, 0.17),
        Color::srgb(0.30, 0.30, 0.40),
        |ev: On<harmonicon_ui::dialogs::text_input::TextInputCommitted>,
         mut state: ResMut<SongPickerState>| state.query = ev.value.to_lowercase(),
    );
    commands.entity(search).insert((
        SongPickerSearch,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(80.0),
            height: Val::Px(38.0),
            // EditableText draws at the content-box origin; flex alignment
            // does not position its glyphs. Inset its single line vertically.
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
    ));
    let clear = commands
        .spawn_empty()
        .apply_scene(button::small(
            &loc.msg("song-clear-search"),
            |_: On<Activate>,
             mut inputs: Query<(Entity, &mut EditableText), With<SongPickerSearch>>,
             mut focus: ResMut<InputFocus>| {
                for (entity, mut text) in &mut inputs {
                    text.editor_mut().set_text("");
                    focus.set(entity, FocusCause::Navigated);
                }
            },
        ))
        .id();
    commands.entity(search_row).add_child(clear);
    spawn_summary(&mut commands, search_row, PickerSummary::Count, 14.0);
    let empty = spawn_summary(&mut commands, content, PickerSummary::Empty, 18.0);
    commands.entity(empty).insert(Node {
        display: Display::None,
        padding: UiRect::all(Val::Px(20.0)),
        ..default()
    });
    commands.entity(content).add_child(sort_row);
    let list_frame = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    commands.entity(content).add_child(list_frame);
    let mut rows = Entity::PLACEHOLDER;
    commands.entity(list_frame).with_children(|parent| {
        rows = spawn_scroll_area(
            parent,
            Color::srgb(0.40, 0.58, 0.73),
            Color::srgb(0.08, 0.10, 0.15),
        );
    });
    commands.entity(rows).insert((
        SongPickerRows,
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            min_height: Val::Px(0.0),
            flex_grow: 1.0,
            overflow: Overflow::scroll_y(),
            ..default()
        },
        BackgroundColor(Color::srgba(0.045, 0.055, 0.085, 0.94)),
    ));
    let preview = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                padding: UiRect::all(Val::Px(16.0)),
                column_gap: Val::Px(20.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.075, 0.10, 0.15)),
        ))
        .id();
    commands.entity(content).add_child(preview);
    let details = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    commands.entity(preview).add_child(details);
    spawn_summary(&mut commands, details, PickerSummary::Title, 22.0);
    spawn_summary(&mut commands, details, PickerSummary::Metadata, 15.0);
    spawn_mode_toggle(&mut commands, preview, *mode == GameplayMode::Play2D);
    let play = commands
        .spawn_empty()
        .apply_scene(button::small(
            &loc.msg("menu-play"),
            |_: On<Activate>,
             state: Res<SongPickerState>,
             asset_server: Res<AssetServer>,
             mut commands: Commands,
             mut page: ResMut<NextState<MenuPage>>| {
                if let Some(path) = &state.selected {
                    commands.insert_resource(SelectedSong(
                        asset_server.load::<SongManifest>(path.clone()),
                    ));
                    page.set(MenuPage::HarpCheck);
                }
            },
        ))
        .insert(PickerPlay)
        .id();
    commands.entity(preview).add_child(play);
    let hints = commands
        .spawn((
            Text::new(loc.msg("song-picker-keys")),
            TextFont {
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(Color::srgb(0.65, 0.70, 0.80)),
        ))
        .id();
    commands.entity(content).add_child(hints);
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mode: Res<GameplayMode>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(if *mode == GameplayMode::JamSession {
                MenuPage::JamSessionMenu
            } else {
                MenuPage::Play
            })
        },
    );
    populate_rows(&mut commands, rows, &songs, &state, true, &[]);
}

fn spawn_summary(
    commands: &mut Commands,
    parent: Entity,
    kind: PickerSummary,
    size: f32,
) -> Entity {
    let entity = commands
        .spawn((
            kind,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

fn spawn_mode_toggle(commands: &mut Commands, parent: Entity, is_2d: bool) {
    let toggle = commands
        .spawn((
            WidgetButton,
            TabIndex(0),
            Node {
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(3.0)),
                column_gap: Val::Px(3.0),
                ..default()
            },
            BorderColor::all(Color::srgb(0.30, 0.40, 0.52)),
        ))
        .id();
    for (two_d, label) in [(true, "2d"), (false, "3d")] {
        let entity = commands
            .spawn((
                ModeLabel(two_d),
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(if two_d == is_2d {
                    CHOICE_SELECTED
                } else {
                    Color::NONE
                }),
                bevy::picking::Pickable::IGNORE,
            ))
            .id();
        commands.entity(toggle).add_child(entity);
    }
    make_interactive(&mut commands.entity(toggle), Color::srgb(0.10, 0.13, 0.19));
    commands
        .entity(toggle)
        .observe(|_: On<Activate>, mut mode: ResMut<GameplayMode>| {
            *mode = if *mode == GameplayMode::Play2D {
                GameplayMode::Play3D
            } else {
                GameplayMode::Play2D
            };
        });
    commands.entity(parent).add_child(toggle);
}

pub(crate) fn update_picker_summary(
    state: Res<SongPickerState>,
    songs: Res<AvailableSongs>,
    mode: Res<GameplayMode>,
    loc: Res<Localization>,
    mut summaries: Query<(&mut Text, &mut Node, Ref<PickerSummary>)>,
    mut modes: Query<(&ModeLabel, &mut BackgroundColor)>,
    mut plays: Query<
        (
            Entity,
            Has<bevy::ui::InteractionDisabled>,
            Option<&mut BaseButtonColor>,
        ),
        With<PickerPlay>,
    >,
    mut commands: Commands,
) {
    for (label, mut bg) in &mut modes {
        bg.set_if_neq(BackgroundColor(
            if label.0 == (*mode == GameplayMode::Play2D) {
                CHOICE_SELECTED
            } else {
                Color::NONE
            },
        ));
    }
    if !state.is_changed()
        && !songs.is_changed()
        && !summaries.iter().any(|(_, _, kind)| kind.is_added())
    {
        return;
    }
    let visible = collect_songs(&songs, &state);
    let selected = visible
        .iter()
        .find(|song| Some(&song.asset_path) == state.selected.as_ref());
    for (mut text, mut node, kind) in &mut summaries {
        let label = match *kind {
            PickerSummary::Count => {
                if visible.len() == 1 {
                    loc.msg("song-result-one").to_string()
                } else {
                    loc.msg_args("song-results", &[("count", visible.len().to_string())])
                        .to_string()
                }
            }
            PickerSummary::Title => selected.map_or_else(
                || loc.msg("song-none-selected").to_string(),
                |song| song.name.clone(),
            ),
            PickerSummary::Metadata => selected.map_or_else(String::new, |song| {
                format!("{} | {} | {}", song.artist, song.genre, song.difficulty)
            }),
            PickerSummary::Empty => {
                node.display = if visible.is_empty() {
                    Display::Flex
                } else {
                    Display::None
                };
                loc.msg("song-no-results").to_string()
            }
        };
        text.set_if_neq(Text::new(label));
    }
    for (entity, disabled, color) in &mut plays {
        if let Some(mut color) = color {
            let target = if selected.is_some() {
                CHOICE_SELECTED
            } else {
                Color::srgb(0.10, 0.11, 0.14)
            };
            if color.0 != target {
                color.0 = target;
            }
        }
        if selected.is_none() && !disabled {
            commands
                .entity(entity)
                .insert(bevy::ui::InteractionDisabled);
        } else if selected.is_some() && disabled {
            commands
                .entity(entity)
                .remove::<bevy::ui::InteractionDisabled>();
        }
    }
}

pub(crate) fn focus_picker_search(
    keyboard: Res<ButtonInput<Key>>,
    inputs: Query<Entity, With<SongPickerSearch>>,
    editable: Query<(), With<EditableText>>,
    mut focus: ResMut<InputFocus>,
) {
    if keyboard.just_pressed(Key::Character("/".into()))
        && !focus.get().is_some_and(|entity| editable.contains(entity))
    {
        if let Some(entity) = inputs.iter().next() {
            focus.set(entity, FocusCause::Navigated);
        }
    }
}

fn difficulty_rank(value: &str) -> usize {
    match value.to_ascii_lowercase().as_str() {
        "easy" => 0,
        "intermediate" => 1,
        "advanced" => 2,
        "expert" => 3,
        _ => 4,
    }
}

/// Match each query word anywhere in the metadata, allowing a small number
/// of edits for longer words. Short fragments stay exact to avoid noisy results.
fn matches_search(query: &str, fields: &[&str]) -> bool {
    let fields: Vec<_> = fields.iter().map(|field| field.to_lowercase()).collect();
    query.to_lowercase().split_whitespace().all(|term| {
        let characters: Vec<_> = term.chars().collect();
        let max_edits = match characters.len() {
            0..=3 => 0,
            4..=7 => 1,
            _ => 2,
        };
        fields.iter().any(|field| {
            field.contains(term)
                || (max_edits > 0
                    && field.split(|c: char| !c.is_alphanumeric()).any(|word| {
                        !word.is_empty() && levenshtein_substring(&characters, word) <= max_edits
                    }))
        })
    })
}

/// Levenshtein distance to the best substring, so partial words can also
/// contain insertions, deletions, or substitutions. Operates on Unicode chars.
fn levenshtein_substring(query: &[char], word: &str) -> usize {
    let word: Vec<_> = word.chars().collect();
    // A free starting position lets a match begin anywhere in the word.
    let mut previous = vec![0; word.len() + 1];
    let mut current = vec![0; word.len() + 1];
    for (i, &query_char) in query.iter().enumerate() {
        current[0] = i + 1;
        for (j, &word_char) in word.iter().enumerate() {
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + usize::from(query_char != word_char));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous.into_iter().min().unwrap_or(query.len())
}

fn collect_songs(available: &AvailableSongs, state: &SongPickerState) -> Vec<SongEntry> {
    let query = state.query.trim();
    let mut entries: Vec<_> = available
        .0
        .values()
        .flatten()
        .filter(|song| matches_search(query, &[&song.name, &song.artist, &song.genre]))
        .cloned()
        .collect();
    entries.sort_by(|a, b| match state.sort {
        SongSort::Band => a
            .artist
            .to_lowercase()
            .cmp(&b.artist.to_lowercase())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        SongSort::Difficulty => difficulty_rank(&a.difficulty)
            .cmp(&difficulty_rank(&b.difficulty))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        SongSort::Song => a
            .name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.artist.to_lowercase().cmp(&b.artist.to_lowercase())),
        SongSort::Genre => a
            .genre
            .to_lowercase()
            .cmp(&b.genre.to_lowercase())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
    });
    if state.descending {
        entries.reverse();
    }
    entries
}

/// Shared column widths and padding keep headers and song metadata aligned.
fn spawn_song_cell(commands: &mut Commands, parent: Entity, label: &str, width: f32, title: bool) {
    let cell = commands
        .spawn((
            Node {
                width: Val::Percent(width),
                min_width: Val::Px(0.0),
                flex_shrink: 0.0,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(12.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(if title { 18.0 } else { 15.0 }),
                ..default()
            },
            TextColor(if title {
                Color::WHITE
            } else {
                Color::srgb(0.76, 0.80, 0.87)
            }),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    commands.entity(cell).add_child(text);
    commands.entity(parent).add_child(cell);
}

fn populate_rows(
    commands: &mut Commands,
    root: Entity,
    songs: &AvailableSongs,
    state: &SongPickerState,
    focus_rows: bool,
    existing: &[(Entity, String)],
) {
    let mut children = Vec::new();
    for (index, song) in collect_songs(songs, state).into_iter().enumerate() {
        let path = song.asset_path.clone();
        if let Some((entity, _)) = existing.iter().find(|(_, old_path)| *old_path == path) {
            commands.entity(*entity).insert(SongRow { path, index });
            children.push(*entity);
            continue;
        }
        let selected = state.selected.as_deref() == Some(path.as_str());
        let base = if selected {
            Color::srgb(0.16, 0.30, 0.43)
        } else {
            Color::srgb(0.11, 0.11, 0.16)
        };
        let row = commands
            .spawn((
                bevy::ui_widgets::Button,
                TabIndex(0),
                SongRow {
                    path: path.clone(),
                    index,
                },
                BorderColor::all(if selected {
                    Color::srgb(0.65, 0.85, 1.0)
                } else {
                    Color::NONE
                }),
                Node {
                    width: Val::Percent(100.0),
                    border: UiRect {
                        left: Val::Px(4.0),
                        ..UiRect::all(Val::Px(1.0))
                    },
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::FlexStart,
                    ..default()
                },
                BackgroundColor(base),
            ))
            .id();
        for (label, width, title) in [
            (song.name.as_str(), 40.0, true),
            (song.artist.as_str(), 25.0, false),
            (song.genre.as_str(), 20.0, false),
            (song.difficulty.as_str(), 15.0, false),
        ] {
            spawn_song_cell(commands, row, label, width, title);
        }
        make_interactive(&mut commands.entity(row), base);
        if focus_rows
            && (state.selected.as_deref() == Some(path.as_str())
                || (state.selected.is_none() && index == 0))
        {
            commands.entity(row).insert(AutoFocus);
        }
        let preview_path = path.clone();
        commands.entity(row).observe(
            move |_: On<bevy::picking::events::PointerOver>,
                  mut state: ResMut<SongPickerState>,
                  mut focus: ResMut<InputFocus>,
                  mut visible: ResMut<InputFocusVisible>,
                  inputs: Query<(), With<EditableText>>| {
                if state.selected.as_deref() != Some(preview_path.as_str()) {
                    state.selected = Some(preview_path.clone());
                }
                if !focus.get().is_some_and(|entity| inputs.contains(entity)) {
                    focus.set(row, FocusCause::Navigated);
                    visible.0 = false;
                }
            },
        );
        commands.entity(row).observe(
            move |_: On<Activate>,
                  asset_server: Res<AssetServer>,
                  mut page: ResMut<NextState<MenuPage>>,
                  mut commands: Commands| {
                commands.insert_resource(SelectedSong(
                    asset_server.load::<SongManifest>(path.clone()),
                ));
                page.set(MenuPage::HarpCheck);
            },
        );
        children.push(row);
    }
    for (entity, _) in existing {
        if !children.contains(entity) {
            commands.entity(*entity).despawn();
        }
    }
    commands.entity(root).replace_children(&children);
}

pub(crate) fn refresh_song_picker(
    mut commands: Commands,
    mut rescanned: MessageReader<SongsRescanned>,
    songs: Res<AvailableSongs>,
    inputs: Query<&EditableText, With<SongPickerSearch>>,
    roots: Query<Entity, With<SongPickerRows>>,
    mut state: ResMut<SongPickerState>,
    mut rendered: Local<Option<(String, SongSort, bool)>>,
    focus: Res<InputFocus>,
    song_rows: Query<(Entity, &SongRow)>,
) {
    let input_changed = inputs.iter().next().is_some_and(|input| {
        let query = input.value().to_string().to_lowercase();
        if query != state.query {
            state.query = query;
            true
        } else {
            false
        }
    });
    let rescanned = rescanned.read().next().is_some();
    if !input_changed
        && !rescanned
        && rendered.as_ref() == Some(&(state.query.clone(), state.sort, state.descending))
    {
        return;
    }
    *rendered = Some((state.query.clone(), state.sort, state.descending));
    let visible = collect_songs(&songs, &state);
    if !visible
        .iter()
        .any(|song| Some(&song.asset_path) == state.selected.as_ref())
    {
        state.selected = visible.first().map(|song| song.asset_path.clone());
    }
    let existing: Vec<_> = song_rows
        .iter()
        .map(|(entity, row)| (entity, row.path.clone()))
        .collect();
    for root in &roots {
        if rescanned {
            commands.entity(root).despawn_related::<Children>();
        }
        populate_rows(
            &mut commands,
            root,
            &songs,
            &state,
            focus.get().is_none_or(|entity| song_rows.contains(entity)),
            if rescanned { &[] } else { &existing },
        );
    }
}

pub(crate) fn navigate_song_picker(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
    rows: Query<(Entity, &SongRow, &ComputedNode)>,
    inputs: Query<(), With<EditableText>>,
    mut areas: Query<(&ComputedNode, &mut ScrollPosition), With<SongPickerRows>>,
    mut state: ResMut<SongPickerState>,
) {
    if focus.get().is_some_and(|entity| inputs.contains(entity)) {
        return;
    }
    let navigating = [
        KeyCode::ArrowDown,
        KeyCode::ArrowUp,
        KeyCode::Home,
        KeyCode::End,
    ]
    .iter()
    .any(|key| keyboard.just_pressed(*key));
    // Hover can preview another song without stealing focus from Search.
    // An unchanged keyboard focus must not overwrite that preview each frame.
    if !navigating && !focus.is_changed() {
        return;
    }
    let mut ordered: Vec<_> = rows.iter().collect();
    ordered.sort_by_key(|(_, row, _)| row.index);
    if ordered.is_empty() {
        return;
    }
    let current = ordered
        .iter()
        .position(|(entity, _, _)| Some(*entity) == focus.get());
    let next = if keyboard.just_pressed(KeyCode::ArrowDown) {
        Some(current.map_or(0, |i| (i + 1).min(ordered.len() - 1)))
    } else if keyboard.just_pressed(KeyCode::ArrowUp) {
        Some(current.map_or(ordered.len() - 1, |i| i.saturating_sub(1)))
    } else if keyboard.just_pressed(KeyCode::Home) {
        Some(0)
    } else if keyboard.just_pressed(KeyCode::End) {
        Some(ordered.len() - 1)
    } else {
        current
    };
    if let Some(index) = next {
        if [
            KeyCode::ArrowDown,
            KeyCode::ArrowUp,
            KeyCode::Home,
            KeyCode::End,
        ]
        .iter()
        .any(|key| keyboard.just_pressed(*key))
        {
            focus_visible.0 = true;
        }
        let (entity, row, _) = ordered[index];
        if current != Some(index) {
            focus.set(entity, FocusCause::Navigated);
        }
        let selection_changed = state.selected.as_deref() != Some(row.path.as_str());
        if selection_changed {
            state.selected = Some(row.path.clone());
        }
        if !selection_changed && current == Some(index) {
            return;
        }
        let height = ordered[index].2.size().y * ordered[index].2.inverse_scale_factor;
        let top: f32 = ordered[..index]
            .iter()
            .map(|(_, _, node)| node.size().y * node.inverse_scale_factor + 4.0)
            .sum();
        for (area, mut scroll) in &mut areas {
            let viewport = area.size().y * area.inverse_scale_factor;
            if top < scroll.0.y {
                scroll.0.y = top;
            } else if top + height > scroll.0.y + viewport {
                scroll.0.y = (top + height - viewport).max(0.0);
            }
        }
    }
}

pub(crate) fn update_picker_feedback(
    state: Res<SongPickerState>,
    focus: Res<InputFocus>,
    mut sorts: Query<(Entity, &SortChoice, &mut BaseButtonColor)>,
    mut arrows: Query<(&mut Text, &mut Visibility)>,
    mut rows: Query<(&SongRow, &mut BaseButtonColor, &mut BorderColor), Without<SortChoice>>,
) {
    for (entity, choice, mut color) in &mut sorts {
        let active = state.sort == choice.sort;
        if let Ok((mut text, mut visibility)) = arrows.get_mut(choice.arrow) {
            text.set_if_neq(Text::new(if state.descending { "↓" } else { "↑" }));
            visibility.set_if_neq(if active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
        color.0 = if active {
            CHOICE_SELECTED
        } else if focus.get() == Some(entity) {
            Color::srgb(0.25, 0.30, 0.45)
        } else {
            Color::srgb(0.14, 0.14, 0.22)
        };
    }
    for (row, mut color, mut border) in &mut rows {
        let selected = state.selected.as_deref() == Some(row.path.as_str());
        color.0 = if selected {
            Color::srgb(0.16, 0.30, 0.43)
        } else {
            Color::srgb(0.11, 0.11, 0.16)
        };
        *border = BorderColor::all(if selected {
            Color::srgb(0.65, 0.85, 1.0)
        } else {
            Color::NONE
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_shortcut_focuses_search_without_interrupting_other_text_fields() {
        let mut app = App::new();
        app.init_resource::<InputFocus>()
            .init_resource::<ButtonInput<Key>>()
            .add_systems(Update, focus_picker_search);
        let search = app
            .world_mut()
            .spawn((SongPickerSearch, EditableText::new("")))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<Key>>()
            .press(Key::Character("/".into()));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(search));
        let other = app.world_mut().spawn(EditableText::new("text")).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(other, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ButtonInput<Key>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<Key>>()
            .press(Key::Character("/".into()));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
    }

    #[test]
    fn preview_tracks_selection_and_disables_play_for_empty_results() {
        let mut app = App::new();
        app.init_resource::<SongPickerState>()
            .init_resource::<AvailableSongs>()
            .insert_resource(GameplayMode::Play2D)
            .insert_resource(Localization::new())
            .add_systems(Update, update_picker_summary);
        app.world_mut().resource_mut::<AvailableSongs>().0.insert(
            "band".into(),
            vec![SongEntry {
                artist: "Band".into(),
                name: "Song".into(),
                genre: "Rock".into(),
                difficulty: "easy".into(),
                asset_path: "song".into(),
            }],
        );
        app.world_mut().resource_mut::<SongPickerState>().selected = Some("song".into());
        let title = app
            .world_mut()
            .spawn((PickerSummary::Title, Text::new("")))
            .id();
        let empty = app
            .world_mut()
            .spawn((PickerSummary::Empty, Text::new("")))
            .id();
        let play = app.world_mut().spawn(PickerPlay).id();
        app.update();
        assert_eq!(app.world().get::<Text>(title).unwrap().0, "Song");
        assert_eq!(
            app.world().get::<Node>(empty).unwrap().display,
            Display::None
        );
        assert!(
            app.world()
                .get::<bevy::ui::InteractionDisabled>(play)
                .is_none()
        );
        app.world_mut().resource_mut::<SongPickerState>().query = "jazz".into();
        app.update();
        assert_eq!(
            app.world().get::<Node>(empty).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world()
                .get::<bevy::ui::InteractionDisabled>(play)
                .is_some()
        );
    }

    #[test]
    fn column_sorting_reverses_each_metadata_order() {
        let mut available = AvailableSongs::default();
        available.0.insert(
            "catalog".into(),
            vec![
                SongEntry {
                    artist: "Alpha".into(),
                    name: "Zulu".into(),
                    genre: "Rock".into(),
                    difficulty: "expert".into(),
                    asset_path: "a".into(),
                },
                SongEntry {
                    artist: "Zulu".into(),
                    name: "Alpha".into(),
                    genre: "Blues".into(),
                    difficulty: "easy".into(),
                    asset_path: "b".into(),
                },
            ],
        );
        for (sort, first) in [
            (SongSort::Song, "b"),
            (SongSort::Band, "a"),
            (SongSort::Genre, "b"),
            (SongSort::Difficulty, "b"),
        ] {
            let mut state = SongPickerState { sort, ..default() };
            let ascending = collect_songs(&available, &state);
            assert_eq!(ascending[0].asset_path, first);
            state.descending = true;
            let descending = collect_songs(&available, &state);
            assert_eq!(descending[0].asset_path, ascending[1].asset_path);
            assert_eq!(descending[1].asset_path, ascending[0].asset_path);
        }
    }

    #[test]
    fn fuzzy_search_matches_fragments_and_typographical_errors() {
        let fields = ["Wonderful Tonight", "Eric Clapton", "Blues Rock"];
        for query in [
            "",
            "   ",
            "WONDER",
            "tonig",
            "claptn",
            "clappton",
            "clapten",
            "wondr tonigt",
            "claptn rock",
            "wonder eric",
        ] {
            assert!(
                matches_search(query, &fields),
                "expected match for {query:?}"
            );
        }
        for query in ["jazz", "wonder jazz", "zz", "rok", "abcdefgh"] {
            assert!(
                !matches_search(query, &fields),
                "unexpected match for {query:?}"
            );
        }
    }

    #[test]
    fn fuzzy_search_handles_unicode_characters() {
        assert!(matches_search("étoi", &["Étoile"]));
        assert!(matches_search("etoile", &["Étoile"]));
        assert!(!matches_search("東京", &["京都"]));
    }

    #[test]
    fn filtering_keeps_existing_song_entities_and_focus() {
        let mut app = App::new();
        app.init_resource::<InputFocus>()
            .init_resource::<SongPickerState>()
            .init_resource::<AvailableSongs>()
            .add_message::<SongsRescanned>()
            .add_systems(Update, refresh_song_picker);
        app.world_mut().resource_mut::<AvailableSongs>().0.insert(
            "Eric Clapton".into(),
            vec![SongEntry {
                artist: "Eric Clapton".into(),
                name: "Wonderful Tonight".into(),
                genre: "Rock".into(),
                difficulty: "easy".into(),
                asset_path: "song".into(),
            }],
        );
        let root = app.world_mut().spawn(SongPickerRows).id();
        let row = app
            .world_mut()
            .spawn(SongRow {
                path: "song".into(),
                index: 0,
            })
            .id();
        app.world_mut().entity_mut(root).add_child(row);
        let input = app
            .world_mut()
            .spawn((SongPickerSearch, EditableText::new("")))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(input, FocusCause::Navigated);
        app.update();
        for query in ["w", "won", "wondr", "wonderful"] {
            app.world_mut()
                .get_mut::<EditableText>(input)
                .unwrap()
                .editor_mut()
                .set_text(query);
            app.update();
            assert_eq!(app.world().get::<Children>(root).unwrap()[0], row);
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(input));
            assert_eq!(
                app.world()
                    .resource::<SongPickerState>()
                    .selected
                    .as_deref(),
                Some("song")
            );
        }
        app.world_mut()
            .get_mut::<EditableText>(input)
            .unwrap()
            .editor_mut()
            .set_text("jazz");
        app.update();
        assert!(app.world().get::<SongRow>(row).is_none());
        assert!(app.world().resource::<SongPickerState>().selected.is_none());
    }

    fn navigation_app() -> (App, Vec<Entity>) {
        let mut app = App::new();
        app.init_resource::<InputFocus>()
            .init_resource::<InputFocusVisible>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SongPickerState>()
            .add_systems(Update, navigate_song_picker);
        let rows = (0..3)
            .map(|index| {
                app.world_mut()
                    .spawn((
                        SongRow {
                            path: format!("song-{index}"),
                            index,
                        },
                        ComputedNode::default(),
                    ))
                    .id()
            })
            .collect();
        (app, rows)
    }

    #[test]
    fn unchanged_keyboard_focus_preserves_mouse_preview() {
        let (mut app, rows) = navigation_app();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(rows[0], FocusCause::Navigated);
        app.update();
        app.world_mut().resource_mut::<SongPickerState>().selected = Some("song-2".into());
        app.update();
        assert_eq!(
            app.world()
                .resource::<SongPickerState>()
                .selected
                .as_deref(),
            Some("song-2")
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();
        assert_eq!(
            app.world()
                .resource::<SongPickerState>()
                .selected
                .as_deref(),
            Some("song-1")
        );
    }

    #[test]
    fn picker_navigation_moves_and_clamps_at_list_ends() {
        let (mut app, rows) = navigation_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::End);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(rows[2]));
        assert_eq!(
            app.world()
                .resource::<SongPickerState>()
                .selected
                .as_deref(),
            Some("song-2")
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(rows[2]));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(rows[1]));
    }

    #[test]
    fn picker_navigation_keeps_search_keyboard_input_in_the_field() {
        let (mut app, _) = navigation_app();
        let input = app.world_mut().spawn(EditableText::new("search")).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(input, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::End);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(input));
        assert!(app.world().resource::<SongPickerState>().selected.is_none());
    }
}
