// SPDX-License-Identifier: MIT

//! Per-stem mute row for a Jam Session backing
//! (`song::BackingStemAudio`): each stem plays as its own simultaneous,
//! synchronized `AudioSink` (spawned by `gameplay::countdown_overlay::
//! update_countdown`), so muting one is just zeroing that sink's volume —
//! no live re-mixing needed, since every stem is already a complete,
//! independent render (`harmonicon_core::midi_file::render_track_pcm`).

use bevy::audio::{AudioSink, Volume};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;

use harmonicon_app::app::GeneratedJamSession;
use harmonicon_audio::AudioSettings;
use harmonicon_gameplay::gameplay::{BackingStemPlayer, MusicPlayer};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::song::BackingStemAudio;
use harmonicon_ui::dialogs::tooltip::Tooltip;

use super::backing::COMPING_STEM;
use super::band::BandTracker;
use super::call_response::CallDuck;

/// Per-stem mute state for the current Jam Session backing — index
/// matches `SongManifest::backing_stems`. Sized (and reset to all-unmuted) by
/// `jam::session::setup` for every jam, whether or not the song actually
/// has backing stems (empty otherwise, so the systems below are cheap no-ops
/// for an ordinary song — nothing to iterate).
#[derive(Resource, Default)]
pub struct JamStemMute(pub Vec<bool>);

/// Tags one mute-toggle button with which track it controls, so one shared
/// `toggle_track_mute` observer (cloned onto every button) can look up
/// which track fired via the clicked entity — see `gameplay::
/// harmonica_overlay::DiagramCellTarget` for the same pattern.
#[derive(Component, Clone, Copy, Default)]
pub struct TrackMuteCell(usize);

/// The "with sound"/"no sound" icon inside one mute button — its text is
/// the only part [`update_stem_mute_buttons`] rewrites; the stem-name
/// label next to it is static, set once at spawn.
#[derive(Component, Clone, Copy, Default)]
pub struct TrackMuteIcon(usize);

const SOUND_ICON: &str = "\u{1F50A}"; // 🔊 — subsetted into fallback_emoji.ttf
// ⊘ rather than the matching 🔇: that codepoint is in none of the bundled
// fonts, so it drew as a box. Whoever subsetted the speaker glyph took the
// "on" state and missed the "off" one. The muted button also turns red
// (MUTED_BG), so the state reads clearly despite the mixed symbol families.
const MUTED_ICON: &str = "\u{2298}";
const MUTED_BG: Color = Color::srgb(0.30, 0.14, 0.14);
const UNMUTED_BG: Color = Color::srgb(0.16, 0.30, 0.18);

/// A horizontal row of per-stem mute-toggle buttons, one per
/// `SongManifest::backing_stems` entry — only call this when the current
/// song actually has backing stems (`jam::session::setup` checks first).
pub fn spawn_backing_stem_row(
    parent: &mut ChildSpawnerCommands,
    tracks: &[BackingStemAudio],
    loc: &Localization,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .with_children(|row| {
            for (index, track) in tracks.iter().enumerate() {
                row.spawn_empty().apply_scene(bsn! {
                    WidgetButton
                    TabIndex(0)
                    TrackMuteCell(index)
                    Node {
                        align_items: {AlignItems::Center},
                        padding: {UiRect::axes(Val::Px(10.0), Val::Px(6.0))},
                        border: {UiRect::all(Val::Px(1.0))},
                    }
                    BackgroundColor({UNMUTED_BG})
                    ~{BorderColor::all(Color::srgb(0.30, 0.30, 0.40))}
                    Tooltip({String::from(loc.msg("jam-midi-track-mute-tooltip"))})
                    on(toggle_track_mute)
                    Children [
                        Text({SOUND_ICON})
                        TextFont { font_size: {FontSize::Px(14.0)} }
                        TextColor({Color::WHITE})
                        TrackMuteIcon(index)
                        ~{Pickable::IGNORE}
                        --
                        Text({format!(" {}", track.name)})
                        TextFont { font_size: {FontSize::Px(14.0)} }
                        TextColor({Color::WHITE})
                        ~{Pickable::IGNORE}
                    ]
                });
            }
        });
}

/// Shared across every mute button; looks up which track fired via
/// `TrackMuteCell` on the clicked entity rather than a per-button closure.
fn toggle_track_mute(
    ev: On<Activate>,
    cells: Query<&TrackMuteCell>,
    mut mute: ResMut<JamStemMute>,
) {
    let Ok(cell) = cells.get(ev.entity) else {
        return;
    };
    if let Some(m) = mute.0.get_mut(cell.0) {
        *m = !*m;
    }
}

/// Keeps each mute button's icon/background in sync with `JamStemMute`.
pub fn update_stem_mute_buttons(
    mute: Res<JamStemMute>,
    mut icons: Query<(&TrackMuteIcon, &mut Text)>,
    mut cells: Query<(&TrackMuteCell, &mut BackgroundColor)>,
) {
    for (icon, mut text) in &mut icons {
        if let Some(&muted) = mute.0.get(icon.0) {
            let want = if muted { MUTED_ICON } else { SOUND_ICON };
            if text.0 != want {
                text.0.clear();
                text.0.push_str(want);
            }
        }
    }
    for (cell, mut bg) in &mut cells {
        if let Some(&muted) = mute.0.get(cell.0) {
            let want = if muted { MUTED_BG } else { UNMUTED_BG };
            if bg.0 != want {
                bg.0 = want;
            }
        }
    }
}

/// The gain one backing sink should sit at: the configured music volume,
/// dipped by the call-and-response duck and by the band's own shaping of
/// that stem (`band` is 1.0 for every stem it leaves alone), and silenced
/// when its stem is muted. A single-track song has no stem index and can
/// only be ducked.
fn backing_gain(music_volume: f32, duck: f32, band: f32, muted: bool) -> f32 {
    if muted { 0.0 } else { music_volume * duck * band }
}

/// Applies `JamStemMute`, `call_response::CallDuck` and the adaptive band's
/// comping gain (`band::BandTracker`, generated jams only — a MIDI-backed
/// song's third track is whatever the file says it is) to every backing
/// sink. Ordered `.after(gameplay::lifecycle::apply_music_volume)` so a
/// mid-song global-volume change (which touches every `MusicPlayer` sink,
/// stems included, since `BackingStemPlayer` entities carry that tag too)
/// can never un-mute a muted track, un-duck a call or un-thin the comping
/// — this system always has the last word.
pub fn apply_backing_gain(
    mute: Res<JamStemMute>,
    duck: Res<CallDuck>,
    band: Res<BandTracker>,
    generated: Option<Res<GeneratedJamSession>>,
    audio: Res<AudioSettings>,
    mut sinks: Query<(Option<&BackingStemPlayer>, &mut AudioSink), With<MusicPlayer>>,
) {
    for (stem, mut sink) in &mut sinks {
        let muted = stem.is_some_and(|s| mute.0.get(s.0).copied().unwrap_or(false));
        let shaped = generated.is_some() && stem.is_some_and(|s| s.0 == COMPING_STEM);
        sink.set_volume(Volume::Linear(backing_gain(
            audio.music_volume,
            duck.0,
            if shaped { band.comping_gain } else { 1.0 },
            muted,
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jam_stem_mute_defaults_to_empty() {
        assert!(JamStemMute::default().0.is_empty());
    }

    #[test]
    fn backing_gain_mutes_first_and_scales_otherwise() {
        assert_eq!(backing_gain(0.8, 1.0, 1.0, false), 0.8);
        assert_eq!(backing_gain(0.8, 0.5, 1.0, false), 0.4);
        assert_eq!(backing_gain(0.8, 1.0, 0.5, false), 0.4);
        assert_eq!(backing_gain(0.8, 0.5, 0.5, true), 0.0);
    }
}
