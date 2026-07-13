// SPDX-License-Identifier: MIT

use bevy::audio::AudioSource;
use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer, Press};
use bevy::prelude::*;

use bevy_fluent::Localization;

use crate::assets_management::AvailableSongs;
use crate::localization::LocalizationExt;
use crate::song::SongManifest;
use crate::theme::LoadedTheme;
use crate::song_editor;

use super::dialogs::button;

mod calibration;
mod credits;
mod lessons;
mod options;

mod theme_picker;

use super::dialogs::button_material::{
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
    SongEditor2,
    /// Standalone bending practice: harmonica bend diagram + metronome, with a
    /// directly pickable key and adjustable tempo (no song).
    BendingTrainer,
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
    /// Curriculum list, grouped by unit (see `crate::lessons`).
    Lessons,
    /// One lesson's instructional page (+ Start for chart-backed lessons).
    LessonReader,
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

// ── Plugin ────────────────────────────────────────────────────────────────────

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<SelectedArtist>()
            .init_resource::<lessons::SelectedLesson>()
            .init_resource::<lessons::SelectedUnitIx>()
            .add_message::<lessons::LessonUnitChanged>()
            .init_resource::<GameplayMode>()
            .init_resource::<ReturnToSongList>()
            .init_resource::<ReturnToOptions>()
            // The Options, Calibration, Credits, and Theme pages own their own lifecycles.
            .add_plugins(ButtonMaterialPlugin)
            .add_plugins(options::OptionsPlugin)
            .add_plugins(calibration::CalibrationPlugin)
            .add_plugins(credits::CreditsPlugin)
            .add_plugins(song_editor::SongEditor2Plugin)
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
            .add_systems(OnEnter(MenuPage::Lessons), lessons::setup_lessons_menu)
            .add_systems(OnExit(MenuPage::Lessons), cleanup_menu)
            .add_systems(OnEnter(MenuPage::LessonReader), lessons::setup_lesson_reader)
            .add_systems(OnExit(MenuPage::LessonReader), cleanup_menu)
            .add_systems(
                Update,
                check_loading.run_if(in_state(AppState::SongLoading)),
            )
            // Tab switches on the Lessons page write `SelectedUnitIx` and
            // fire `LessonUnitChanged`; this swaps the scrollbox rows in
            // response (message-gated, not resource-change-gated — see the
            // doc comment on `LessonUnitChanged`).
            .add_systems(
                Update,
                lessons::repopulate_lesson_list.run_if(in_state(MenuPage::Lessons)),
            )
            // If a combobox dropdown was open, its own Escape handler closes
            // it and consumes the keypress — this handler never sees it, so
            // one Escape press doesn't both close a dropdown and navigate
            // back a page.
            .add_systems(
                Update,
                handle_menu_escape
                    .after(super::dialogs::combobox::close_open_comboboxes_on_escape)
                    .run_if(in_state(AppState::Menu)),
            );
    }
}

/// Escape navigates back one level in the menu hierarchy, mirroring each
/// page's own "Back" button target. `Main` has no parent, so it's a no-op
/// there.
fn handle_menu_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    page: Res<State<MenuPage>>,
    mut next_page: ResMut<NextState<MenuPage>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    let target = match page.get() {
        MenuPage::Main => return,
        MenuPage::Play | MenuPage::Options | MenuPage::Lessons => MenuPage::Main,
        MenuPage::ArtistList | MenuPage::ModeSelect => MenuPage::Play,
        MenuPage::SongList => MenuPage::ArtistList,
        MenuPage::Theme => MenuPage::Options,
        MenuPage::LessonReader => MenuPage::Lessons,
    };
    next_page.set(target);
}
// ── UI helpers ────────────────────────────────────────────────────────────────

fn menu_bg() -> Color {
    Color::srgb(0.05, 0.05, 0.08)
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

/// Spawn a single button as a child of `parent`.
///
/// When the theme JSON defines coords for this button in `menu_id`, the button
/// is absolutely positioned at those pixel coordinates. Otherwise it joins the
/// normal flex flow of the parent.
///
/// When the theme has shaders the button also gets a smoke background layer,
/// an optional icon, and audio on hover/click.
///
/// `on_click` is the button's own dedicated click behaviour, wired inline as the
/// `on(...)` callback (plain buttons) or via `observe` (themed buttons).
/// `coord_id` is the optional theme-JSON key used to look up fixed coordinates.
pub(super) fn spawn_button<M: 'static>(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    coord_id: Option<&str>,
    theme: &LoadedTheme,
    btn_mats: &ButtonMaterials,
    menu_id: &str,
    on_click: impl IntoObserverSystem<Pointer<Click>, (), M> + Clone + Sync + 'static,
) {
    // Resolve pixel coords from the theme JSON (if defined for this button).
    let coords = coord_id
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

    // Children are `Pickable::IGNORE` so the pointer always hits the button
    // itself (not the text/icon), keeping the hover/press observers below
    // robust — otherwise picking would target a child and the button would
    // flicker between hovered/unhovered.
    //
    // Themed buttons stay imperative (runtime shader-material handle, optional
    // icon, z-ordered smoke layer); plain buttons are authored with bsn!. Either
    // way the click rides along as the caller's dedicated `on_click`.
    if theme.has_shaders {
        let e = commands.spawn((Button, node, ThemedButton)).id();

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
                    ..default()
                },
                TextColor(Color::WHITE),
                Pickable::IGNORE,
            ));
        });

        // Themed hover/press visuals via observers, not a
        // `Changed<Interaction>` system.
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

        // The caller's dedicated click behaviour.
        commands.entity(e).observe(on_click);
        commands.entity(parent).add_child(e);
    } else {
        // Plain button: authored declaratively; click + hover ride along as
        // inline on(...)
        let e = commands
            .spawn_scene(button::default(label, on_click))
            .insert(node)
            .id();
        commands.entity(parent).add_child(e);
    }
}

// ── Menu pages ────────────────────────────────────────────────────────────────

fn setup_main_menu(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(&mut commands, &loc.msg("app-title"), None, &theme, "Main");
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-play"),
        Some("Play"),
        &theme,
        &btn_mats,
        "Main",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-lessons"),
        Some("Lessons"),
        &theme,
        &btn_mats,
        "Main",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::Lessons)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-song-editor-2"),
        Some("SongEditor2"),
        &theme,
        &btn_mats,
        "Main",
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::SongEditor2)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-options"),
        Some("Options"),
        &theme,
        &btn_mats,
        "Main",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Options),
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-credits"),
        Some("Credits"),
        &theme,
        &btn_mats,
        "Main",
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::Credits)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("menu-quit"),
        Some("Quit"),
        &theme,
        &btn_mats,
        "Main",
        |_: On<Pointer<Click>>, mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit::Success);
        },
    );
}

fn setup_play_menu(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(&mut commands, &loc.msg("menu-play"), None, &theme, "Play");
    // The render mode is chosen up front, before picking a song.
    spawn_button(
        &mut commands,
        root,
        &loc.msg("play-song"),
        Some("PlaySong"),
        &theme,
        &btn_mats,
        "Play",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::ModeSelect)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-session"),
        Some("JamSession"),
        &theme,
        &btn_mats,
        "Play",
        |_: On<Pointer<Click>>,
         mut mode: ResMut<GameplayMode>,
         mut page: ResMut<NextState<MenuPage>>| {
            *mode = GameplayMode::JamSession;
            page.set(MenuPage::ArtistList);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("bending-trainer"),
        Some("BendingTrainer"),
        &theme,
        &btn_mats,
        "Play",
        |_: On<Pointer<Click>>, mut state: ResMut<NextState<AppState>>| {
            state.set(AppState::BendingTrainer)
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        Some("BackToMain"),
        &theme,
        &btn_mats,
        "Play",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
    );
}

fn setup_artist_list(
    mut commands: Commands,

    songs: Res<AvailableSongs>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("select-artist"),
        None,
        &theme,
        "ArtistList",
    );

    if songs.0.is_empty() {
        let msg = commands
            .spawn((
                Text::new(loc.msg("no-songs-found")),
                TextFont {
                    font_size: FontSize::Px(16.0),
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
            let artist = artist.clone();
            spawn_button(
                &mut commands,
                root,
                &label,
                None,
                &theme,
                &btn_mats,
                "ArtistList",
                move |_: On<Pointer<Click>>,
                      mut selected: ResMut<SelectedArtist>,
                      mut page: ResMut<NextState<MenuPage>>| {
                    selected.0 = artist.clone();
                    page.set(MenuPage::SongList);
                },
            );
        }
    }
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        Some("BackToPlay"),
        &theme,
        &btn_mats,
        "ArtistList",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
}

fn setup_song_list(
    mut commands: Commands,
    songs: Res<AvailableSongs>,
    selected_artist: Res<SelectedArtist>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let subtitle = format!("by {}", selected_artist.0);
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("select-song"),
        Some(&subtitle),
        &theme,
        "SongList",
    );

    if let Some(artist_songs) = songs.0.get(&selected_artist.0) {
        let mut sorted = artist_songs.clone();
        sorted.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        for song in &sorted {
            let path = song.asset_path.clone();
            // The mode is already chosen — picking a song starts the game.
            spawn_button(
                &mut commands,
                root,
                &song.name,
                None,
                &theme,
                &btn_mats,
                "SongList",
                move |_: On<Pointer<Click>>,
                      asset_server: Res<AssetServer>,
                      mut state: ResMut<NextState<AppState>>,
                      mut commands: Commands| {
                    commands.insert_resource(SelectedSong(
                        asset_server.load::<SongManifest>(path.clone()),
                    ));
                    state.set(AppState::SongLoading);
                },
            );
        }
    }
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        Some("BackToArtistList"),
        &theme,
        &btn_mats,
        "SongList",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::ArtistList)
        },
    );
}

fn setup_mode_select(
    mut commands: Commands,

    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("select-mode"),
        None,
        &theme,
        "ModeSelect",
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("play-2d"),
        Some("PlayMode2D"),
        &theme,
        &btn_mats,
        "ModeSelect",
        |_: On<Pointer<Click>>,
         mut mode: ResMut<GameplayMode>,
         mut page: ResMut<NextState<MenuPage>>| {
            *mode = GameplayMode::Play2D;
            page.set(MenuPage::ArtistList);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("play-3d"),
        Some("PlayMode3D"),
        &theme,
        &btn_mats,
        "ModeSelect",
        |_: On<Pointer<Click>>,
         mut mode: ResMut<GameplayMode>,
         mut page: ResMut<NextState<MenuPage>>| {
            *mode = GameplayMode::Play3D;
            page.set(MenuPage::ArtistList);
        },
    );
    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        Some("BackToPlay"),
        &theme,
        &btn_mats,
        "ModeSelect",
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Play),
    );
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
/// A finished/quit *lesson* run returns to the lesson list instead, and its
/// [`LessonContext`] ends here — the menu is the boundary where a lesson run
/// stops being in flight (Results→Retry never passes through the menu, so
/// retries keep their context).
fn route_menu_entry(
    lesson: Option<Res<crate::lessons::LessonContext>>,
    mut ret_song: ResMut<ReturnToSongList>,
    mut ret_opts: ResMut<ReturnToOptions>,
    mut next_page: ResMut<NextState<MenuPage>>,
    mut commands: Commands,
) {
    if lesson.is_some() {
        commands.remove_resource::<crate::lessons::LessonContext>();
        // "Quit Song" sets this unconditionally; for a lesson run the lesson
        // list is the right place to land, so the flag is consumed here.
        ret_song.0 = false;
        next_page.set(MenuPage::Lessons);
    } else if ret_song.0 {
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
