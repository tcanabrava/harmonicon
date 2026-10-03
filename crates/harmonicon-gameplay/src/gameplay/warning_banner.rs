// SPDX-License-Identifier: MIT

//! Says so, during play, when something makes the game unable to score what
//! the player is doing.
//!
//! Every warning here shares one shape: notes scroll past, nothing scores,
//! and from the player's seat the game isn't broken — *they* are. Each was
//! reported nowhere, or only on the Options page, which is somewhere a
//! confused player has no reason to go.
//!
//! One banner rather than one per problem, because they'd occupy the same
//! strip of screen and overlap. [`warning_key`] picks by priority:
//!
//! 1. **The microphone isn't working.** Nothing can score. Dominates.
//! 2. **The chart has chords and the chosen detector is monophonic.** Those
//!    specific notes can never score (`scoring::chord_is_sounding` needs
//!    every pitch at once), while the rest of the song plays normally — so
//!    it reads as being inexplicably bad at a few notes.
//!
//! Spawned for every mode under `AppState::Playing`. Jam Session isn't
//! scored, but it still feeds back against what it hears, so a deaf one is
//! just as misleading; the chord warning simply never fires there, since
//! Jam Session builds no `SongNotes`.

use bevy::prelude::*;

use harmonicon_app::app::AppState;
use harmonicon_audio::AudioSettings;
use harmonicon_audio::audio_input::MicStatus;
use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::{GameplayRoot, SongNotes};

/// The warning's text node.
#[derive(Component, Default, Clone)]
pub struct WarningLabel;

/// The warning's outermost node. Visibility is toggled here rather than on
/// [`WarningLabel`], because the label sits inside a padded, coloured
/// panel — hiding just the text would leave an empty red box over the
/// highway.
#[derive(Component, Default, Clone)]
pub struct WarningRoot;

/// Spawns the (initially hidden) warning. Tagged `GameplayRoot` so it's torn
/// down with the rest of the scene.
///
/// Sits immediately *below* the song-progress bar, offset by that bar's own
/// [`song_progress_overlay::BAR_HEIGHT`] rather than a copied number, so the
/// two can't drift apart. It has to clear that bar rather than share the
/// top edge with it: the bar paints at `GlobalZIndex(250)` — above the pause
/// overlay, deliberately — so anything overlapping it is painted over,
/// which is exactly what a first attempt at `top: 8px` did (the warning
/// rendered *behind* the note-lane markers and was unreadable).
///
/// Its own z sits above the mode scenes (1) and their HUD panels (100) but
/// below the pause overlay (200), so pausing still covers it — a paused
/// player is already being told what to do.
///
/// A system registered on `OnEnter(AppState::Playing)` rather than a helper
/// called from `gameplay_2d`/`gameplay_3d`'s own setup, which is how
/// `spawn_wait_freeze_prompt` is wired: those two run only in their own
/// modes, and Jam Session needs this warning just as much.
pub fn setup_warning_banner(mut commands: Commands) {
    commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                top: {Val::Px(super::song_progress_overlay::BAR_HEIGHT + 8.0)},
                width: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
            }
            GlobalZIndex(150)
            GameplayRoot
            WarningRoot
            Children [
                Node {
                    padding: {UiRect::axes(Val::Px(16.0), Val::Px(8.0))},
                    border_radius: {BorderRadius::all(Val::Px(6.0))},
                }
                BackgroundColor({Color::srgba(0.35, 0.05, 0.05, 0.92)})
                Children [
                    Text({""})
                    TextFont { font_size: {FontSize::Px(20.0)} }
                    TextColor({Color::srgb(1.0, 0.85, 0.85)})
                    WarningLabel
                ]
            ]
        })
        .insert(Visibility::Hidden);
}

/// The localized message to show, or `None` when nothing is wrong.
pub fn warning_text(
    status: &MicStatus,
    chords_unhearable: bool,
    loc: &Localization,
) -> Option<String> {
    warning_key(status, chords_unhearable).map(|key| String::from(loc.msg(key)))
}

/// Which Fluent key to show, or `None` when nothing is wrong.
///
/// The pure half of [`warning_text`], split out so the priority order and
/// the choice of message stay testable — the text itself needs a
/// `Localization`, which a unit test has no cheap way to build.
pub fn warning_key(status: &MicStatus, chords_unhearable: bool) -> Option<&'static str> {
    match status {
        // A broken mic outranks everything: with nothing being heard at all,
        // telling the player about chords specifically would be misleading.
        MicStatus::AwaitingPermission => Some("mic-warning-permission"),
        MicStatus::Failed { .. } => Some("mic-warning-failed"),
        MicStatus::Connected { .. } if chords_unhearable => Some("chord-warning-monophonic"),
        MicStatus::Connected { .. } => None,
    }
}

/// Whether this song contains notes the current detector cannot possibly
/// resolve: a chord (or octave split) under a monophonic algorithm.
///
/// `chord_pitches` is non-empty exactly for notes belonging to a
/// multi-event item, which is the same set `chord_is_sounding` gates — so
/// this asks the question against the notes actually being scored rather
/// than re-reading the chart.
fn chords_are_unhearable(notes: &SongNotes, settings: &AudioSettings) -> bool {
    !settings.pitch_algorithm.is_polyphonic()
        && notes.notes.iter().any(|n| !n.chord_pitches.is_empty())
}

/// Shows or hides the warning as [`MicStatus`] changes.
///
/// The deliberately-omitted detail is the `Failed { reason }` string: it's a
/// raw cpal error ("Device not available", "The requested device is no
/// longer available"), untranslated and meaningless mid-song. The Options
/// page is where the specifics belong; this is a signpost pointing at it.
fn sync_warning_banner(
    status: Res<MicStatus>,
    settings: Res<AudioSettings>,
    notes: Option<Res<SongNotes>>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<WarningLabel>>,
    mut roots: Query<&mut Visibility, With<WarningRoot>>,
) {
    // `SongNotes` is inserted once per song, so its change tick covers the
    // "new song loaded" case that the other two don't.
    if !status.is_changed()
        && !settings.is_changed()
        && !notes.as_ref().is_some_and(|n| n.is_changed())
    {
        return;
    }
    let unhearable = notes.as_ref().is_some_and(|n| chords_are_unhearable(n, &settings));
    let message = warning_text(&status, unhearable, &loc);
    if let Some(message) = &message {
        for mut text in &mut labels {
            *text = Text::new(message.clone());
        }
    }
    for mut vis in &mut roots {
        *vis = if message.is_some() { Visibility::Visible } else { Visibility::Hidden };
    }
}

pub struct WarningBannerPlugin;

impl Plugin for WarningBannerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_warning_banner.run_if(in_state(AppState::Playing)));
    }
}

#[cfg(test)]
mod tests {
    use super::super::notes::ScheduledNote;
    use super::*;
    use harmonicon_audio::pitch_detect::PitchAlgorithm;

    /// One scheduled note carrying `chord_pitches`, wrapped in a `SongNotes`.
    /// Spelled out rather than `..Default::default()` because
    /// `ScheduledNote` has no `Default` and shouldn't grow one just for a
    /// test — the fields below are the whole struct.
    fn song_notes(chord_pitches: Vec<u8>) -> SongNotes {
        SongNotes {
            notes: vec![ScheduledNote {
                duration: 0.5,
                hole: 2,
                is_blow: false,
                expected_pitch: Some(62),
                chord_pitches,
                ..Default::default()
            }],
            cursor: 0,
        }
    }

    fn connected() -> MicStatus {
        MicStatus::Connected { device_name: "Some Mic".into() }
    }

    #[test]
    fn nothing_is_shown_when_everything_is_fine() {
        assert_eq!(warning_key(&connected(), false), None);
    }

    #[test]
    fn each_kind_of_trouble_gets_its_own_message() {
        // "grant a permission" and "your device is broken" are different
        // problems with different fixes; one generic message for both would
        // send an Android player hunting through Options for nothing.
        assert_ne!(
            warning_key(&MicStatus::AwaitingPermission, false),
            warning_key(&MicStatus::Failed { reason: "Device not available".into() }, false)
        );
    }

    #[test]
    fn the_in_play_warning_never_shows_the_raw_device_error() {
        // The reason is an untranslated cpal string, meaningless mid-song —
        // Options is where the specifics belong. Keying off `&'static str`
        // makes that structural rather than a habit: there is nowhere to
        // interpolate it.
        assert_eq!(
            warning_key(&MicStatus::Failed { reason: "Device not available".into() }, false),
            Some("mic-warning-failed")
        );
    }

    #[test]
    fn an_unhearable_chord_is_reported_when_the_mic_is_otherwise_fine() {
        assert_eq!(warning_key(&connected(), true), Some("chord-warning-monophonic"));
    }

    #[test]
    fn a_broken_mic_outranks_the_chord_warning() {
        // Both are true at once whenever a monophonic detector is selected
        // and the mic dies. Telling the player about chords then would be
        // actively misleading: nothing at all is being heard.
        assert_eq!(
            warning_key(&MicStatus::Failed { reason: "Device not available".into() }, true),
            Some("mic-warning-failed")
        );
    }

    #[test]
    fn only_a_monophonic_detector_makes_chords_unhearable() {
        // Guards the pairing that actually matters: the same chart is fine
        // under FFT/NMF and impossible under the other three.
        let chorded = song_notes(vec![62, 67]);
        for algo in PitchAlgorithm::all() {
            let settings = AudioSettings { pitch_algorithm: *algo, ..Default::default() };
            assert_eq!(
                chords_are_unhearable(&chorded, &settings),
                !algo.is_polyphonic(),
                "{} disagreed with its own is_polyphonic()",
                algo.label()
            );
        }
    }

    #[test]
    fn a_song_without_chords_never_warns_about_them() {
        let plain = song_notes(vec![]);
        let settings = AudioSettings { pitch_algorithm: PitchAlgorithm::Yin, ..Default::default() };
        assert!(!chords_are_unhearable(&plain, &settings));
    }
}
