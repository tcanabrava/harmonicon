// SPDX-License-Identifier: MIT

//! Generated Jam Session setup: pick a key and tempo, then start an
//! endless synthesized 12-bar backing (`crate::jam::backing`) without first
//! picking an existing song — a second way into `GameplayMode::JamSession`
//! alongside the "Jam Session" button's real-song flow.

use bevy::audio::AudioSource;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_fluent::Localization;

use crate::audio_system::midi::{next_key, prev_key};
use crate::dialogs::button;
use crate::jam::backing::{GeneratedJamSession, build_generated_manifest};
use crate::localization::LocalizationExt;
use crate::song::SongManifest;
use crate::song::harmonica::Progression;
use crate::theme::LoadedTheme;

use crate::app::{AppState, GameplayMode, JamProgression, SelectedSong};
use crate::dialogs::button_material::ButtonMaterials;
use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_button, spawn_menu_root};

const MIN_BPM: f32 = 60.0;
const MAX_BPM: f32 = 160.0;
const BPM_STEP: f32 = 5.0;

/// The key/tempo currently selected on this page. Persists across visits
/// (like `bending_trainer::TrainerKey`/`TrainerTarget`), so re-opening the
/// page keeps your last choice instead of resetting to the default.
#[derive(Resource)]
pub(crate) struct JamGenerateConfig {
    pub key: String,
    pub bpm: f32,
    pub progression: Progression,
}

impl Default for JamGenerateConfig {
    fn default() -> Self {
        Self {
            key: "C".to_string(),
            bpm: 90.0,
            progression: Progression::Standard,
        }
    }
}

#[derive(Component)]
pub(crate) struct KeyLabel;
#[derive(Component)]
pub(crate) struct BpmLabel;
#[derive(Component)]
pub(crate) struct ProgressionLabel;

pub(crate) fn setup_jam_generate_menu(
    mut commands: Commands,
    config: Res<JamGenerateConfig>,
    theme: Res<LoadedTheme>,
    btn_mats: Res<ButtonMaterials>,
    loc: Res<Localization>,
) {
    let root = spawn_menu_root(
        &mut commands,
        &loc.msg("jam-generate-title"),
        None,
        &theme,
        "JamGenerate",
    );

    commands.entity(root).with_children(|root| {
        root.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn_empty().apply_scene(button::small(
                "\u{25C2}",
                |_: On<Pointer<Click>>, mut cfg: ResMut<JamGenerateConfig>| {
                    cfg.key = prev_key(&cfg.key);
                },
            ));
            row.spawn((
                Node {
                    width: Val::Px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Text::new(format!("Key: {}", config.key)),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.80, 0.35)),
                KeyLabel,
            ));
            row.spawn_empty().apply_scene(button::small(
                "\u{25B8}",
                |_: On<Pointer<Click>>, mut cfg: ResMut<JamGenerateConfig>| {
                    cfg.key = next_key(&cfg.key);
                },
            ));
        });

        root.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn_empty().apply_scene(button::small(
                "\u{25C2}",
                |_: On<Pointer<Click>>, mut cfg: ResMut<JamGenerateConfig>| {
                    cfg.bpm = (cfg.bpm - BPM_STEP).max(MIN_BPM);
                },
            ));
            row.spawn((
                Node {
                    width: Val::Px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Text::new(format!("Tempo: {:.0}", config.bpm)),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.80, 0.35)),
                BpmLabel,
            ));
            row.spawn_empty().apply_scene(button::small(
                "\u{25B8}",
                |_: On<Pointer<Click>>, mut cfg: ResMut<JamGenerateConfig>| {
                    cfg.bpm = (cfg.bpm + BPM_STEP).min(MAX_BPM);
                },
            ));
        });

        root.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn_empty().apply_scene(button::small(
                "\u{25C2}",
                |_: On<Pointer<Click>>, mut cfg: ResMut<JamGenerateConfig>| {
                    cfg.progression = cfg.progression.prev();
                },
            ));
            row.spawn((
                Node {
                    width: Val::Px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Text::new(format!("Progression: {}", config.progression.label())),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.80, 0.35)),
                ProgressionLabel,
            ));
            row.spawn_empty().apply_scene(button::small(
                "\u{25B8}",
                |_: On<Pointer<Click>>, mut cfg: ResMut<JamGenerateConfig>| {
                    cfg.progression = cfg.progression.next();
                },
            ));
        });
    });

    spawn_button(
        &mut commands,
        root,
        &loc.msg("jam-generate-start"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>,
         config: Res<JamGenerateConfig>,
         theme: Res<LoadedTheme>,
         mut manifests: ResMut<Assets<SongManifest>>,
         mut sources: ResMut<Assets<AudioSource>>,
         mut mode: ResMut<GameplayMode>,
         mut progression: ResMut<JamProgression>,
         mut commands: Commands,
         mut state: ResMut<NextState<AppState>>| {
            let background = theme.default_background.clone().unwrap_or_default();
            let manifest = build_generated_manifest(
                &config.key,
                config.bpm,
                config.progression,
                background,
                Handle::default(),
                &mut sources,
            );
            let handle = manifests.add(manifest);
            commands.insert_resource(SelectedSong(handle));
            commands.insert_resource(GeneratedJamSession);
            *mode = GameplayMode::JamSession;
            progression.0 = config.progression;
            // Synthesized synchronously above (no async asset load to wait
            // on), so this skips `AppState::SongLoading` entirely and goes
            // straight to `Playing` — `check_loading`'s only job is waiting
            // on `asset_server.is_loaded_with_dependencies`, which a
            // manifest built by `Assets::add` (not `AssetServer::load`)
            // never needs (and, per `GeneratedJamSession`'s doc comment,
            // never gets — `on_restart` skips `SongLoading` the same way).
            state.set(AppState::Playing);
        },
    );

    spawn_button(
        &mut commands,
        root,
        &loc.msg("back"),
        &theme,
        &btn_mats,
        |_: On<Pointer<Click>>, mut page: ResMut<NextState<MenuPage>>| {
            page.set(MenuPage::JamSessionMenu)
        },
    );
}

/// Keeps the "Key: ..." / "Tempo: ..." readouts in step with
/// [`JamGenerateConfig`], same pattern as `bending_trainer::update_key_label`.
pub(crate) fn update_jam_generate_labels(
    config: Res<JamGenerateConfig>,
    mut keys: Query<
        &mut Text,
        (
            With<KeyLabel>,
            Without<BpmLabel>,
            Without<ProgressionLabel>,
        ),
    >,
    mut bpms: Query<
        &mut Text,
        (
            With<BpmLabel>,
            Without<KeyLabel>,
            Without<ProgressionLabel>,
        ),
    >,
    mut progressions: Query<
        &mut Text,
        (
            With<ProgressionLabel>,
            Without<KeyLabel>,
            Without<BpmLabel>,
        ),
    >,
) {
    if !config.is_changed() {
        return;
    }
    for mut text in &mut keys {
        *text = Text::new(format!("Key: {}", config.key));
    }
    for mut text in &mut bpms {
        *text = Text::new(format!("Tempo: {:.0}", config.bpm));
    }
    for mut text in &mut progressions {
        *text = Text::new(format!("Progression: {}", config.progression.label()));
    }
}

// `next_key`/`prev_key` themselves are tested once, centrally, in
// `audio_system::midi` — see `next_key_cycles_forward_and_wraps` et al.
