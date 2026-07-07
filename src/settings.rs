// SPDX-License-Identifier: MIT

//! Persisted player settings (audio levels, chosen note theme).
//!
//! Read at startup with [`figment`] — defaults layered under the on-disk file so
//! a missing or partial file still works — and written back with `serde_json`
//! whenever a setting changes. The file lives in the user's config directory at
//! `<config>/harmonicon/settings.json`.

use bevy::prelude::*;
use figment::{
    Figment,
    providers::{Format, Json, Serialized},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::assets_management::{
    SelectedHarmonicaModel, SelectedNoteTheme2d, SelectedNoteTheme3d, SelectedTheme,
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

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioSettings>()
            .add_systems(Startup, apply_loaded_settings)
            // Save whenever either settings resource changes. The Startup load
            // also marks them changed, so the file is created on first run.
            .add_systems(
                Update,
                persist_settings.run_if(
                    |audio: Res<AudioSettings>,
                     theme_2d: Res<SelectedNoteTheme2d>,
                     theme_3d: Res<SelectedNoteTheme3d>,
                     model: Res<SelectedHarmonicaModel>,
                     ui_theme: Res<SelectedTheme>| {
                        audio.is_changed()
                            || theme_2d.is_changed()
                            || theme_3d.is_changed()
                            || model.is_changed()
                            || ui_theme.is_changed()
                    },
                ),
            );
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
    info!(
        "Loaded settings: music={:.2} metronome={:.2} latency={}ms themes(2d={}, 3d={}) harmonica={} ui_theme={}",
        audio.music_volume,
        audio.metronome_volume,
        audio.input_latency_ms,
        theme_2d.0,
        theme_3d.0,
        model.0,
        ui_theme.0,
    );
}

/// Writes the current resource values back to disk.
fn persist_settings(
    audio: Res<AudioSettings>,
    theme_2d: Res<SelectedNoteTheme2d>,
    theme_3d: Res<SelectedNoteTheme3d>,
    model: Res<SelectedHarmonicaModel>,
    ui_theme: Res<SelectedTheme>,
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
    });
}
