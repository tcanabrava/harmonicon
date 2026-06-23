// SPDX-License-Identifier: MIT

use bevy::audio::AudioSource;
use bevy::picking::Pickable;
use bevy::picking::events::{Out, Over, Pointer, Press};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button as WidgetButton, UiWidgetsPlugins};

use crate::assets_management::{AvailableSongs, GlobalFonts};
use crate::song::SongManifest;
use crate::theme::LoadedTheme;

pub(crate) mod button_material;
mod calibration;
mod credits;
mod options;
mod song_editor;
mod theme_picker;

use button_material::{
    ButtonMaterialPlugin, ButtonMaterials, ButtonShaderLayer, ButtonVisual, ThemedButton,
    set_button_visual,
};

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

/// Set to `true` by the calibration screen so that returning to `AppState::Menu`
/// lands on the Options page (where the Input lag slider lives).
#[derive(Resource, Default)]
pub struct ReturnToOptions(pub bool);

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
    /// Latency calibration screen (outside the menu sub-state hierarchy).
    Calibration,
    /// Credits screen with scrolling text and 3D harmonica background.
    Credits,
    /// Song authoring tool, launched from the main menu.
    SongEditor,
}

// ── Menu sub-states (only active while AppState == Menu) ──────────────────────

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(AppState = AppState::Menu)]
pub(crate) enum MenuPage {
    #[default]
    Main,
    Play,
    ArtistList,
    SongList,
    ModeSelect,
    Options,
    Theme,
}

// ── Public resources ──────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct SelectedSong(pub Handle<SongManifest>);
// ── Private resources / components ───────────────────────────────────────────

#[derive(Resource, Default)]
struct SelectedArtist(String);

/// Marks every entity that belongs to a menu screen so `cleanup_menu` can
/// remove it in one sweep when the page changes. Shared with the `options` page.
#[derive(Component, Default, Clone)]
pub(super) struct MenuRoot;

#[derive(Component, Clone)]
pub(super) enum MenuButton {
    // Main menu
    Play,
    SongEditor,
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
    // Options utilities
    Calibrate,
    Theme,
    BackToOptions,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<SelectedArtist>()
            .init_resource::<GameplayMode>()
            .init_resource::<ReturnToSongList>()
            .init_resource::<ReturnToOptions>()
            // The Options, Calibration, Credits, and Theme pages own their own lifecycles.
            .add_plugins(ButtonMaterialPlugin)
            .add_plugins(options::OptionsPlugin)
            .add_plugins(calibration::CalibrationPlugin)
            .add_plugins(credits::CreditsPlugin)
            .add_plugins(song_editor::SongEditorPlugin)
            .add_plugins(theme_picker::ThemePickerPlugin)
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
            // Button clicks/hover are wired per-button as observers in
            // `spawn_button`, so there's no central interaction system here.
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
pub(super) fn btn_default() -> Color {
    Color::srgb(0.14, 0.14, 0.22)
}

/// Spawn a full-screen centred column with a title and optional subtitle.
/// Returns the entity so the caller can add button children afterwards.
/// `menu_id` is matched against the theme's `menus` keys (e.g. "Main", "Play")
/// to look up the per-menu background image.
/// The menu root container as a `bsn!` [`Scene`]: a full-screen centred column.
fn menu_root_scene() -> impl Scene {
    bsn! {
        Node {
            width: {Val::Percent(100.0)},
            height: {Val::Percent(100.0)},
            flex_direction: {FlexDirection::Column},
            align_items: {AlignItems::Center},
            justify_content: {JustifyContent::Center},
            row_gap: {Val::Px(16.0)},
        }
        BackgroundColor({menu_bg()})
        MenuRoot
    }
}

/// A heading text line as a `bsn!` [`Scene`]. Uses the default font (no custom
/// `FontSource`, which `bsn!` can't take directly in 0.19-rc.3).
fn heading_scene(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text({text})
        TextFont { font_size: {FontSize::Px(size)} }
        TextColor({color})
    }
}

pub(super) fn spawn_menu_root(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
    theme: &LoadedTheme,
    menu_id: &str,
) -> Entity {
    // Root container + title (+ optional subtitle) as one composed scene. The
    // subtitle is conditional, so the two `Children [...]` shapes are spawned in
    // separate branches (each `bsn!` is a distinct concrete `Scene` type).
    let title = title.to_string();
    let root = if let Some(sub) = subtitle {
        commands
            .spawn_scene(bsn! {
                menu_root_scene()
                Children [
                    heading_scene(title, 52.0, Color::WHITE),
                    heading_scene(sub.to_string(), 20.0, Color::srgb(0.6, 0.6, 0.7)),
                ]
            })
            .id()
    } else {
        commands
            .spawn_scene(bsn! {
                menu_root_scene()
                Children [ heading_scene(title, 52.0, Color::WHITE) ]
            })
            .id()
    };

    // Background image behind all other content. Inserted at index 0 so it stays
    // the lowest layer regardless of when this command is applied.
    if let Some(bg) = theme.background_for(menu_id) {
        let bg_layer = commands
            .spawn((
                ImageNode::new(bg.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
            ))
            .id();
        commands.entity(root).insert_children(0, &[bg_layer]);
    }
    root
}

/// Maps a `MenuButton` variant to the string id used in `theme.json`.
/// Dynamic variants (Artist, Song) return `None` and fall back to flow layout.
fn button_id(btn: &MenuButton) -> Option<&'static str> {
    match btn {
        MenuButton::Play => Some("Play"),
        MenuButton::SongEditor => Some("SongEditor"),
        MenuButton::Options => Some("Options"),
        MenuButton::Credits => Some("Credits"),
        MenuButton::Quit => Some("Quit"),
        MenuButton::PlaySong => Some("PlaySong"),
        MenuButton::JamSession => Some("JamSession"),
        MenuButton::PlayMode2D => Some("PlayMode2D"),
        MenuButton::PlayMode3D => Some("PlayMode3D"),
        MenuButton::BackToMain => Some("BackToMain"),
        MenuButton::BackToPlay => Some("BackToPlay"),
        MenuButton::BackToArtistList => Some("BackToArtistList"),
        MenuButton::Calibrate => Some("Calibrate"),
        MenuButton::Theme => Some("Theme"),
        MenuButton::BackToOptions => Some("BackToOptions"),
        MenuButton::Artist(_) | MenuButton::Song(_) => None,
    }
}

/// Spawn a single button as a child of `parent`.
///
/// When the theme JSON defines coords for this button in `menu_id`, the button
/// is absolutely positioned at those pixel coordinates. Otherwise it joins the
/// normal flex flow of the parent.
///
/// When the theme has shaders the button also gets a smoke background layer,
/// an optional icon, and audio on hover/click.
pub(super) fn spawn_button(
    commands: &mut Commands,
    parent: Entity,
    font: &FontSource,
    label: &str,
    btn: MenuButton,
    theme: &LoadedTheme,
    btn_mats: &ButtonMaterials,
    menu_id: &str,
) {
    // Resolve pixel coords from the theme JSON (if defined for this button).
    let coords = button_id(&btn)
        .and_then(|id| theme.button_coords(menu_id, id))
        .cloned();

    // Build the layout node: absolute when coords exist, flex-flow otherwise.
    let node = match &coords {
        Some(c) => Node {
            position_type: PositionType::Absolute,
            left: Val::Px(c.x),
            top: Val::Px(c.y),
            width: Val::Px(c.width),
            height: Val::Px(c.height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        None => Node {
            min_width: Val::Px(260.0),
            padding: UiRect::axes(Val::Px(32.0), Val::Px(14.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    };

    // Captured by the click observer below; `btn` itself is moved into the entity.
    let btn_action = btn.clone();

    // Children are `Pickable::IGNORE` so the pointer always hits the button
    // itself (not the text/icon), keeping the hover/press observers below
    // robust — otherwise picking would target a child and the button would
    // flicker between hovered/unhovered.
    let button = if theme.has_shaders {
        let e = commands
            .spawn((WidgetButton, node, btn, ThemedButton))
            .id();

        // Smoke shader layer — absolute, behind content. Keep its entity so the
        // pointer observers can swap its material.
        let layer = commands
            .spawn((
                MaterialNode(btn_mats.idle.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
                ButtonShaderLayer,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(e).add_child(layer);

        commands.entity(e).with_children(|b| {
            // Icon from theme (optional)
            if let Some(ref icon) = theme.btn_icon {
                b.spawn((
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ImageNode {
                        image: icon.clone(),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            }

            b.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });

        // Themed hover/press visuals via observers (replaces the old
        // Changed<Interaction> system in button_material).
        commands.entity(e).observe(
            move |_: On<Pointer<Over>>,
                  mats: Res<ButtonMaterials>,
                  theme: Res<LoadedTheme>,
                  mut commands: Commands| {
                set_button_visual(&mut commands, layer, ButtonVisual::Hover, &mats);
                if let Some(ref snd) = theme.btn_sound_hover {
                    commands.spawn((
                        AudioPlayer::<AudioSource>(snd.clone()),
                        PlaybackSettings::DESPAWN,
                    ));
                }
            },
        );
        commands.entity(e).observe(
            move |_: On<Pointer<Out>>, mats: Res<ButtonMaterials>, mut commands: Commands| {
                set_button_visual(&mut commands, layer, ButtonVisual::Idle, &mats);
            },
        );
        commands.entity(e).observe(
            move |_: On<Pointer<Press>>,
                  mats: Res<ButtonMaterials>,
                  theme: Res<LoadedTheme>,
                  mut commands: Commands| {
                set_button_visual(&mut commands, layer, ButtonVisual::Click, &mats);
                if let Some(ref snd) = theme.btn_sound_click {
                    commands.spawn((
                        AudioPlayer::<AudioSource>(snd.clone()),
                        PlaybackSettings::DESPAWN,
                    ));
                }
            },
        );

        e
    } else {
        let e = commands
            .spawn((WidgetButton, node, BackgroundColor(btn_default()), btn))
            .id();

        commands.entity(e).with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    font: font.clone(),
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });

        // Plain buttons highlight on hover via observers.
        commands.entity(e).observe(move |_: On<Pointer<Over>>, mut q: Query<&mut BackgroundColor>| {
            if let Ok(mut bg) = q.get_mut(e) {
                *bg = BackgroundColor(Color::srgb(0.20, 0.20, 0.32));
            }
        });
        commands.entity(e).observe(move |_: On<Pointer<Out>>, mut q: Query<&mut BackgroundColor>| {
            if let Ok(mut bg) = q.get_mut(e) {
                *bg = BackgroundColor(btn_default());
            }
        });

        e
    };

    // Activating the button (ui_widgets::Button emits `Activate` on click or
    // keyboard) performs its navigation/selection — observer style, no central
    // Changed<Interaction> system.
    commands.entity(button).observe(
        move |_: On<Activate>,
              mut next_page: ResMut<NextState<MenuPage>>,
              mut next_state: ResMut<NextState<AppState>>,
              mut selected_artist: ResMut<SelectedArtist>,
              mut gameplay_mode: ResMut<GameplayMode>,
              asset_server: Res<AssetServer>,
              mut app_exit: MessageWriter<AppExit>,
              mut commands: Commands| {
            // Selections that accompany a navigation.
            match &btn_action {
                MenuButton::JamSession => *gameplay_mode = GameplayMode::JamSession,
                MenuButton::PlayMode2D => *gameplay_mode = GameplayMode::Play2D,
                MenuButton::PlayMode3D => *gameplay_mode = GameplayMode::Play3D,
                MenuButton::Artist(a) => selected_artist.0 = a.clone(),
                MenuButton::Song(path) => {
                    commands.insert_resource(SelectedSong(asset_server.load::<SongManifest>(path.clone())));
                }
                _ => {}
            }
            match menu_nav(&btn_action) {
                MenuNav::To(page) => next_page.set(page),
                MenuNav::Enter(state) => next_state.set(state),
                MenuNav::Quit => {
                    app_exit.write(AppExit::Success);
                }
                MenuNav::Stay => {}
            }
        },
    );

    commands.entity(parent).add_child(button);
}

// ── Menu pages ────────────────────────────────────────────────────────────────

fn setup_main_menu(
    mut commands: Commands,
    font: Res<GlobalFonts>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
) {
    let root = spawn_menu_root(&mut commands, "Harmonicon", None, &theme, "Main");
    let font = font.gameplay.clone();
    spawn_button(&mut commands, root, &font, "Play", MenuButton::Play, &theme, &btn_mats, "Main");
    spawn_button(&mut commands, root, &font, "Song Editor", MenuButton::SongEditor, &theme, &btn_mats, "Main");
    spawn_button(&mut commands, root, &font, "Options", MenuButton::Options, &theme, &btn_mats, "Main");
    spawn_button(&mut commands, root, &font, "Credits", MenuButton::Credits, &theme, &btn_mats, "Main");
    spawn_button(&mut commands, root, &font, "Quit", MenuButton::Quit, &theme, &btn_mats, "Main");
}

fn setup_play_menu(
    mut commands: Commands,
    font: Res<GlobalFonts>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
) {
    let root = spawn_menu_root(&mut commands, "Play", None, &theme, "Play");
    spawn_button(&mut commands, root, &font.gameplay, "Play Song", MenuButton::PlaySong, &theme, &btn_mats, "Play");
    spawn_button(&mut commands, root, &font.gameplay, "Jam Session", MenuButton::JamSession, &theme, &btn_mats, "Play");
    spawn_button(&mut commands, root, &font.symbols, "\u{2190} Back", MenuButton::BackToMain, &theme, &btn_mats, "Play");
}

fn setup_artist_list(
    mut commands: Commands,
    font: Res<GlobalFonts>,
    songs: Res<AvailableSongs>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
) {
    let root = spawn_menu_root(&mut commands, "Select Artist", None, &theme, "ArtistList");

    if songs.0.is_empty() {
        // NOTE: kept imperative — `TextFont.font` is a templated `FontSource`
        // (derives `FromTemplate`) with no `From<FontSource>` for its generated
        // template type, so `bsn!` can't take our `FontSource` handle directly.
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
            spawn_button(&mut commands, root, &font.gameplay, &label, MenuButton::Artist(artist.clone()), &theme, &btn_mats, "ArtistList");
        }
    }
    spawn_button(&mut commands, root, &font.symbols, "\u{2190} Back", MenuButton::BackToPlay, &theme, &btn_mats, "ArtistList");
}

fn setup_song_list(
    mut commands: Commands,
    songs: Res<AvailableSongs>,
    selected_artist: Res<SelectedArtist>,
    font: Res<GlobalFonts>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
) {
    let subtitle = format!("by {}", selected_artist.0);
    let root = spawn_menu_root(&mut commands, "Select Song", Some(&subtitle), &theme, "SongList");

    if let Some(artist_songs) = songs.0.get(&selected_artist.0) {
        let mut sorted = artist_songs.clone();
        sorted.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        for song in &sorted {
            spawn_button(&mut commands, root, &font.gameplay, &song.name, MenuButton::Song(song.asset_path.clone()), &theme, &btn_mats, "SongList");
        }
    }
    spawn_button(&mut commands, root, &font.symbols, "\u{2190} Back", MenuButton::BackToArtistList, &theme, &btn_mats, "SongList");
}

fn setup_mode_select(
    mut commands: Commands,
    font: Res<GlobalFonts>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
) {
    let root = spawn_menu_root(&mut commands, "Select Mode", None, &theme, "ModeSelect");
    spawn_button(&mut commands, root, &font.gameplay, "Play 2D", MenuButton::PlayMode2D, &theme, &btn_mats, "ModeSelect");
    spawn_button(&mut commands, root, &font.gameplay, "Play 3D", MenuButton::PlayMode3D, &theme, &btn_mats, "ModeSelect");
    spawn_button(&mut commands, root, &font.symbols, "\u{2190} Back", MenuButton::BackToPlay, &theme, &btn_mats, "ModeSelect");
}

// ── Input + hover ─────────────────────────────────────────────────────────────

/// Where a menu button leads. Separated from the side effects (mode/artist/song
/// selection) so the navigation graph — which page each action opens and which
/// parent each "Back" closes to — is a pure, testable function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MenuNav {
    /// Switch to another menu page.
    To(MenuPage),
    /// Leave the menu for an app state (start loading the chosen song).
    Enter(AppState),
    /// Quit the game.
    Quit,
    /// Do nothing (e.g. unimplemented Credits).
    Stay,
}

/// The navigation a button triggers. Forward buttons open deeper pages; `Back*`
/// buttons close to the correct parent.
pub(super) fn menu_nav(button: &MenuButton) -> MenuNav {
    match button {
        MenuButton::Play => MenuNav::To(MenuPage::Play),
        MenuButton::SongEditor => MenuNav::Enter(AppState::SongEditor),
        MenuButton::Options => MenuNav::To(MenuPage::Options),
        MenuButton::Credits => MenuNav::Enter(AppState::Credits),
        MenuButton::Quit => MenuNav::Quit,
        // The render mode is chosen up front, before picking a song.
        MenuButton::PlaySong => MenuNav::To(MenuPage::ModeSelect),
        MenuButton::JamSession => MenuNav::To(MenuPage::ArtistList),
        MenuButton::PlayMode2D | MenuButton::PlayMode3D => MenuNav::To(MenuPage::ArtistList),
        MenuButton::Artist(_) => MenuNav::To(MenuPage::SongList),
        // The mode is already chosen — picking a song starts the game.
        MenuButton::Song(_) => MenuNav::Enter(AppState::SongLoading),
        MenuButton::BackToMain => MenuNav::To(MenuPage::Main),
        MenuButton::BackToPlay => MenuNav::To(MenuPage::Play),
        MenuButton::BackToArtistList => MenuNav::To(MenuPage::ArtistList),
        MenuButton::Calibrate => MenuNav::Enter(AppState::Calibration),
        MenuButton::Theme => MenuNav::To(MenuPage::Theme),
        MenuButton::BackToOptions => MenuNav::To(MenuPage::Options),
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

pub(super) fn cleanup_menu(mut commands: Commands, roots: Query<Entity, With<MenuRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

/// On entering the menu, jump straight to the song list if we just quit a song
/// (so "Quit Song" returns to the list, not the main menu). Otherwise the menu
/// opens on its default page (Main), unless a return-to flag says otherwise.
fn route_menu_entry(
    mut ret_song: ResMut<ReturnToSongList>,
    mut ret_opts: ResMut<ReturnToOptions>,
    mut next_page: ResMut<NextState<MenuPage>>,
) {
    if ret_song.0 {
        ret_song.0 = false;
        next_page.set(MenuPage::SongList);
    } else if ret_opts.0 {
        ret_opts.0 = false;
        next_page.set(MenuPage::Options);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    // ── button_id ─────────────────────────────────────────────────────────

    #[test]
    fn button_id_maps_every_static_variant_to_its_json_id() {
        use MenuButton::*;
        let cases = [
            (Play,            "Play"),
            (SongEditor,      "SongEditor"),
            (Options,         "Options"),
            (Credits,         "Credits"),
            (Quit,            "Quit"),
            (PlaySong,        "PlaySong"),
            (JamSession,      "JamSession"),
            (PlayMode2D,      "PlayMode2D"),
            (PlayMode3D,      "PlayMode3D"),
            (BackToMain,      "BackToMain"),
            (BackToPlay,      "BackToPlay"),
            (BackToArtistList,"BackToArtistList"),
            (Calibrate,       "Calibrate"),
            (Theme,           "Theme"),
            (BackToOptions,   "BackToOptions"),
        ];
        for (btn, expected) in cases {
            assert_eq!(button_id(&btn), Some(expected), "variant {expected}");
        }
    }

    #[test]
    fn button_id_returns_none_for_dynamic_variants() {
        assert!(button_id(&MenuButton::Artist("x".into())).is_none());
        assert!(button_id(&MenuButton::Song("path/to.toml".into())).is_none());
    }

    // ── navigation_graph ──────────────────────────────────────────────────

    #[test]
    fn navigation_graph_opens_and_closes_to_the_right_pages() {
        use MenuButton::*;
        // Forward — each action opens the next page down the hierarchy.
        assert_eq!(menu_nav(&Play), MenuNav::To(MenuPage::Play));
        assert_eq!(menu_nav(&Options), MenuNav::To(MenuPage::Options));
        assert_eq!(menu_nav(&PlaySong), MenuNav::To(MenuPage::ModeSelect));
        assert_eq!(menu_nav(&PlayMode2D), MenuNav::To(MenuPage::ArtistList));
        assert_eq!(menu_nav(&PlayMode3D), MenuNav::To(MenuPage::ArtistList));
        assert_eq!(menu_nav(&JamSession), MenuNav::To(MenuPage::ArtistList));
        assert_eq!(
            menu_nav(&Artist("x".into())),
            MenuNav::To(MenuPage::SongList)
        );
        assert_eq!(
            menu_nav(&Song("p".into())),
            MenuNav::Enter(AppState::SongLoading)
        );
        // Back — each closes to its correct parent.
        assert_eq!(
            menu_nav(&BackToArtistList),
            MenuNav::To(MenuPage::ArtistList)
        );
        assert_eq!(menu_nav(&BackToPlay), MenuNav::To(MenuPage::Play));
        assert_eq!(menu_nav(&BackToMain), MenuNav::To(MenuPage::Main));
        // Terminal actions.
        assert_eq!(menu_nav(&Quit), MenuNav::Quit);
        assert_eq!(menu_nav(&Credits), MenuNav::Enter(AppState::Credits));
        assert_eq!(menu_nav(&SongEditor), MenuNav::Enter(AppState::SongEditor));
    }

    // Records page enter/exit so the close-then-open order can be asserted.
    #[derive(Resource, Default)]
    struct PageLog(Vec<String>);

    fn track_page(app: &mut App, page: MenuPage, label: &'static str) {
        app.add_systems(OnEnter(page.clone()), move |mut log: ResMut<PageLog>| {
            log.0.push(format!("enter {label}"))
        });
        app.add_systems(OnExit(page), move |mut log: ResMut<PageLog>| {
            log.0.push(format!("exit {label}"))
        });
    }

    #[test]
    fn changing_page_exits_the_old_before_entering_the_new() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<PageLog>();
        track_page(&mut app, MenuPage::Main, "Main");
        track_page(&mut app, MenuPage::Play, "Play");

        // Enter the menu → its default page (Main) opens.
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        app.update();
        // Open Play (Main must close first), then go Back to Main (Play closes).
        app.world_mut()
            .resource_mut::<NextState<MenuPage>>()
            .set(MenuPage::Play);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<MenuPage>>()
            .set(MenuPage::Main);
        app.update();

        let log = &app.world().resource::<PageLog>().0;
        assert_eq!(
            log,
            &[
                "enter Main",
                "exit Main",
                "enter Play",
                "exit Play",
                "enter Main"
            ],
        );
    }
}
