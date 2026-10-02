// SPDX-License-Identifier: MIT

//! Unified song catalog with an inline 2D/3D toggle, sorting and filtering.

use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::ScrollArea;
use bevy::ui_widgets::{Activate, Button as WidgetButton, ValueChange};

use harmonicon_app::app::{GameplayMode, SelectedSong};
use harmonicon_platform::assets_management::{AvailableSongs, SongEntry, SongsRescanned};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::checkbox::spawn_checkbox;
use harmonicon_ui::dialogs::text_input::spawn_text_input;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_menu_root_plain};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SongSort {
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
}

#[derive(Component)]
pub(crate) struct SongPickerRows;

#[derive(Component)]
pub(crate) struct SongPickerSearch;

pub(crate) fn setup_artist_list(
    mut commands: Commands,
    songs: Res<AvailableSongs>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    mode: Res<GameplayMode>,
) {
    commands.insert_resource(SongPickerState::default());
    let (content, header, _) = spawn_menu_root_plain(
        &mut commands,
        &loc.msg("select-song"),
        None,
        &theme,
        "SongPicker",
    );
    spawn_mode_toggle(&mut commands, content, *mode == GameplayMode::Play2D);
    let sort_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(content).add_child(sort_row);
    for (sort, label) in [
        (SongSort::Band, loc.msg("song-sort-band").to_string()),
        (
            SongSort::Difficulty,
            loc.msg("song-sort-difficulty").to_string(),
        ),
        (SongSort::Song, loc.msg("song-sort-name").to_string()),
        (SongSort::Genre, loc.msg("song-sort-genre").to_string()),
    ] {
        let entity = commands
            .spawn((
                WidgetButton,
                Node {
                    min_width: Val::Px(105.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ))
            .id();
        commands
            .entity(entity)
            .observe(move |_: On<Activate>, mut state: ResMut<SongPickerState>| state.sort = sort);
        commands.entity(sort_row).add_child(entity);
    }
    let search_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
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
        "",
        420.0,
        Color::srgb(0.12, 0.12, 0.17),
        Color::srgb(0.30, 0.30, 0.40),
        |ev: On<harmonicon_ui::dialogs::text_input::TextInputCommitted>,
         mut state: ResMut<SongPickerState>| state.query = ev.value.to_lowercase(),
    );
    commands.entity(search).insert(SongPickerSearch);
    let rows = commands
        .spawn((
            SongPickerRows,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                min_height: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollArea,
        ))
        .id();
    commands.entity(content).add_child(rows);
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
    populate_rows(&mut commands, rows, &songs, &SongPickerState::default());
}

fn spawn_mode_toggle(commands: &mut Commands, parent: Entity, is_2d: bool) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let two_d = commands
        .spawn((
            Text::new("2d"),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();
    let three_d = commands
        .spawn((
            Text::new("3d"),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();
    commands.entity(row).add_child(two_d);
    spawn_checkbox(
        commands,
        row,
        "",
        is_2d,
        |ev: On<ValueChange<bool>>, mut mode: ResMut<GameplayMode>| {
            *mode = if ev.value {
                GameplayMode::Play2D
            } else {
                GameplayMode::Play3D
            };
        },
    );
    commands.entity(row).add_child(three_d);
    commands.entity(parent).add_child(row);
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

fn collect_songs(available: &AvailableSongs, state: &SongPickerState) -> Vec<SongEntry> {
    let query = state.query.trim();
    let mut entries: Vec<_> = available
        .0
        .values()
        .flatten()
        .filter(|song| {
            query.is_empty()
                || song.name.to_lowercase().contains(query)
                || song.artist.to_lowercase().contains(query)
                || song.genre.to_lowercase().contains(query)
        })
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
    entries
}

fn populate_rows(
    commands: &mut Commands,
    root: Entity,
    songs: &AvailableSongs,
    state: &SongPickerState,
) {
    for song in collect_songs(songs, state) {
        let label = format!(
            "{}     {} · {} · {}",
            song.name, song.artist, song.genre, song.difficulty
        );
        let path = song.asset_path.clone();
        let row = commands
            .spawn((
                bevy::ui_widgets::Button,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                    justify_content: JustifyContent::FlexStart,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.11, 0.11, 0.16)),
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ))
            .id();
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
        commands.entity(root).add_child(row);
    }
}

pub(crate) fn refresh_song_picker(
    mut commands: Commands,
    mut rescanned: MessageReader<SongsRescanned>,
    songs: Res<AvailableSongs>,
    inputs: Query<&EditableText, With<SongPickerSearch>>,
    roots: Query<Entity, With<SongPickerRows>>,
    mut state: ResMut<SongPickerState>,
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
    if !input_changed && !rescanned && !state.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn_related::<Children>();
        populate_rows(&mut commands, root, &songs, &state);
    }
}
