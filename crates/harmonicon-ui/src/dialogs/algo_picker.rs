// SPDX-License-Identifier: MIT

//! Shared pitch-detection algorithm picker: a [`combobox`] whose tooltip
//! explains whichever algorithm is selected, instead of a separate static
//! panel. Used on the Options page; in the Bending Trainer, so a player can
//! quickly compare algorithms while actually bending notes; and in the Song
//! Editor's Record mode, since a take reads pitches off the same global
//! detector. All three drive the same global [`AudioSettings::
//! pitch_algorithm`] via [`on_algo_selected`], so picking one anywhere takes
//! effect everywhere immediately.

use bevy::prelude::*;

use crate::dialogs::combobox::ComboboxSelect;
use crate::dialogs::tooltip::Tooltip;
use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::PitchAlgorithm;
use harmonicon_platform::localization::{Localization, LocalizationExt};

/// Marks an entity (a pitch-algorithm combobox's root, from
/// [`attach_algo_tooltip`]) whose [`Tooltip`] should always describe
/// whichever algorithm [`AudioSettings::pitch_algorithm`] currently is —
/// kept in sync by [`update_algo_tooltip`].
#[derive(Component)]
pub struct AlgoTooltip;

/// One option's text: the bare label, plus a marker for detectors that
/// cannot resolve a chord.
///
/// The marker is not decoration. `scoring::chord_is_sounding` needs every
/// pitch of a chord at once, so choosing a monophonic detector silently
/// makes every chord in a chart unhittable — and worse, pYIN and MPM answer
/// a two-note chord with a phantom third pitch (see
/// `PitchAlgorithm::is_polyphonic`). Saying so in the picker is the cheapest
/// place to prevent that choice being made blind.
///
/// Built here rather than on `PitchAlgorithm` itself because the marker is
/// player-facing text and `harmonicon-dsp` is deliberately Bevy-free, with
/// no access to `Localization`.
fn algo_option_label(algo: PitchAlgorithm, loc: &Localization) -> String {
    if algo.is_polyphonic() {
        algo.label().to_string()
    } else {
        format!("{} — {}", algo.label(), loc.msg("algo-single-notes-only"))
    }
}

/// Every algorithm's option text, in [`PitchAlgorithm::all`]'s order — the
/// options list for a `dialogs::combobox`-based algorithm picker.
pub fn algo_labels(loc: &Localization) -> Vec<String> {
    PitchAlgorithm::all().iter().map(|a| algo_option_label(*a, loc)).collect()
}

/// A combobox `on_select` that writes straight to the shared global
/// [`AudioSettings::pitch_algorithm`] — picking an algorithm from either the
/// Options page's or the Bending Trainer's combobox takes effect everywhere
/// immediately.
///
/// Resolves by regenerating each option's text with the same function that
/// produced the list, rather than parsing the decoration back off: the
/// marker is localized, so `from_label` alone would stop matching the moment
/// the player isn't running in English. Unrecognized values are ignored
/// rather than silently resetting the setting.
pub fn on_algo_selected(
    ev: On<ComboboxSelect>,
    mut settings: ResMut<AudioSettings>,
    loc: Res<Localization>,
) {
    if let Some(algo) =
        PitchAlgorithm::all().iter().copied().find(|a| algo_option_label(*a, &loc) == ev.value)
    {
        settings.pitch_algorithm = algo;
    }
}

/// Attaches a [`Tooltip`] describing `selected` to a pitch-algorithm
/// combobox's root entity (the `Entity` returned by
/// `combobox::spawn_combobox`), kept current by [`update_algo_tooltip`].
pub fn attach_algo_tooltip(
    commands: &mut Commands,
    combobox_root: Entity,
    selected: PitchAlgorithm,
) {
    commands
        .entity(combobox_root)
        .insert((Tooltip(selected.description().to_string()), AlgoTooltip));
}

/// Keeps every [`AlgoTooltip`]'s text in step with the chosen algorithm.
pub fn update_algo_tooltip(
    settings: Res<AudioSettings>,
    mut tooltips: Query<&mut Tooltip, With<AlgoTooltip>>,
) {
    if !settings.is_changed() {
        return;
    }
    for mut tooltip in &mut tooltips {
        tooltip.0 = settings.pitch_algorithm.description().to_string();
    }
}

/// Runs [`update_algo_tooltip`] unconditionally: it only touches entities
/// carrying [`AlgoTooltip`], so it's a no-op on any screen that hasn't
/// spawned this widget.
pub struct AlgoPickerPlugin;

impl Plugin for AlgoPickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_algo_tooltip);
    }
}
