// SPDX-License-Identifier: MIT

//! Wires gameplay's own note/clock state into the shared `music_score`
//! overlay — Play 2D and Play 3D only (Jam Session has no [`SongNotes`] to
//! draw from, same reason it's excluded from scoring). Kept as its own
//! small plugin, separate from `notes.rs` (pure data/logic, no systems) and
//! `song_progress_overlay.rs` (a different overlay), since this is purely
//! translation: `SongNotes`/`GameplayClock` (gameplay's own vocabulary) into
//! [`MusicScoreNotes`]/[`MusicScorePlayhead`] (the shared plugin's).

use bevy::prelude::*;

use harmonicon_app::app::{AppState, GameplayMode, SelectedSong};
use harmonicon_core::chart::HarpChart;
use harmonicon_core::chart::seconds_to_tick;
use harmonicon_song::song::SongManifest;
use harmonicon_ui::music_score::{
    MeterMap, MusicScoreBarMap, MusicScoreMeter, MusicScoreNotes, MusicScorePlayhead,
};

use super::bars::pickup_lead_ticks;
use super::{GameplayClock, GameplayLogic, SongNotes, chart_meter, notes_to_notation};

pub struct MusicScoreBridgePlugin;

impl Plugin for MusicScoreBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                sync_music_score_notes.run_if(resource_changed::<SongNotes>),
                update_music_score_playhead.after(GameplayLogic),
            )
                .run_if(in_state(AppState::Playing).and_then(playing_2d_or_3d)),
        );
    }
}

fn playing_2d_or_3d(mode: Res<GameplayMode>) -> bool {
    matches!(*mode, GameplayMode::Play2D | GameplayMode::Play3D)
}

/// The loader has already expanded repeats into performance order. Build
/// the staff's bar grid from that same chart, including copied meter changes.
fn bar_map_for_chart(chart: &HarpChart) -> MusicScoreBarMap {
    let timing = &chart.timing;
    let meter = MeterMap::with_pickup(
        std::iter::once((0, chart.song.time_signature.as_deref().unwrap_or("4/4"))).chain(
            timing
                .time_signature_map
                .iter()
                .flatten()
                .map(|p| (p.tick, p.time_signature.as_str())),
        ),
        timing.resolution,
        timing.pickup(),
    );
    MusicScoreBarMap {
        meter,
        quarter_ticks: timing.resolution,
    }
}

/// Rebuilds [`MusicScoreNotes`] whenever [`SongNotes`] changes — song setup,
/// and any adaptive-difficulty resync mid-song (`gameplay_2d`/`gameplay_3d`'s
/// `resync_notes_on_adaptive_change`), so the staff stays in step with
/// whichever notes are actually live.
fn sync_music_score_notes(
    song_notes: Res<SongNotes>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut score_notes: ResMut<MusicScoreNotes>,
    mut meter: ResMut<MusicScoreMeter>,
    mut bar_map: ResMut<MusicScoreBarMap>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let timing = &manifest.chart.timing;
    // The one reading of a chart's meter, so the staff and the bar counter
    // cannot disagree.
    let parsed = chart_meter(&manifest.chart);
    if *meter != parsed {
        *meter = parsed;
    }
    let next_bar_map = bar_map_for_chart(&manifest.chart);
    if *bar_map != next_bar_map {
        *bar_map = next_bar_map;
    }
    score_notes.0 = notes_to_notation(
        &song_notes.notes,
        timing.resolution,
        &timing.tempo_map,
        &bar_map,
    );
}

/// Keeps [`MusicScorePlayhead`] following the same [`GameplayClock`] every
/// other clock-reading system uses — ordered `.after(GameplayLogic)` per
/// that invariant (see `gameplay/plugin.rs`'s own doc comment on it).
fn update_music_score_playhead(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut playhead: ResMut<MusicScorePlayhead>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let timing = &manifest.chart.timing;
    let tick = seconds_to_tick(clock.get().max(0.0), timing.resolution, &timing.tempo_map);
    // The same lead the notes carry, so the playhead reads against them.
    let tick = tick + pickup_lead_ticks(&manifest.chart);
    let beat = tick as f64 / timing.resolution.max(1) as f64;
    // Unchanged while paused or frozen on a wait-for-note.
    if playhead.0 != beat {
        playhead.0 = beat;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_core::chart::{Repeat, TimeSigPoint};

    #[test]
    fn repeated_bars_follow_the_expanded_performance() {
        let mut chart: HarpChart = serde_json::from_str(
            r#"{
            "song": {"title":"Repeat", "artist":"Test", "genre":"Test", "tempo_bpm":120,
                     "key":"C", "difficulty":"easy", "time_signature":"4/4"},
            "timing": {"resolution":480, "tempo_map":[{"tick":0,"bpm":120}]},
            "harmonica": {"type":"diatonic", "holes":10,
                "bending_profile":"richter_standard",
                "layout":{"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                          "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}},
            "track": [],
            "scoring": {"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}
        }"#,
        )
        .unwrap();
        chart.timing.repeats = vec![Repeat {
            start_tick: 0,
            end_tick: 4 * u64::from(chart.timing.resolution),
            times: Some(2),
            endings: Vec::new(),
        }];
        chart.timing.time_signature_map = Some(vec![
            TimeSigPoint {
                tick: 0,
                time_signature: "4/4".into(),
            },
            TimeSigPoint {
                tick: 960,
                time_signature: "2/4".into(),
            },
        ]);
        chart.track.push(
            serde_json::from_value(serde_json::json!({
                "tick": 960, "duration": 0.5,
                "events": [{"hole": 4, "action": "blow"}]
            }))
            .unwrap(),
        );
        let played = harmonicon_core::repeats::expand(chart);
        let note_ticks: Vec<_> = played.track.iter().map(|item| item.tick.unwrap()).collect();
        assert_eq!(note_ticks, vec![960, 2880]);
        let bars = bar_map_for_chart(&played);
        assert_eq!(bars.beats(0.0, 10.0), vec![2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(bars.split_note(3360, 4320, 60, false)[1].start_beat, 8.0);
    }
}
