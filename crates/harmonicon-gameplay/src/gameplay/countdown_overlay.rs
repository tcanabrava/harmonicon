// SPDX-License-Identifier: MIT

use bevy::{
    audio::{AudioSource, Volume},
    prelude::*,
};

use harmonicon_app::app::{AppState, GameplayMode, SelectedSong};
use harmonicon_audio::AudioSettings;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::song::SongManifest;

use super::{
    BackingStemPlayer, GameplayClock, GameplayLogic, GameplayRoot, MusicPlayer, MusicStarted,
    Paused, SongInfo, spawn_song_details,
};

#[derive(Component, Default, Clone)]
pub struct CountdownOverlay;

#[derive(Component, Default, Clone)]
pub struct CountdownText;

/// `song_info` is `None` for Jam Session, which has no chart to describe.
///
/// The countdown is one of the two moments the player is not playing, so it
/// is where the song's own details belong — key, harp, description, author
/// — rather than in a HUD column nobody reads mid-performance.
pub fn spawn_countdown(
    commands: &mut Commands,
    loc: &Localization,
    harp_hint: Option<&str>,
    song_info: Option<&SongInfo>,
) {
    // The shell and text rows use `bsn!`; optional harp and song details are
    // still chosen imperatively so absent content takes no layout space.
    let overlay = commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                width: {Val::Percent(100.0)},
                height: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
                justify_content: {JustifyContent::Center},
                row_gap: {Val::Px(12.0)},
            }
            BackgroundColor({Color::srgba(0.0, 0.0, 0.05, 0.55)})
            GlobalZIndex(100)
            CountdownOverlay
            GameplayRoot
        })
        .id();
    commands.entity(overlay).with_children(|ov| {
        ov.spawn_empty().apply_scene(bsn! {
            Text({String::from(loc.msg("gameplay-get-ready"))})
            TextFont { font_size: {FontSize::Px(22.0)} }
            TextColor({Color::srgba(0.85, 0.85, 1.0, 0.80)})
        });
        // Which physical harp to grab (2D/3D pass this; jam shows it elsewhere).
        if let Some(hint) = harp_hint {
            ov.spawn_empty().apply_scene(bsn! {
                Text({hint.to_string()})
                TextFont { font_size: {FontSize::Px(16.0)} }
                TextColor({Color::srgb(0.95, 0.80, 0.35)})
            });
        }
        if let Some(info) = song_info {
            spawn_song_details(ov, info);
        }
        ov.spawn_empty().apply_scene(bsn! {
            Text("3")
            TextFont { font_size: {FontSize::Px(120.0)} }
            TextColor({Color::WHITE})
            CountdownText
        });
    });
}

pub fn update_countdown(
    clock: Res<GameplayClock>,
    mut overlay: Query<&mut Visibility, With<CountdownOverlay>>,
    mut text: Query<(&mut Text, &mut TextFont), With<CountdownText>>,
    mut music_started: ResMut<MusicStarted>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    audio: Res<AudioSettings>,
    mode: Res<GameplayMode>,
    mut commands: Commands,
) {
    // The overlay's visibility is only written when it flips: rewriting the
    // same value re-runs visibility propagation over its whole subtree, and
    // this system runs every frame of the song.
    let wanted = if clock.get() >= 0.0 { Visibility::Hidden } else { Visibility::Visible };
    for mut vis in &mut overlay {
        if *vis != wanted {
            *vis = wanted;
        }
    }

    if clock.get() >= 0.0 {
        if !music_started.0 {
            music_started.0 = true;
            // `manifest.music` is `None` for a song with no `song/*.ogg`/
            // `*.wav` — play the chart silently rather than not starting at
            // all (the clock free-runs on frame delta instead of anchoring
            // to a sink; see `gameplay::should_anchor_to_sink`), *unless*
            // the song ships `midi_tracks` instead (see `song::
            // BackingStemAudio`), in which case every stem gets its own
            // sink, all spawned together this same frame so they start in
            // sync — muting one later is just zeroing that sink's volume
            // (`jam::midi_tracks::apply_backing_gain`), no re-mixing.
            if let Some(manifest) = manifests.get(&selected.0) {
                // Jam Session's own `restart_finished_jam_music` re-spawns
                // these entities once they despawn themselves, if Loop is on
                // at that moment — so they always start as plain one-shots
                // that self-clean; scored modes need the same self-cleaning
                // one-shot to move on to the results screen.
                let settings = if *mode == GameplayMode::JamSession {
                    PlaybackSettings::DESPAWN
                } else {
                    PlaybackSettings::ONCE
                };
                if let Some(music) = manifest.music.clone() {
                    commands.spawn((
                        AudioPlayer::<AudioSource>(music),
                        settings.with_volume(Volume::Linear(audio.music_volume)),
                        MusicPlayer,
                        GameplayRoot,
                    ));
                } else if let Some(tracks) = &manifest.backing_stems {
                    for (index, track) in tracks.iter().enumerate() {
                        commands.spawn((
                            AudioPlayer::<AudioSource>(track.source.clone()),
                            settings.with_volume(Volume::Linear(audio.music_volume)),
                            MusicPlayer,
                            BackingStemPlayer(index),
                            GameplayRoot,
                        ));
                    }
                }
            }
        }
        return;
    }

    let remaining = -clock.get();
    let n = remaining.ceil() as u32;
    let frac = remaining.fract() as f32;
    let font_size = 80.0 + (1.0 - frac) * 80.0;

    for (mut t, mut font) in &mut text {
        // The digit changes once a second; only the size animates per frame.
        if t.0.parse::<u32>().ok() != Some(n) {
            t.0 = n.to_string();
        }
        font.font_size = FontSize::Px(font_size);
    }
}

pub struct CountdownPlugin;

impl Plugin for CountdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_countdown
                .after(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(|p: Res<Paused>| !p.0)),
        );
    }
}
