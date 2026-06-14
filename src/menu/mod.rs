// SPDX-License-Identifier: MIT

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::assets_management::AvailableNoteThemes;
use crate::assets_management::AvailableSongs;
use crate::assets_management::GlobalFonts;
use crate::assets_management::SelectedNoteTheme;
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

/// Player-tunable audio levels (0.0–1.0, linear). Edited on the Options page and
/// read by the audio spawners (song music, metronome clicks). Adjusting the
/// music slider updates the currently playing song in real time.
#[derive(Resource)]
pub struct AudioSettings {
    pub music_volume: f32,
    pub metronome_volume: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            music_volume: 0.8,
            metronome_volume: 0.7,
        }
    }
}

impl AudioSettings {
    /// Current level for a given slider kind.
    fn value(&self, kind: VolumeSlider) -> f32 {
        match kind {
            VolumeSlider::Music => self.music_volume,
            VolumeSlider::Metronome => self.metronome_volume,
        }
    }
}

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
    Options,
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

/// Which audio level a slider controls.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum VolumeSlider {
    Music,
    Metronome,
}

/// The growing fill of a slider track; its width mirrors the bound level.
#[derive(Component)]
struct SliderFill(VolumeSlider);

/// The "NN%" readout beside a slider.
#[derive(Component)]
struct SliderValueLabel(VolumeSlider);

/// A note-theme choice button on the Options page; carries the theme name. Kept
/// separate from `MenuButton` so its highlight reflects the current selection
/// instead of the generic hover styling.
#[derive(Component)]
struct ThemeButton(String);

// ── Plugin ────────────────────────────────────────────────────────────────────

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<SelectedArtist>()
            .init_resource::<GameplayMode>()
            .init_resource::<ReturnToSongList>()
            .init_resource::<AudioSettings>()
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
            .add_systems(OnEnter(MenuPage::Options), setup_options_menu)
            .add_systems(OnExit(MenuPage::Options), cleanup_menu)
            .add_systems(
                Update,
                (
                    drag_sliders,
                    update_sliders,
                    handle_theme_buttons,
                    theme_button_visuals,
                )
                    .run_if(in_state(MenuPage::Options)),
            )
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

fn setup_options_menu(
    mut commands: Commands,
    font: Res<GlobalFonts>,
    settings: Res<AudioSettings>,
    themes: Res<AvailableNoteThemes>,
) {
    let root = spawn_menu_root(&mut commands, "Options", Some("Audio"));
    spawn_volume_slider(
        &mut commands,
        root,
        &font.gameplay,
        "Music",
        VolumeSlider::Music,
        settings.music_volume,
    );
    spawn_volume_slider(
        &mut commands,
        root,
        &font.gameplay,
        "Metronome",
        VolumeSlider::Metronome,
        settings.metronome_volume,
    );

    spawn_theme_selector(&mut commands, root, &font.gameplay, &themes.0);

    spawn_button(
        &mut commands,
        root,
        &font.symbols,
        "\u{2190} Back",
        MenuButton::BackToMain,
    );
}

/// A "Note theme" row of selectable buttons, one per discovered theme. The
/// current selection is shown by `theme_button_visuals`, not the spawn color.
fn spawn_theme_selector(
    commands: &mut Commands,
    parent: Entity,
    font: &FontSource,
    themes: &[String],
) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();

    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(110.0),
                ..default()
            },
            Text::new("Note theme"),
            TextFont {
                font_size: FontSize::Px(20.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
        for theme in themes {
            r.spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(btn_default()),
                ThemeButton(theme.clone()),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(theme.clone()),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        font: font.clone(),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        }
    });

    commands.entity(parent).add_child(row);
}

/// Apply a clicked theme button to the selected-theme resource.
fn handle_theme_buttons(
    buttons: Query<(&Interaction, &ThemeButton), Changed<Interaction>>,
    mut selected: ResMut<SelectedNoteTheme>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            selected.0 = button.0.clone();
        }
    }
}

/// Highlight the selected theme button; the rest follow normal hover styling.
fn theme_button_visuals(
    selected: Res<SelectedNoteTheme>,
    mut buttons: Query<(&Interaction, &ThemeButton, &mut BackgroundColor)>,
) {
    for (interaction, button, mut bg) in &mut buttons {
        *bg = BackgroundColor(if button.0 == selected.0 {
            Color::srgb(0.25, 0.45, 0.30)
        } else {
            match interaction {
                Interaction::Pressed => Color::srgb(0.25, 0.25, 0.40),
                Interaction::Hovered => Color::srgb(0.20, 0.20, 0.32),
                Interaction::None => btn_default(),
            }
        });
    }
}

/// One labelled slider row: `<name>  [====       ]  NN%`. The track is a `Button`
/// so it reports `Interaction`, and carries `RelativeCursorPosition` so the drag
/// system can read the cursor's position along it.
fn spawn_volume_slider(
    commands: &mut Commands,
    parent: Entity,
    font: &FontSource,
    label: &str,
    kind: VolumeSlider,
    value: f32,
) {
    let row = commands
        .spawn(Node {
            width: Val::Px(420.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .id();

    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(110.0),
                ..default()
            },
            Text::new(label.to_string()),
            TextFont {
                font_size: FontSize::Px(20.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));

        r.spawn((
            Button,
            Node {
                width: Val::Px(220.0),
                height: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.22)),
            RelativeCursorPosition::default(),
            kind,
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Percent(value * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.35, 0.75, 1.0)),
                SliderFill(kind),
            ));
        });

        r.spawn((
            Node {
                width: Val::Px(50.0),
                ..default()
            },
            Text::new(format!("{:.0}%", value * 100.0)),
            TextFont {
                font_size: FontSize::Px(18.0),
                font: font.clone(),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.7)),
            SliderValueLabel(kind),
        ));
    });

    commands.entity(parent).add_child(row);
}

/// While a slider track is pressed, set its level from the cursor's position
/// along the track. Only writes when the value actually changes so resting on a
/// pressed slider doesn't re-trigger downstream change detection every frame.
fn drag_sliders(
    mut settings: ResMut<AudioSettings>,
    sliders: Query<(&Interaction, &RelativeCursorPosition, &VolumeSlider)>,
) {
    for (interaction, rel, kind) in &sliders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(norm) = rel.normalized else {
            continue;
        };
        let frac = (norm.x + 0.5).clamp(0.0, 1.0);
        if (settings.value(*kind) - frac).abs() <= f32::EPSILON {
            continue;
        }
        match kind {
            VolumeSlider::Music => settings.music_volume = frac,
            VolumeSlider::Metronome => settings.metronome_volume = frac,
        }
    }
}

/// Mirror the current levels onto the slider fills and percentage readouts.
fn update_sliders(
    settings: Res<AudioSettings>,
    mut fills: Query<(&mut Node, &SliderFill)>,
    mut labels: Query<(&mut Text, &SliderValueLabel)>,
) {
    for (mut node, fill) in &mut fills {
        node.width = Val::Percent(settings.value(fill.0) * 100.0);
    }
    for (mut text, label) in &mut labels {
        text.0 = format!("{:.0}%", settings.value(label.0) * 100.0);
    }
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
            MenuButton::Options => next_page.set(MenuPage::Options),
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