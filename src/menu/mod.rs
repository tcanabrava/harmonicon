use bevy::prelude::*;

use crate::song::SongManifest;

pub struct MenuPlugin;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Startup,
    Menu,
    SongLoading,
    Playing,
}

#[derive(Resource)]
pub struct SelectedSong(pub Handle<SongManifest>);

#[derive(Debug, Clone)]
pub struct SongEntry {
    pub artist: String,
    pub name: String,
    pub asset_path: String,
}

#[derive(Resource, Default)]
pub struct AvailableSongs(pub Vec<SongEntry>);

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct SongButton(String);

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
        .init_resource::<AvailableSongs>()

        .add_systems(OnEnter(AppState::Startup), scan_songs)
        .add_systems(Update, startup_complete.run_if(in_state(AppState::Startup)))

        .add_systems(OnEnter(AppState::Menu), setup_menu)
        .add_systems(Update,
            handle_song_selection.run_if(in_state(AppState::Menu)))
        .add_systems(OnExit(AppState::Menu), cleanup_menu)

        .add_systems(Update,
            check_loading.run_if(in_state(AppState::SongLoading)))
        ;
    }
}

fn startup_complete(mut next_state: ResMut<NextState<AppState>>) {
    info!("Startup complete");
    next_state.set(AppState::Menu);
}

fn finish_loading(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Menu);
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn scan_songs(mut available: ResMut<AvailableSongs>, mut next_state: ResMut<NextState<AppState>>) {
    let songs_root = std::path::Path::new("assets/songs");

    let Ok(artists) = std::fs::read_dir(songs_root) else {
        warn!("No songs directory found at assets/songs/");
        return;
    };

    let mut entries = Vec::new();

    for artist_dir in artists.flatten() {
        if !artist_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let artist = artist_dir.file_name().to_string_lossy().into_owned();

        let Ok(song_dirs) = std::fs::read_dir(artist_dir.path()) else {
            continue;
        };

        for song_dir in song_dirs.flatten() {
            if !song_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            if !song_dir.path().join("chart.harpchart").exists() {
                continue;
            }
            let name = song_dir.file_name().to_string_lossy().into_owned();
            entries.push(SongEntry {
                asset_path: format!("songs/{artist}/{name}/chart.harpchart"),
                artist: artist.clone(),
                name,
            });
        }
    }
    available.0 = entries;

    info!("Found {} song(s)", available.0.len());
}

fn setup_menu(mut commands: Commands, songs: Res<AvailableSongs>) {
    info!("Setting up menu");
    commands
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
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            MenuRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Harmonicon"),
                TextFont { font_size: 52.0, ..default() },
                TextColor(Color::WHITE),
            ));

            root.spawn((
                Text::new("Choose a song"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.6, 0.6, 0.7)),
            ));

            if songs.0.is_empty() {
                info!("MENU: Found {} song(s)", songs.0.len());
                root.spawn((
                    Text::new("No songs found — add folders under assets/songs/<artist>/<song>/"),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(Color::srgb(0.8, 0.4, 0.4)),
                ));
                return;
            }

            for song in &songs.0 {
                root.spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(32.0), Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.18, 0.28)),
                    SongButton(song.asset_path.clone()),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(format!("{} — {}", song.artist, song.name)),
                        TextFont { font_size: 20.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
}

fn handle_song_selection(
    buttons: Query<(&Interaction, &SongButton), Changed<Interaction>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, song_button) in &buttons {
        if *interaction == Interaction::Pressed {
            info!("Loading song: {}", song_button.0);
            let handle = asset_server.load::<SongManifest>(&song_button.0);
            commands.insert_resource(SelectedSong(handle));
            next_state.set(AppState::SongLoading);
        }
    }
}

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
