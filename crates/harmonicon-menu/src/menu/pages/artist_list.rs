// SPDX-License-Identifier: MIT

//! Unified song catalog with an inline 2D/3D toggle, sorting and filtering.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AutoFocus, FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::{ComputedNode, ScrollPosition};
use bevy::ui_widgets::ScrollArea;
use bevy::ui_widgets::{Activate, Button as WidgetButton, ValueChange};

use harmonicon_app::app::{GameplayMode, SelectedSong};
use harmonicon_platform::assets_management::{AvailableSongs, SongEntry, SongsRescanned};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::button::{BaseButtonColor, CHOICE_SELECTED, make_interactive};
use harmonicon_ui::dialogs::checkbox::spawn_checkbox;
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
}

#[derive(Component)]
pub(crate) struct SortChoice {
    sort: SongSort,
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
        row_gap: Val::Px(16.0),
        ..default()
    });
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
                SortChoice { sort },
                Node {
                    min_width: Val::Px(105.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            ))
            .id();
        commands.entity(entity).add_child(label);
        make_interactive(&mut commands.entity(entity), Color::srgb(0.14, 0.14, 0.22));
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
        &state.query,
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
                flex_grow: 1.0,
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
    populate_rows(&mut commands, rows, &songs, &state, true);
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
    focus_rows: bool,
) {
    for (index, song) in collect_songs(songs, state).into_iter().enumerate() {
        let label = format!(
            "{}     {} · {} · {}",
            song.name, song.artist, song.genre, song.difficulty
        );
        let path = song.asset_path.clone();
        let row = commands
            .spawn((
                bevy::ui_widgets::Button,
                TabIndex(0),
                SongRow {
                    path: path.clone(),
                    index,
                },
                BorderColor::all(Color::NONE),
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    flex_shrink: 0.0,
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
        make_interactive(&mut commands.entity(row), Color::srgb(0.11, 0.11, 0.16));
        if focus_rows
            && (state.selected.as_deref() == Some(path.as_str())
                || (state.selected.is_none() && index == 0))
        {
            commands.entity(row).insert(AutoFocus);
        }
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
    mut rendered: Local<Option<(String, SongSort)>>,
    focus: Res<InputFocus>,
    song_rows: Query<(), With<SongRow>>,
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
    if !input_changed && !rescanned && rendered.as_ref() == Some(&(state.query.clone(), state.sort))
    {
        return;
    }
    *rendered = Some((state.query.clone(), state.sort));
    let visible = collect_songs(&songs, &state);
    if !visible
        .iter()
        .any(|song| Some(&song.asset_path) == state.selected.as_ref())
    {
        state.selected = visible.first().map(|song| song.asset_path.clone());
    }
    for root in &roots {
        commands.entity(root).despawn_related::<Children>();
        populate_rows(
            &mut commands,
            root,
            &songs,
            &state,
            focus.get().is_none_or(|entity| song_rows.contains(entity)),
        );
    }
}

pub(crate) fn navigate_song_picker(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    rows: Query<(Entity, &SongRow, &ComputedNode)>,
    inputs: Query<(), With<EditableText>>,
    mut areas: Query<(&ComputedNode, &mut ScrollPosition), With<SongPickerRows>>,
    mut state: ResMut<SongPickerState>,
) {
    if focus.get().is_some_and(|entity| inputs.contains(entity)) {
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
    mut rows: Query<(&SongRow, &mut BaseButtonColor, &mut BorderColor), Without<SortChoice>>,
) {
    for (entity, choice, mut color) in &mut sorts {
        let active = state.sort == choice.sort;
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
            Color::srgb(0.20, 0.30, 0.42)
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

    fn navigation_app() -> (App, Vec<Entity>) {
        let mut app = App::new();
        app.init_resource::<InputFocus>()
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
