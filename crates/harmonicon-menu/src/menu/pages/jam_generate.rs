// SPDX-License-Identifier: MIT

//! Generated Jam Session setup: pick a key and tempo, then start an
//! looping synthesized 12-bar backing (`harmonicon_jam::jam::backing`) without first
//! picking an existing song — a second way into `GameplayMode::JamSession`
//! alongside the "Jam Session" button's real-song flow.

use bevy::audio::AudioSource;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};
use bevy::ui_widgets::Activate;

use harmonicon_app::app::{GeneratedJamSession, GeneratedSong};
use harmonicon_core::chart::Scale;
use harmonicon_core::harmonica::{Position, Progression};
use harmonicon_core::midi::NOTE_NAMES;
use harmonicon_jam::jam::backing::{
    BandEnergy, Genre, JamGenre, RenderedBacking, assemble_generated_manifest,
    render_generated_backing,
};
use harmonicon_platform::localization::{Localization, LocalizationExt, enum_label_key};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_song::song::SongManifest;
use harmonicon_ui::dialogs::combobox;
use harmonicon_ui::dialogs::text_input::{NumericInputCommitted, spawn_numeric_input};

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_button, spawn_menu_root_plain};
use harmonicon_app::app::{AppState, GameplayMode, JamProgression, JamScale, SelectedSong};

const MIN_BPM: f32 = 60.0;
const MAX_BPM: f32 = 160.0;

/// The key/tempo currently selected on this page. Persists across visits
/// (like `bending_trainer::TrainerKey`/`TrainerTarget`), so re-opening the
/// page keeps your last choice instead of resetting to the default.
#[derive(Resource, Clone)]
pub(crate) struct JamGenerateConfig {
    pub key: String,
    pub bpm: f32,
    pub progression: Progression,
    pub position: Position,
    pub scale: Scale,
    pub genre: Genre,
    pub energy: BandEnergy,
}

impl Default for JamGenerateConfig {
    fn default() -> Self {
        Self {
            key: "C".to_string(),
            bpm: 90.0,
            progression: Progression::Standard,
            position: Position::First,
            scale: Scale::FirstPosition,
            genre: Genre::Blues,
            energy: BandEnergy::Medium,
        }
    }
}

fn key_labels() -> Vec<String> {
    NOTE_NAMES.iter().map(|s| s.to_string()).collect()
}

// Each enum's English `label()` is its stable id; what the combobox shows
// is the locale's wording for it, and a selection maps back by *index*
// into the same `all()` order (`ComboboxSelect::index`), never by text.

/// Locale key of one progression's label.
pub(crate) fn progression_key(p: Progression) -> String {
    enum_label_key("progression", p.label())
}

/// Locale key of one position's label.
pub(crate) fn position_key(p: Position) -> String {
    enum_label_key("position", p.label())
}

/// Locale key of one scale's label.
pub(crate) fn scale_key(s: Scale) -> String {
    enum_label_key("scale", s.label())
}

/// Locale key of one genre's label.
pub(crate) fn genre_key(g: Genre) -> String {
    enum_label_key("genre", g.label())
}

pub(crate) fn energy_key(energy: BandEnergy) -> String {
    enum_label_key("band-energy", energy.label())
}

fn progression_labels(loc: &Localization) -> Vec<String> {
    Progression::all().iter().map(|p| loc.msg(&progression_key(*p)).into()).collect()
}

fn position_labels(loc: &Localization) -> Vec<String> {
    Position::all().iter().map(|p| loc.msg(&position_key(*p)).into()).collect()
}

fn scale_labels(loc: &Localization) -> Vec<String> {
    Scale::all().iter().map(|s| loc.msg(&scale_key(*s)).into()).collect()
}

fn genre_labels(loc: &Localization) -> Vec<String> {
    Genre::all().iter().map(|g| loc.msg(&genre_key(*g)).into()).collect()
}

fn energy_labels(loc: &Localization) -> Vec<String> {
    BandEnergy::all().iter().map(|energy| loc.msg(&energy_key(*energy)).into()).collect()
}

pub(crate) fn setup_jam_generate_menu(
    mut commands: Commands,
    config: Res<JamGenerateConfig>,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
) {
    // `_plain`, not `spawn_menu_root`: this page's six comboboxes + tempo
    // field + button will never realistically overflow a window, but a
    // combobox's open dropdown is a literal ECS child of its toggle and
    // gets clipped to a `ScrollArea` ancestor's own (content-sized, not
    // full-window) bounds — see `spawn_menu_root`'s own doc comment. The
    // Genre combobox (the row closest to the clip edge) hit exactly that.
    let (root, header, page_root) = spawn_menu_root_plain(
        &mut commands,
        &loc.msg("jam-generate-title"),
        None,
        &theme,
        "JamGenerate",
    );

    combobox::spawn_combobox(
        &mut commands,
        root,
        page_root,
        &loc.msg("jam-generate-key"),
        &key_labels(),
        &config.key,
        |ev: On<combobox::ComboboxSelect>, mut cfg: ResMut<JamGenerateConfig>| {
            cfg.key = ev.value.clone();
        },
    );

    combobox::spawn_combobox(
        &mut commands,
        root,
        page_root,
        &loc.msg("jam-generate-progression"),
        &progression_labels(&loc),
        &loc.msg(&progression_key(config.progression)),
        |ev: On<combobox::ComboboxSelect>, mut cfg: ResMut<JamGenerateConfig>| {
            if let Some(p) = Progression::all().get(ev.index) {
                cfg.progression = *p;
            }
        },
    );

    combobox::spawn_combobox(
        &mut commands,
        root,
        page_root,
        &loc.msg("jam-generate-energy"),
        &energy_labels(&loc),
        &loc.msg(&energy_key(config.energy)),
        |ev: On<combobox::ComboboxSelect>, mut cfg: ResMut<JamGenerateConfig>| {
            if let Some(energy) = BandEnergy::all().get(ev.index) {
                cfg.energy = *energy;
            }
        },
    );

    combobox::spawn_combobox(
        &mut commands,
        root,
        page_root,
        &loc.msg("jam-generate-position"),
        &position_labels(&loc),
        &loc.msg(&position_key(config.position)),
        |ev: On<combobox::ComboboxSelect>, mut cfg: ResMut<JamGenerateConfig>| {
            if let Some(p) = Position::all().get(ev.index) {
                cfg.position = *p;
            }
        },
    );

    combobox::spawn_combobox(
        &mut commands,
        root,
        page_root,
        &loc.msg("jam-generate-scale"),
        &scale_labels(&loc),
        &loc.msg(&scale_key(config.scale)),
        |ev: On<combobox::ComboboxSelect>, mut cfg: ResMut<JamGenerateConfig>| {
            if let Some(s) = Scale::all().get(ev.index) {
                cfg.scale = *s;
            }
        },
    );

    combobox::spawn_combobox(
        &mut commands,
        root,
        page_root,
        &loc.msg("jam-generate-genre"),
        &genre_labels(&loc),
        &loc.msg(&genre_key(config.genre)),
        |ev: On<combobox::ComboboxSelect>, mut cfg: ResMut<JamGenerateConfig>| {
            if let Some(g) = Genre::all().get(ev.index) {
                cfg.genre = *g;
            }
        },
    );

    // ── Tempo: a numeric text box rather than a combobox — a free-form BPM
    // isn't a short pick-one-of-N choice, it's a number, so it gets its own
    // widget (`dialogs::text_input::spawn_numeric_input`) instead of forcing
    // it into the same "pick from a list" shape as Key/Progression/Position.
    let tempo_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(tempo_row);
    commands.entity(tempo_row).with_children(|row| {
        row.spawn_empty().apply_scene(bsn! {
            Text({String::from(loc.msg("jam-generate-tempo"))})
            TextFont { font_size: {FontSize::Px(20.0)} }
            TextColor({Color::WHITE})
        });
    });
    spawn_numeric_input(
        &mut commands,
        tempo_row,
        config.bpm,
        MIN_BPM,
        MAX_BPM,
        Color::srgb(0.10, 0.10, 0.16),
        Color::srgb(0.35, 0.35, 0.45),
        |ev: On<NumericInputCommitted>, mut cfg: ResMut<JamGenerateConfig>| {
            cfg.bpm = ev.value;
        },
    );

    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-generate-start"),
        |_: On<Activate>,
         config: Res<JamGenerateConfig>,
         pending: Option<Res<PendingJam>>,
         mut label: Query<&mut Visibility, With<PreparingLabel>>,
         mut commands: Commands| {
            if pending.is_some() {
                return;
            }
            let config = config.clone();
            let seed = rand::random();
            let (key, bpm, progression, genre, energy) =
                (config.key.clone(), config.bpm, config.progression, config.genre, config.energy);
            let task = AsyncComputeTaskPool::get().spawn(async move {
                render_generated_backing(&key, bpm, progression, genre, energy, seed)
            });
            commands.insert_resource(PendingJam { task, config, seed });
            for mut visibility in &mut label {
                *visibility = Visibility::Inherited;
            }
        },
    );

    commands.entity(root).with_children(|page| {
        page.spawn_empty()
            .apply_scene(bsn! {
                Text({String::from(loc.msg("jam-generate-preparing"))})
                TextFont { font_size: {FontSize::Px(20.0)} }
                TextColor({Color::WHITE})
                ~{Visibility::Hidden}
            })
            .insert(PreparingLabel);
    });

    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::JamSessionMenu),
    );
}

/// A Start press whose backing is still rendering on a worker thread. The
/// render takes 100–250 ms, long enough to freeze the menu if it ran inside
/// the click. Holds the settings as they were at the press, so changing a
/// combobox meanwhile can't mix two configurations.
#[derive(Resource)]
pub(crate) struct PendingJam {
    task: Task<RenderedBacking>,
    config: JamGenerateConfig,
    seed: u64,
}

/// "Getting the band ready…", shown while a [`PendingJam`] renders.
#[derive(Component)]
pub(crate) struct PreparingLabel;

/// Enters Jam Session once the backing render lands.
pub(crate) fn finish_pending_jam(
    mut pending: Option<ResMut<PendingJam>>,
    theme: Res<LoadedTheme>,
    mut manifests: ResMut<Assets<SongManifest>>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut mode: ResMut<GameplayMode>,
    mut progression: ResMut<JamProgression>,
    mut scale: ResMut<JamScale>,
    mut genre_res: ResMut<JamGenre>,
    mut commands: Commands,
    mut state: ResMut<NextState<AppState>>,
) {
    let Some(rendered) =
        pending.as_mut().and_then(|p| future::block_on(future::poll_once(&mut p.task)))
    else {
        return;
    };
    let Some(PendingJam { config, seed, .. }) = pending.as_deref() else {
        return;
    };
    let (config, seed) = (config.clone(), *seed);
    commands.remove_resource::<PendingJam>();

    let background = theme.default_background.clone().unwrap_or_default();
    let manifest = assemble_generated_manifest(
        rendered,
        &config.key,
        config.bpm,
        config.progression,
        config.position,
        config.genre,
        background,
        Handle::default(),
        &mut sources,
    );
    let handle = manifests.add(manifest);
    commands.insert_resource(SelectedSong(handle));
    // Both: `GeneratedSong` says the handle came from `Assets::add` and has
    // no `LoadState`; `GeneratedJamSession` says it is a jam and picks the
    // page to return to.
    commands.insert_resource(GeneratedSong);
    commands.insert_resource(GeneratedJamSession { seed });
    *mode = GameplayMode::JamSession;
    progression.0 = config.progression;
    scale.0 = config.scale;
    genre_res.0 = config.genre;
    // The manifest was built here, not loaded, so this skips
    // `AppState::SongLoading` and goes straight to `Playing`:
    // `check_loading` only waits on `asset_server.is_loaded_with_
    // dependencies`, which a manifest built by `Assets::add` never needs
    // (and, per `GeneratedJamSession`'s doc comment, never gets —
    // `on_restart` skips `SongLoading` the same way).
    state.set(AppState::Playing);
}

/// Leaving the page drops a render still in flight, which cancels it.
pub(crate) fn cancel_pending_jam(mut commands: Commands) {
    commands.remove_resource::<PendingJam>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;

    /// The resources Start and its render hand-off touch.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_state::<AppState>()
            .add_sub_state::<MenuPage>()
            .init_asset::<AudioSource>()
            .init_asset::<SongManifest>()
            .init_resource::<LoadedTheme>()
            .init_resource::<GameplayMode>()
            .init_resource::<JamProgression>()
            .init_resource::<JamScale>()
            .init_resource::<JamGenre>();
        app
    }

    fn pending(config: JamGenerateConfig, seed: u64) -> PendingJam {
        let (key, bpm, progression, genre, energy) =
            (config.key.clone(), config.bpm, config.progression, config.genre, config.energy);
        PendingJam {
            task: AsyncComputeTaskPool::get().spawn(async move {
                render_generated_backing(&key, bpm, progression, genre, energy, seed)
            }),
            config,
            seed,
        }
    }

    #[test]
    fn a_finished_render_enters_the_jam_with_the_pressed_settings() {
        let mut app = app();
        app.add_systems(Update, finish_pending_jam);
        let config =
            JamGenerateConfig { bpm: 160.0, genre: Genre::Rock, ..JamGenerateConfig::default() };
        app.insert_resource(pending(config, 7));

        for _ in 0..300 {
            app.update();
            if !app.world().contains_resource::<PendingJam>() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        app.update();

        let world = app.world();
        assert!(!world.contains_resource::<PendingJam>(), "render never landed");
        assert_eq!(*world.resource::<GameplayMode>(), GameplayMode::JamSession);
        assert_eq!(world.resource::<GeneratedJamSession>().seed, 7);
        assert_eq!(world.resource::<JamGenre>().0, Genre::Rock);
        let song = &world.resource::<SelectedSong>().0;
        let manifest = world.resource::<Assets<SongManifest>>().get(song).unwrap();
        assert_eq!(manifest.backing_stems.as_ref().map(Vec::len), Some(3));
        assert_eq!(*world.resource::<State<AppState>>(), AppState::Playing);
    }

    #[test]
    fn leaving_the_page_cancels_a_render_in_flight() {
        let mut app = app();
        app.add_systems(OnExit(MenuPage::JamGenerate), cancel_pending_jam);
        app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::Menu);
        app.update();
        app.world_mut().resource_mut::<NextState<MenuPage>>().set(MenuPage::JamGenerate);
        app.update();
        app.insert_resource(pending(JamGenerateConfig::default(), 1));

        app.world_mut().resource_mut::<NextState<MenuPage>>().set(MenuPage::JamSessionMenu);
        app.update();

        assert!(!app.world().contains_resource::<PendingJam>());
    }

    /// Every key the comboboxes will ask for, so a variant added to one of
    /// the enums without a locale line fails here rather than showing its
    /// raw key on the Generate Jam page. `locales_define_the_same_keys`
    /// (harmonicon-platform) then guarantees the other locales have it too.
    #[test]
    fn every_enum_variant_has_a_locale_key() {
        let ftl = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/locales/en-US/main/ui.ftl"),
        )
        .unwrap();
        let has = |key: &str| ftl.lines().any(|l| l.starts_with(&format!("{key} =")));
        let mut missing = Vec::new();
        for p in Progression::all() {
            let k = progression_key(*p);
            if !has(&k) {
                missing.push(k);
            }
        }
        for p in Position::all() {
            let k = position_key(*p);
            if !has(&k) {
                missing.push(k);
            }
        }
        for s in Scale::all() {
            let k = scale_key(*s);
            if !has(&k) {
                missing.push(k);
            }
        }
        for g in Genre::all() {
            let k = genre_key(*g);
            if !has(&k) {
                missing.push(k);
            }
        }
        for energy in BandEnergy::all() {
            let k = energy_key(*energy);
            if !has(&k) {
                missing.push(k);
            }
        }
        assert!(missing.is_empty(), "missing locale keys: {missing:?}");
    }
}
