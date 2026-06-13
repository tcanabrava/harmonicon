use bevy::prelude::*;

use crate::assets_management::AvailableSongs;
use crate::assets_management::GlobalFonts;
use crate::song::SongManifest;

#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
pub enum GameplayMode {
    #[default]
    Play2D,
    Play3D,
    /// Free-play: the 12-bar chart + metronome, no falling notes.
    JamSession,
}

/// Set to `true` by the pause menu's "Quit Song" button so that re-entering
/// `AppState::Menu` lands on the song list rather than the main menu.
#[derive(Resource, Default)]
pub struct ReturnToSongList(pub bool);

pub struct MenuPlugin;

// ── App-level states ──────────────────────────────────────────────────────────

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Startup,
    Menu,
    SongLoading,
    Playing,
    /// Post-song results / statistics screen.
    Results,
}

// ── Menu sub-states (only active while AppState == Menu) ──────────────────────

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(AppState = AppState::Menu)]
enum MenuPage {
    #[default]
    Main,
    Play,
    ArtistList,
    SongList,
    ModeSelect,
}

// ── Public resources ──────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SelectedSong(pub Handle<SongManifest>);
// ── Private resources / components ───────────────────────────────────────────

#[derive(Resource, Default)]
struct SelectedArtist(String);

/// Marks every entity that belongs to a menu screen so `cleanup_menu` can
/// remove it in one sweep when the page changes.
#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
enum MenuButton {
    // Main menu
    Play,
    Options,
    Credits,
    Quit,
    // Play sub-menu
    PlaySong,
    JamSession,
    // Drill-down
    Artist(String),
    Song(String), // carries the asset path
    // Mode selection
    PlayMode2D,
    PlayMode3D,
    // Back navigation — each variant knows exactly where to return
    BackToMain,
    BackToPlay,
    BackToArtistList,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<SelectedArtist>()
            .init_resource::<GameplayMode>()
            .init_resource::<ReturnToSongList>()
            .add_systems(OnEnter(AppState::Menu), route_menu_entry)
            // Each page manages its own lifetime.
            .add_systems(OnEnter(MenuPage::Main), setup_main_menu)
            .add_systems(OnExit(MenuPage::Main), cleanup_menu)
            .add_systems(OnEnter(MenuPage::Play), setup_play_menu)
            .add_systems(OnExit(MenuPage::Play), cleanup_menu)
            .add_systems(OnEnter(MenuPage::ArtistList), setup_artist_list)
            .add_systems(OnExit(MenuPage::ArtistList), cleanup_menu)
            .add_systems(OnEnter(MenuPage::SongList), setup_song_list)
            .add_systems(OnExit(MenuPage::SongList), cleanup_menu)
            .add_systems(OnEnter(MenuPage::ModeSelect), setup_mode_select)
            .add_systems(OnExit(MenuPage::ModeSelect), cleanup_menu)
            // Input and hover are independent — two separate registrations.
            .add_systems(Update, handle_menu_input.run_if(in_state(AppState::Menu)))
            .add_systems(Update, button_hover.run_if(in_state(AppState::Menu)))
            // Wait for the asset to finish loading before starting gameplay.
            .add_systems(
                Update,
                check_loading.run_if(in_state(AppState::SongLoading)),
            );
    }
}
// ── UI helpers ────────────────────────────────────────────────────────────────

fn menu_bg() -> Color {
    Color::srgb(0.05, 0.05, 0.08)
}
fn btn_default() -> Color {
    Color::srgb(0.14, 0.14, 0.22)
}

/// Spawn a full-screen centred column with a title and optional subtitle.
/// Returns the entity so the caller can add button children afterwards.
fn spawn_menu_root(commands: &mut Commands, title: &str, subtitle: Option<&str>) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(menu_bg()),
            MenuRoot,
        ))
        .id();

    commands.entity(root).with_children(|p| {
        p.spawn((
            Text::new(title.to_string()),
            TextFont {
                font_size: FontSize::Px(52.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
        if let Some(sub) = subtitle {
            p.spawn((
                Text::new(sub.to_string()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.7)),
            ));
        }
    });
    root
}

/// Spawn a single button as a child of `parent_entity`.
fn spawn_button(
    commands: &mut Commands,
    parent: Entity,
    font: &FontSource,
    label: &str,
    btn: MenuButton,
) {
    let button = commands
        .spawn((
            Button,
            Node {
                min_width: Val::Px(260.0),
                padding: UiRect::axes(Val::Px(32.0), Val::Px(14.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(btn_default()),
            btn,
        ))
        .id();

    commands.entity(button).with_children(|b| {
        b.spawn((
            Text::new(label.to_string()),
            TextFont {
                font_size: FontSize::Px(20.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    });

    commands.entity(parent).add_child(button);
}

// ── Menu pages ────────────────────────────────────────────────────────────────

fn setup_main_menu(mut commands: Commands, font: Res<GlobalFonts>) {
    let root = spawn_menu_root(&mut commands, "Harmonicon", None);
    let font = font.gameplay.clone();
    spawn_button(&mut commands, root, &font, "Play", MenuButton::Play);
    spawn_button(&mut commands, root, &font, "Options", MenuButton::Options);
    spawn_button(&mut commands, root, &font, "Credits", MenuButton::Credits);
    spawn_button(&mut commands, root, &font, "Quit", MenuButton::Quit);
}

fn setup_play_menu(mut commands: Commands, font: Res<GlobalFonts>) {
    let root = spawn_menu_root(&mut commands, "Play", None);
    spawn_button(
        &mut commands,
        root,
        &font.gameplay,
        "Play Song",
        MenuButton::PlaySong,
    );
    spawn_button(
        &mut commands,
        root,
        &font.gameplay,
        "Jam Session",
        MenuButton::JamSession,
    );
    spawn_button(
        &mut commands,
        root,
        &font.symbols,
        "\u{2190} Back",
        MenuButton::BackToMain,
    );
}

fn setup_artist_list(mut commands: Commands, font: Res<GlobalFonts>, songs: Res<AvailableSongs>) {
    let root = spawn_menu_root(&mut commands, "Select Artist", None);

    if songs.0.is_empty() {
        let msg = commands
            .spawn((
                Text::new("No songs found. Add folders under assets/songs/<artist>/<song>/"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    font: font.gameplay.clone(),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.4, 0.4)),
            ))
            .id();
        commands.entity(root).add_child(msg);
    } else {
        let mut artists: Vec<&String> = songs.0.keys().collect();
        artists.sort_unstable();
        for artist in artists {
            let n = songs.0[artist].len();
            let label = format!("{artist}  ({n} song{})", if n == 1 { "" } else { "s" });
            spawn_button(
                &mut commands,
                root,
                &font.gameplay,
                &label,
                MenuButton::Artist(artist.clone()),
            );
        }
    }
    spawn_button(
        &mut commands,
        root,
        &font.symbols,
        "\u{2190} Back",
        MenuButton::BackToPlay,
    );
}

fn setup_song_list(
    mut commands: Commands,
    songs: Res<AvailableSongs>,
    selected_artist: Res<SelectedArtist>,
    font: Res<GlobalFonts>,
) {
    let subtitle = format!("by {}", selected_artist.0);
    let root = spawn_menu_root(&mut commands, "Select Song", Some(&subtitle));

    if let Some(artist_songs) = songs.0.get(&selected_artist.0) {
        let mut sorted = artist_songs.clone();
        sorted.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        for song in &sorted {
            spawn_button(
                &mut commands,
                root,
                &font.gameplay,
                &song.name,
                MenuButton::Song(song.asset_path.clone()),
            );
        }
    }
    spawn_button(
        &mut commands,
        root,
        &font.symbols,
        "\u{2190} Back",
        MenuButton::BackToArtistList,
    );
}

fn setup_mode_select(mut commands: Commands, font: Res<GlobalFonts>) {
    let root = spawn_menu_root(&mut commands, "Select Mode", None);
    spawn_button(
        &mut commands,
        root,
        &font.gameplay,
        "Play 2D",
        MenuButton::PlayMode2D,
    );
    spawn_button(
        &mut commands,
        root,
        &font.gameplay,
        "Play 3D",
        MenuButton::PlayMode3D,
    );
    spawn_button(
        &mut commands,
        root,
        &font.symbols,
        "\u{2190} Back",
        MenuButton::BackToPlay,
    );
}

// ── Input + hover ─────────────────────────────────────────────────────────────

fn handle_menu_input(
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut next_page: ResMut<NextState<MenuPage>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut selected_artist: ResMut<SelectedArtist>,
    mut gameplay_mode: ResMut<GameplayMode>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut app_exit: MessageWriter<AppExit>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            MenuButton::Play => next_page.set(MenuPage::Play),
            MenuButton::Options => { /* TODO */ }
            MenuButton::Credits => { /* TODO */ }
            MenuButton::Quit => {
                app_exit.write(AppExit::Success);
            }
            // The render mode is chosen up front, before picking a song.
            MenuButton::PlaySong => next_page.set(MenuPage::ModeSelect),
            MenuButton::JamSession => {
                *gameplay_mode = GameplayMode::JamSession;
                next_page.set(MenuPage::ArtistList);
            }
            MenuButton::PlayMode2D => {
                *gameplay_mode = GameplayMode::Play2D;
                next_page.set(MenuPage::ArtistList);
            }
            MenuButton::PlayMode3D => {
                *gameplay_mode = GameplayMode::Play3D;
                next_page.set(MenuPage::ArtistList);
            }
            MenuButton::Artist(a) => {
                selected_artist.0 = a.clone();
                next_page.set(MenuPage::SongList);
            }
            // The mode is already chosen — picking a song starts the game.
            MenuButton::Song(path) => {
                let handle = asset_server.load::<SongManifest>(path.clone());
                commands.insert_resource(SelectedSong(handle));
                next_state.set(AppState::SongLoading);
            }
            MenuButton::BackToMain => next_page.set(MenuPage::Main),
            MenuButton::BackToPlay => next_page.set(MenuPage::Play),
            MenuButton::BackToArtistList => next_page.set(MenuPage::ArtistList),
        }
    }
}

fn button_hover(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MenuButton>),
    >,
) {
    for (interaction, mut bg) in &mut buttons {
        *bg = BackgroundColor(match interaction {
            Interaction::Pressed => Color::srgb(0.25, 0.25, 0.40),
            Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
            Interaction::None => btn_default(),
        });
    }
}

// ── Loading + cleanup ─────────────────────────────────────────────────────────

fn check_loading(
    selected: Res<SelectedSong>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if asset_server.is_loaded_with_dependencies(selected.0.id()) {
        info!("Song loaded — starting game");
        next_state.set(AppState::Playing);
    }
}

fn cleanup_menu(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

/// On entering the menu, jump straight to the song list if we just quit a song
/// (so "Quit Song" returns to the list, not the main menu). Otherwise the menu
/// opens on its default page (Main).
fn route_menu_entry(
    mut ret: ResMut<ReturnToSongList>,
    mut next_page: ResMut<NextState<MenuPage>>,
) {
    if ret.0 {
        ret.0 = false;
        next_page.set(MenuPage::SongList);
    }
}
