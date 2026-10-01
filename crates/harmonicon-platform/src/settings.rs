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
use crate::content_packs::ContentSources;
use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::PitchAlgorithm;

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

/// Whether scored note visuals (the falling-note highway in Play2D/Play3D)
/// use the fixed colorblind-safe blow/draw pair
/// (`theme::COLORBLIND_NOTE_COLORS`) instead of the active theme's own note
/// colors. Off by default, same "opt-in, not sprung on a new player"
/// reasoning as [`AdaptiveDifficultyEnabled`]. Edited on the Options page;
/// `gameplay_2d`/`gameplay_3d`'s note spawners/tinters read it (via
/// `theme::effective_note_colors`) alongside `theme::LoadedTheme::
/// note_colors` every time they resolve a note's blow/draw color.
#[derive(Resource, Default)]
pub struct ColorblindPalette(pub bool);

/// Whether the highway's decorative motion is stilled: the head pop on a hit
/// (a miss's shrink becomes immediate — it's a state, not a flourish), the
/// flowing technique animations on note tails and the hold shimmer, and the
/// skill tree's collapse and slide
/// transitions (which jump to their end state). The
/// notes themselves still scroll: that *is* the game, and the hit line never
/// moves either way. Off by default; edited on the Options page and read by
/// `gameplay::note_feedback::judged_scale`, the tail animators and
/// `lesson_tree::transition::transition_step`. Exists
/// ahead of any camera shake or larger pulse, so those can be gated from
/// day one rather than retrofitted.
#[derive(Resource, Default)]
pub struct ReducedMotion(pub bool);

/// The Bending Trainer's precision controls — its "Advanced" drawer.
///
/// **One resource rather than a resource per knob**, unlike every setting
/// above it: these are one screen's worth of preferences, always read
/// together, and this module threads each persisted resource through five
/// places by hand (`apply_loaded_settings`, `mark_settings_dirty`,
/// `tick_pending_save`, `flush_pending_save_on_exit`, `save_current`) —
/// seven more would have crowded out the settings that genuinely are
/// independent, and pushed those systems toward Bevy's parameter limit.
///
/// Every field is a *preference*, persisted here rather than on the player
/// profile, which holds per-target performance evidence
/// (`profile::DrillRecord`) instead. The one field that is a measurement
/// rather than a taste — [`natural_center_cents`](Self::natural_center_cents)
/// — is here because it describes the *instrument*, not the player's
/// progress: it stays true across every session with that harp and means
/// nothing about how well anyone plays it.
///
/// Defaults reproduce the behaviour that used to be hardcoded in
/// `gameplay::bending_trainer`, so an existing player sees no change until
/// they open the drawer.
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BendingTrainerSettings {
    /// How close to the target counts as centred, in cents.
    pub tolerance_cents: f32,
    /// Seconds centred before the drill credits a controlled attempt.
    pub hold_secs: f32,
    /// Seconds before an unfinished drill attempt ends (as a miss if the
    /// player was audibly trying, a skip otherwise).
    pub timeout_secs: f32,
    /// Concert-pitch reference. A real harp is not always at 440.
    pub a4_hz: f32,
    /// Seconds of pitch history the bend rail draws behind the live marker.
    pub trace_secs: f32,
    /// How hard the *drawn* path is smoothed, 0 (raw) to 0.9. Display only:
    /// the stability residual is always computed from raw samples, or
    /// "steadiness" would become a function of a display knob.
    pub trace_smoothing: f32,
    /// Metronome pulses per beat that the Repeated practice shape asks a
    /// bend on. 1 = on the beat.
    pub subdivision: u8,
    /// Whether the Advanced drawer is open, remembered across visits.
    pub advanced_open: bool,
    /// Measured natural-reed centre in cents, keyed `"{key}:{hole}"` (e.g.
    /// `"C:2"`) — captured by the readiness check, subtracted from readings
    /// on that hole. Real reeds sit where they sit; an equal-tempered table
    /// is an orientation, not ground truth.
    pub natural_center_cents: std::collections::HashMap<String, f32>,
}

impl Default for BendingTrainerSettings {
    fn default() -> Self {
        Self {
            tolerance_cents: 6.0,
            hold_secs: 0.5,
            timeout_secs: 12.0,
            a4_hz: 440.0,
            trace_secs: 3.0,
            trace_smoothing: 0.0,
            subdivision: 1,
            advanced_open: false,
            natural_center_cents: std::collections::HashMap::new(),
        }
    }
}

impl BendingTrainerSettings {
    /// Inclusive bounds for each knob, as `(min, max)`. Public so the UI's
    /// steppers and [`clamped`](Self::clamped) cannot disagree about what
    /// is reachable.
    pub const TOLERANCE_CENTS: (f32, f32) = (1.0, 30.0);
    pub const HOLD_SECS: (f32, f32) = (0.1, 3.0);
    pub const TIMEOUT_SECS: (f32, f32) = (4.0, 60.0);
    /// Roughly A415 (baroque) to A466 — the range a real instrument or
    /// ensemble might actually ask for.
    pub const A4_HZ: (f32, f32) = (415.0, 466.0);
    pub const TRACE_SECS: (f32, f32) = (1.0, 10.0);
    pub const TRACE_SMOOTHING: (f32, f32) = (0.0, 0.9);
    pub const SUBDIVISION: (u8, u8) = (1, 4);

    /// The same values with every knob forced back inside its bounds.
    /// `settings.json` is a plain file a player can edit, and a
    /// `tolerance_cents` of `0` or a `trace_secs` of `-1` would otherwise
    /// reach the trainer's maths as a divide-by-zero or an empty window.
    #[must_use]
    pub fn clamped(mut self) -> Self {
        let clamp = |v: f32, (lo, hi): (f32, f32)| if v.is_finite() { v.clamp(lo, hi) } else { lo };
        self.tolerance_cents = clamp(self.tolerance_cents, Self::TOLERANCE_CENTS);
        self.hold_secs = clamp(self.hold_secs, Self::HOLD_SECS);
        self.timeout_secs = clamp(self.timeout_secs, Self::TIMEOUT_SECS);
        self.a4_hz = clamp(self.a4_hz, Self::A4_HZ);
        self.trace_secs = clamp(self.trace_secs, Self::TRACE_SECS);
        self.trace_smoothing = clamp(self.trace_smoothing, Self::TRACE_SMOOTHING);
        self.subdivision = self
            .subdivision
            .clamp(Self::SUBDIVISION.0, Self::SUBDIVISION.1);
        self
    }

    /// Key for [`natural_center_cents`](Self::natural_center_cents) — the
    /// harp key and hole, since a measured reed centre belongs to one
    /// physical instrument, not to a (hole, technique) skill the way
    /// `profile::DrillRecord` does.
    #[must_use]
    pub fn center_key(harp_key: &str, hole: u8) -> String {
        format!("{harp_key}:{hole}")
    }
}

/// How the Song Editor's action buttons (transport strip, mod panel,
/// timeline tools, ...) render their icon/label — see
/// `song_editor::panel_widgets::spawn_button_shell`, which is the one place
/// that reads this. `TextOnly` is the default so an existing player's UI
/// doesn't change until they opt in.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum ActionButtonStyle {
    IconOnly,
    TextBesideIcon,
    #[default]
    TextOnly,
}

impl ActionButtonStyle {
    /// All three styles, in the order the Options combobox lists them.
    pub fn all() -> &'static [ActionButtonStyle] {
        &[
            ActionButtonStyle::IconOnly,
            ActionButtonStyle::TextBesideIcon,
            ActionButtonStyle::TextOnly,
        ]
    }

    /// Locale key for this style's combobox label — a real UI phrase,
    /// unlike `PitchAlgorithm::label`'s bare acronyms, so this goes through
    /// `loc.msg` rather than being shown directly.
    pub fn loc_key(self) -> &'static str {
        match self {
            ActionButtonStyle::IconOnly => "options-button-style-icon-only",
            ActionButtonStyle::TextBesideIcon => "options-button-style-text-beside-icon",
            ActionButtonStyle::TextOnly => "options-button-style-text-only",
        }
    }

    /// Inverse of [`loc_key`](Self::loc_key) resolved against `loc` — for UI
    /// that deals in the combobox's plain selected string rather than the
    /// enum itself. `None` for anything that isn't one of [`Self::all`]'s
    /// current labels.
    pub fn from_localized_label(
        loc: &crate::localization::Localization,
        label: &str,
    ) -> Option<Self> {
        use crate::localization::LocalizationExt;
        Self::all()
            .iter()
            .copied()
            .find(|s| &*loc.msg(s.loc_key()) == label)
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
    show_note_numbers: bool,
    adaptive_difficulty_enabled: bool,
    fullscreen: bool,
    colorblind_palette: bool,
    reduced_motion: bool,
    action_button_style: ActionButtonStyle,
    bending_trainer: BendingTrainerSettings,
    content_sources: ContentSources,
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
            colorblind_palette: false,
            reduced_motion: false,
            action_button_style: ActionButtonStyle::default(),
            bending_trainer: BendingTrainerSettings::default(),
            content_sources: ContentSources::default(),
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    super::paths::config_file("settings.json")
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
        warn!("No writable config directory; settings not saved");
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
            if let Err(err) = harmonicon_core::config_file::write_atomic(&path, &json) {
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
            .init_resource::<ColorblindPalette>()
            .init_resource::<ReducedMotion>()
            .init_resource::<ActionButtonStyle>()
            .init_resource::<BendingTrainerSettings>()
            .init_resource::<ContentSources>()
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
                         fullscreen: Res<FullscreenEnabled>,
                         colorblind_palette: Res<ColorblindPalette>,
                         reduced_motion: Res<ReducedMotion>,
                         action_button_style: Res<ActionButtonStyle>,
                         bending_trainer: Res<BendingTrainerSettings>,
                         content_sources: Res<ContentSources>| {
                            audio.is_changed()
                                || theme_2d.is_changed()
                                || theme_3d.is_changed()
                                || model.is_changed()
                                || ui_theme.is_changed()
                                || note_numbers.is_changed()
                                || adaptive_difficulty.is_changed()
                                || fullscreen.is_changed()
                                || colorblind_palette.is_changed()
                                || reduced_motion.is_changed()
                                || action_button_style.is_changed()
                                || bending_trainer.is_changed()
                                || content_sources.is_changed()
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
    mut colorblind_palette: ResMut<ColorblindPalette>,
    mut reduced_motion: ResMut<ReducedMotion>,
    mut action_button_style: ResMut<ActionButtonStyle>,
    mut bending_trainer: ResMut<BendingTrainerSettings>,
    mut content_sources: ResMut<ContentSources>,
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
    colorblind_palette.0 = settings.colorblind_palette;
    reduced_motion.0 = settings.reduced_motion;
    *action_button_style = settings.action_button_style;
    // Clamped on the way in, not just on the way out: `settings.json` is a
    // plain file, and an out-of-range value would otherwise reach the
    // trainer's maths before the player ever opens the drawer.
    *bending_trainer = settings.bending_trainer.clamped();
    *content_sources = settings.content_sources;
    info!(
        "Loaded settings: music={:.2} metronome={:.2} latency={}ms themes(2d={}, 3d={}) harmonica={} ui_theme={} note_numbers={} adaptive_difficulty={} fullscreen={} colorblind_palette={} reduced_motion={} action_button_style={:?}",
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
        colorblind_palette.0,
        reduced_motion.0,
        *action_button_style,
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
    colorblind_palette: &ColorblindPalette,
    reduced_motion: &ReducedMotion,
    action_button_style: &ActionButtonStyle,
    bending_trainer: &BendingTrainerSettings,
    content_sources: &ContentSources,
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
        colorblind_palette: colorblind_palette.0,
        reduced_motion: reduced_motion.0,
        action_button_style: *action_button_style,
        bending_trainer: bending_trainer.clone(),
        content_sources: content_sources.clone(),
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
    colorblind_palette: Res<ColorblindPalette>,
    reduced_motion: Res<ReducedMotion>,
    action_button_style: Res<ActionButtonStyle>,
    bending_trainer: Res<BendingTrainerSettings>,
    content_sources: Res<ContentSources>,
) {
    // Nothing pending is the usual state; don't rewrite it every frame.
    if pending.0.is_none() {
        return;
    }
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
            &colorblind_palette,
            &reduced_motion,
            &action_button_style,
            &bending_trainer,
            &content_sources,
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
    colorblind_palette: Res<ColorblindPalette>,
    reduced_motion: Res<ReducedMotion>,
    action_button_style: Res<ActionButtonStyle>,
    bending_trainer: Res<BendingTrainerSettings>,
    content_sources: Res<ContentSources>,
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
        &colorblind_palette,
        &reduced_motion,
        &action_button_style,
        &bending_trainer,
        &content_sources,
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
    use crate::localization::{Localization, LocalizationExt};

    // ── BendingTrainerSettings ───────────────────────────────────────────

    #[test]
    fn trainer_defaults_reproduce_the_previously_hardcoded_behaviour() {
        let s = BendingTrainerSettings::default();
        assert_eq!(s.tolerance_cents, 6.0);
        assert_eq!(s.hold_secs, 0.5);
        assert_eq!(s.timeout_secs, 12.0);
        assert_eq!(s.a4_hz, 440.0);
        assert_eq!(s.trace_secs, 3.0);
        assert_eq!(s.trace_smoothing, 0.0, "smoothing is opt-in, not default");
        assert_eq!(s.subdivision, 1);
        assert!(!s.advanced_open);
        assert_eq!(s.clone().clamped(), s, "defaults must be inside bounds");
    }

    #[test]
    fn a_hand_edited_settings_file_cannot_push_a_knob_out_of_range() {
        let wild = BendingTrainerSettings {
            tolerance_cents: 0.0,
            hold_secs: -4.0,
            timeout_secs: 9_000.0,
            a4_hz: 12.0,
            trace_secs: f32::NAN,
            trace_smoothing: 5.0,
            subdivision: 250,
            ..default()
        }
        .clamped();
        assert_eq!(
            wild.tolerance_cents,
            BendingTrainerSettings::TOLERANCE_CENTS.0
        );
        assert_eq!(wild.hold_secs, BendingTrainerSettings::HOLD_SECS.0);
        assert_eq!(wild.timeout_secs, BendingTrainerSettings::TIMEOUT_SECS.1);
        assert_eq!(wild.a4_hz, BendingTrainerSettings::A4_HZ.0);
        // NaN can't be clamped into range — it takes the floor rather than
        // propagating into the trace window's arithmetic.
        assert_eq!(wild.trace_secs, BendingTrainerSettings::TRACE_SECS.0);
        assert_eq!(
            wild.trace_smoothing,
            BendingTrainerSettings::TRACE_SMOOTHING.1
        );
        assert_eq!(wild.subdivision, BendingTrainerSettings::SUBDIVISION.1);
    }

    #[test]
    fn an_older_settings_file_loads_with_trainer_defaults() {
        let older: Settings =
            serde_json::from_str(r#"{"music_volume":0.5,"input_latency_ms":20}"#).unwrap();
        assert_eq!(older.music_volume, 0.5);
        assert_eq!(older.input_latency_ms, 20);
        assert_eq!(older.bending_trainer, BendingTrainerSettings::default());
    }

    #[test]
    fn a_measured_reed_centre_is_keyed_by_harp_and_hole() {
        // Not by technique: one reed, whatever is asked of it.
        assert_eq!(BendingTrainerSettings::center_key("C", 2), "C:2");
        assert_eq!(BendingTrainerSettings::center_key("F#", 10), "F#:10");
    }

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

    // ── ContentSources ───────────────────────────────────────────────────────

    /// Through figment, as `load_settings` reads it: the file's list must
    /// *replace* the default one, or a removed official repository would
    /// come back on every start.
    fn layered(file: &str) -> Settings {
        Figment::from(Serialized::defaults(Settings::default()))
            .merge(Json::string(file))
            .extract()
            .unwrap()
    }

    #[test]
    fn a_settings_file_without_sources_gets_the_official_repositories() {
        assert_eq!(layered("{}").content_sources, ContentSources::default());
    }

    #[test]
    fn a_players_repository_list_replaces_the_defaults() {
        let s =
            layered(r#"{"content_sources":{"lessons":[],"songs":[{"path":"/home/me/songs"}]}}"#);
        assert!(s.content_sources.lessons.is_empty());
        assert_eq!(
            s.content_sources.songs,
            [harmonicon_packs::repo::RepoSpec::Local {
                path: "/home/me/songs".into()
            }]
        );
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

    // ── ColorblindPalette ────────────────────────────────────────────────────

    #[test]
    fn colorblind_palette_is_off_by_default() {
        assert!(!ColorblindPalette::default().0);
        assert!(!Settings::default().colorblind_palette);
    }

    #[test]
    fn missing_colorblind_palette_field_defaults_off_via_serde_default() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(!s.colorblind_palette);
    }

    // ── ReducedMotion ────────────────────────────────────────────────────────

    #[test]
    fn reduced_motion_is_off_by_default_and_when_the_field_is_missing() {
        assert!(!ReducedMotion::default().0);
        assert!(!Settings::default().reduced_motion);
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(!s.reduced_motion);
    }

    // ── ActionButtonStyle ────────────────────────────────────────────────────

    #[test]
    fn action_button_style_defaults_to_text_only() {
        assert_eq!(ActionButtonStyle::default(), ActionButtonStyle::TextOnly);
        assert_eq!(
            Settings::default().action_button_style,
            ActionButtonStyle::TextOnly
        );
    }

    #[test]
    fn missing_action_button_style_field_defaults_via_serde_default() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.action_button_style, ActionButtonStyle::TextOnly);
    }

    #[test]
    fn from_localized_label_round_trips_every_style() {
        let loc = Localization::default();
        for style in ActionButtonStyle::all() {
            // `Localization::default()` has no bundle loaded, so `loc.msg`
            // falls back to the key itself — this only exercises the
            // round trip through whatever `loc.msg` actually returns, not
            // real translated text.
            let label = loc.msg(style.loc_key());
            assert_eq!(
                ActionButtonStyle::from_localized_label(&loc, &label),
                Some(*style)
            );
        }
    }

    #[test]
    fn from_localized_label_rejects_unknown_text() {
        let loc = Localization::default();
        assert_eq!(
            ActionButtonStyle::from_localized_label(&loc, "not a real label"),
            None
        );
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
