// SPDX-License-Identifier: MIT

//! Persisted player settings (audio levels, chosen note theme).
//!
//! Read at startup with [`figment`] — defaults layered under the on-disk file so
//! a missing or partial file still works — and written back with `serde_json`
//! whenever a setting changes. The file lives in the user's config directory at
//! `<config>/harmonicon/settings.json`.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use figment::{
    Figment,
    providers::{Format, Json, Serialized},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::assets_management::{
    SelectedHarmonicaModel, SelectedNoteTheme2d, SelectedNoteTheme3d, SelectedTheme,
    ShowNoteNumbers,
};
use crate::audio_system::pitch_detect::PitchAlgorithm;

/// Player-tunable audio levels (0.0–1.0, linear), read by the audio spawners
/// (song music, metronome clicks) and edited on the Options page. Persisted by
/// this module; adjusting the music level updates the playing song in real time.
#[derive(Resource)]
pub struct AudioSettings {
    pub music_volume: f32,
    pub metronome_volume: f32,
    /// Milliseconds subtracted from the gameplay clock when judging whether
    /// a detected pitch was played in time. Compensates for the microphone
    /// input pipeline (FFT window ≈ 46 ms, OS buffer, cpal callback).
    /// Typical values: 50–100 ms for USB/built-in microphones.
    pub input_latency_ms: i32,
    /// Which algorithm the audio pipeline uses to detect played pitches.
    pub pitch_algorithm: PitchAlgorithm,
    /// Preferred microphone input device name; empty means "use the system
    /// default". Read by `audio_system::audio_input::start_capture`, which
    /// falls back to the default if this device isn't currently plugged in.
    pub input_device: String,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            music_volume: 0.8,
            metronome_volume: 0.7,
            input_latency_ms: 0,
            pitch_algorithm: PitchAlgorithm::default(),
            input_device: String::new(),
        }
    }
}

/// Whether adaptive difficulty (`gameplay::adaptive_difficulty`) gates note
/// visibility at all — a single global preference, not per-song (unlike the
/// "learned" progress it uses once on, which stays per-song in
/// `profile::SongRecord::phrase_learned`). Off by default: it's an opt-in
/// aid, not something a new player should discover mid-song via a pause
/// menu toggle they didn't know existed. Edited on the Options page;
/// `gameplay::adaptive_difficulty::setup_adaptive_difficulty` reads it when
/// seeding a song's live `AdaptiveDifficulty::enabled`, and the pause
/// menu's own toggle flips both this and that live resource together, for
/// an immediate mid-song effect that also persists as the new default.
#[derive(Resource, Default)]
pub struct AdaptiveDifficultyEnabled(pub bool);

/// Whether the game window runs borderless-fullscreen. Edited on the Options
/// page; `apply_fullscreen` mirrors this onto the primary window's
/// `WindowMode` whenever it changes (including once at startup, since the
/// Startup load marks it changed).
#[derive(Resource, Default)]
pub struct FullscreenEnabled(pub bool);

/// The on-disk shape of the settings. `#[serde(default)]` lets an older or
/// hand-edited file omit fields and still load.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
struct Settings {
    music_volume: f32,
    metronome_volume: f32,
    input_latency_ms: i32,
    note_theme_2d: String,
    note_theme_3d: String,
    harmonica_model: String,
    ui_theme: String,
    pitch_algorithm: PitchAlgorithm,
    input_device: String,
    show_note_numbers: bool,
    adaptive_difficulty_enabled: bool,
    fullscreen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_volume: 0.8,
            metronome_volume: 0.7,
            input_latency_ms: 0,
            note_theme_2d: "circular".into(),
            note_theme_3d: "circular".into(),
            harmonica_model: "default".into(),
            ui_theme: "default".into(),
            pitch_algorithm: PitchAlgorithm::default(),
            input_device: String::new(),
            show_note_numbers: false,
            adaptive_difficulty_enabled: false,
            fullscreen: false,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("harmonicon").join("settings.json"))
}

/// Defaults overlaid with whatever the file provides (missing file → defaults).
fn load_settings() -> Settings {
    let mut figment = Figment::from(Serialized::defaults(Settings::default()));
    if let Some(path) = settings_path() {
        figment = figment.merge(Json::file(path));
    }
    figment.extract().unwrap_or_else(|err| {
        warn!("Could not read settings ({err}); using defaults");
        Settings::default()
    })
}

fn save_settings(settings: &Settings) {
    let Some(path) = settings_path() else {
        warn!("No config directory available; settings not saved");
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        warn!("Could not create config dir {}: {err}", parent.display());
        return;
    }
    match serde_json::to_string_pretty(settings) {
        Ok(json) => {
            if let Err(err) = std::fs::write(&path, json) {
                warn!("Could not write settings to {}: {err}", path.display());
            }
        }
        Err(err) => warn!("Could not serialize settings: {err}"),
    }
}

/// How long to wait after the last settings change before actually writing
/// to disk. Without this, dragging a volume slider (which changes
/// `AudioSettings` every frame) would rewrite `settings.json` every frame
/// too; debouncing coalesces a burst of changes into one write.
const SAVE_DEBOUNCE_SECS: f32 = 0.5;

/// Seconds left before a pending settings change is written to disk.
/// `None` means nothing has changed since the last save.
#[derive(Resource, Default)]
struct PendingSave(Option<f32>);

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioSettings>()
            .init_resource::<AdaptiveDifficultyEnabled>()
            .init_resource::<FullscreenEnabled>()
            .init_resource::<PendingSave>()
            .add_systems(Startup, apply_loaded_settings)
            // Save whenever either settings resource changes. The Startup load
            // also marks them changed, so the file is created on first run
            // (after one debounce interval).
            .add_systems(
                Update,
                (
                    mark_settings_dirty.run_if(
                        |audio: Res<AudioSettings>,
                         theme_2d: Res<SelectedNoteTheme2d>,
                         theme_3d: Res<SelectedNoteTheme3d>,
                         model: Res<SelectedHarmonicaModel>,
                         ui_theme: Res<SelectedTheme>,
                         note_numbers: Res<ShowNoteNumbers>,
                         adaptive_difficulty: Res<AdaptiveDifficultyEnabled>,
                         fullscreen: Res<FullscreenEnabled>| {
                            audio.is_changed()
                                || theme_2d.is_changed()
                                || theme_3d.is_changed()
                                || model.is_changed()
                                || ui_theme.is_changed()
                                || note_numbers.is_changed()
                                || adaptive_difficulty.is_changed()
                                || fullscreen.is_changed()
                        },
                    ),
                    tick_pending_save,
                    apply_fullscreen,
                )
                    .chain(),
            )
            // Don't lose a change made just before quitting to the debounce
            // window — flush it immediately once exit is requested.
            .add_systems(Last, flush_pending_save_on_exit);
    }
}

/// Loads the saved settings into the live resources at startup. `pub` so
/// other Startup systems that need the loaded values (e.g. audio capture
/// reading `AudioSettings::input_device`) can order themselves `.after` it.
pub fn apply_loaded_settings(
    mut audio: ResMut<AudioSettings>,
    mut theme_2d: ResMut<SelectedNoteTheme2d>,
    mut theme_3d: ResMut<SelectedNoteTheme3d>,
    mut model: ResMut<SelectedHarmonicaModel>,
    mut ui_theme: ResMut<SelectedTheme>,
    mut note_numbers: ResMut<ShowNoteNumbers>,
    mut adaptive_difficulty: ResMut<AdaptiveDifficultyEnabled>,
    mut fullscreen: ResMut<FullscreenEnabled>,
) {
    let settings = load_settings();
    audio.music_volume = settings.music_volume;
    audio.metronome_volume = settings.metronome_volume;
    audio.input_latency_ms = settings.input_latency_ms;
    audio.pitch_algorithm = settings.pitch_algorithm;
    audio.input_device = settings.input_device;
    theme_2d.0 = settings.note_theme_2d;
    theme_3d.0 = settings.note_theme_3d;
    model.0 = settings.harmonica_model;
    ui_theme.0 = settings.ui_theme;
    note_numbers.0 = settings.show_note_numbers;
    adaptive_difficulty.0 = settings.adaptive_difficulty_enabled;
    fullscreen.0 = settings.fullscreen;
    info!(
        "Loaded settings: music={:.2} metronome={:.2} latency={}ms themes(2d={}, 3d={}) harmonica={} ui_theme={} note_numbers={} adaptive_difficulty={} fullscreen={}",
        audio.music_volume,
        audio.metronome_volume,
        audio.input_latency_ms,
        theme_2d.0,
        theme_3d.0,
        model.0,
        ui_theme.0,
        note_numbers.0,
        adaptive_difficulty.0,
        fullscreen.0,
    );
}

/// Writes the current resource values back to disk.
fn save_current(
    audio: &AudioSettings,
    theme_2d: &SelectedNoteTheme2d,
    theme_3d: &SelectedNoteTheme3d,
    model: &SelectedHarmonicaModel,
    ui_theme: &SelectedTheme,
    note_numbers: &ShowNoteNumbers,
    adaptive_difficulty: &AdaptiveDifficultyEnabled,
    fullscreen: &FullscreenEnabled,
) {
    save_settings(&Settings {
        music_volume: audio.music_volume,
        metronome_volume: audio.metronome_volume,
        input_latency_ms: audio.input_latency_ms,
        pitch_algorithm: audio.pitch_algorithm,
        input_device: audio.input_device.clone(),
        note_theme_2d: theme_2d.0.clone(),
        note_theme_3d: theme_3d.0.clone(),
        harmonica_model: model.0.clone(),
        ui_theme: ui_theme.0.clone(),
        show_note_numbers: note_numbers.0,
        adaptive_difficulty_enabled: adaptive_difficulty.0,
        fullscreen: fullscreen.0,
    });
}

/// (Re)starts the debounce countdown — called only when something actually
/// changed this frame, so a burst of changes (e.g. dragging a slider)
/// coalesces into one save `SAVE_DEBOUNCE_SECS` after the last of them.
fn mark_settings_dirty(mut pending: ResMut<PendingSave>) {
    pending.0 = Some(SAVE_DEBOUNCE_SECS);
}

/// Advances a pending-save countdown by `dt` seconds. Returns whether the
/// save should fire now, and the countdown's new state.
fn tick_debounce(remaining: Option<f32>, dt: f32) -> (bool, Option<f32>) {
    let Some(remaining) = remaining else {
        return (false, None);
    };
    let remaining = remaining - dt;
    if remaining > 0.0 {
        (false, Some(remaining))
    } else {
        (true, None)
    }
}

/// Ticks the debounce countdown; once it elapses, writes the current
/// resource values to disk exactly once.
fn tick_pending_save(
    time: Res<Time>,
    mut pending: ResMut<PendingSave>,
    audio: Res<AudioSettings>,
    theme_2d: Res<SelectedNoteTheme2d>,
    theme_3d: Res<SelectedNoteTheme3d>,
    model: Res<SelectedHarmonicaModel>,
    ui_theme: Res<SelectedTheme>,
    note_numbers: Res<ShowNoteNumbers>,
    adaptive_difficulty: Res<AdaptiveDifficultyEnabled>,
    fullscreen: Res<FullscreenEnabled>,
) {
    let (should_save, remaining) = tick_debounce(pending.0, time.delta_secs());
    pending.0 = remaining;
    if should_save {
        save_current(
            &audio,
            &theme_2d,
            &theme_3d,
            &model,
            &ui_theme,
            &note_numbers,
            &adaptive_difficulty,
            &fullscreen,
        );
    }
}

/// Flushes a pending save immediately when the app is exiting, so a change
/// made just before quitting isn't lost to the debounce window.
fn flush_pending_save_on_exit(
    mut exit: MessageReader<AppExit>,
    mut pending: ResMut<PendingSave>,
    audio: Res<AudioSettings>,
    theme_2d: Res<SelectedNoteTheme2d>,
    theme_3d: Res<SelectedNoteTheme3d>,
    model: Res<SelectedHarmonicaModel>,
    ui_theme: Res<SelectedTheme>,
    note_numbers: Res<ShowNoteNumbers>,
    adaptive_difficulty: Res<AdaptiveDifficultyEnabled>,
    fullscreen: Res<FullscreenEnabled>,
) {
    if exit.read().next().is_none() || pending.0.is_none() {
        return;
    }
    pending.0 = None;
    save_current(
        &audio,
        &theme_2d,
        &theme_3d,
        &model,
        &ui_theme,
        &note_numbers,
        &adaptive_difficulty,
        &fullscreen,
    );
}

/// Mirrors [`FullscreenEnabled`] onto the primary window's `WindowMode`.
/// Borderless (not exclusive) fullscreen, so toggling doesn't require a
/// video-mode change/re-negotiation with the display server.
fn apply_fullscreen(
    fullscreen: Res<FullscreenEnabled>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !fullscreen.is_changed() {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    window.mode = if fullscreen.0 {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AdaptiveDifficultyEnabled ────────────────────────────────────────────────

    #[test]
    fn adaptive_difficulty_is_off_by_default() {
        assert!(!AdaptiveDifficultyEnabled::default().0);
        assert!(!Settings::default().adaptive_difficulty_enabled);
    }

    #[test]
    fn missing_adaptive_difficulty_field_defaults_off_via_serde_default() {
        // An older settings.json predating this field should load with it
        // off, not silently on.
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(!s.adaptive_difficulty_enabled);
    }

    // ── FullscreenEnabled ────────────────────────────────────────────────────

    #[test]
    fn fullscreen_is_off_by_default() {
        assert!(!FullscreenEnabled::default().0);
        assert!(!Settings::default().fullscreen);
    }

    #[test]
    fn missing_fullscreen_field_defaults_off_via_serde_default() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(!s.fullscreen);
    }

    // ── tick_debounce ────────────────────────────────────────────────────────

    #[test]
    fn no_pending_save_stays_idle() {
        assert_eq!(tick_debounce(None, 0.1), (false, None));
    }

    #[test]
    fn counts_down_without_firing_before_it_elapses() {
        let (fired, remaining) = tick_debounce(Some(0.5), 0.1);
        assert!(!fired);
        assert!((remaining.unwrap() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn fires_once_the_countdown_elapses() {
        assert_eq!(tick_debounce(Some(0.05), 0.1), (true, None));
    }

    #[test]
    fn fires_exactly_at_zero() {
        assert_eq!(tick_debounce(Some(0.1), 0.1), (true, None));
    }
}
