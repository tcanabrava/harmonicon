// SPDX-License-Identifier: MIT

//! Call-and-response phrases: a chart's consecutive `TrackItem::call: true`
//! items are synthesized into a one-shot "call" demo (reusing
//! `audio_system::synth` — the same one behind the song editor's own Play
//! button), played automatically before the phrase's notes arrive
//! as the scored "response." The response notes are ordinary
//! `ScheduledNote`s with `force_wait: true` (see `gameplay::wait_freeze_
//! index`), so the player is always given the freeze-and-wait treatment to
//! echo them, regardless of the practice-only `WaitForNoteMode` toggle.
//!
//! Deliberately *not* a clock-jumping feature: the demo plays as a plain
//! fire-and-forget overlay sound (exactly like a hit-feedback or button
//! click), timed by working backwards from the response's own authored
//! start time — `GameplayClock`/the music sink are never touched, so this
//! can't run afoul of the sink-anchoring invariant (see `CLAUDE.md`'s clock
//! notes). A chart author simply needs to leave enough silence before a
//! call group for its demo to finish playing.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::app::SelectedSong;
use crate::audio_system::midi::midi_to_freq_hz;
use crate::audio_system::synth::{Expr, PhraseNote, SAMPLE_RATE, TICKS_PER_BEAT, render_pcm};
use crate::audio_system::wav::encode_wav;
use crate::settings::AudioSettings;
use crate::song::SongManifest;
use crate::song::chart::{HarpChart, Modifier, TrackItem};

use super::{GameplayClock, GameplayRoot, resolve_item_time, target_pitch};

/// Silent gap the demo audio is scheduled to finish before its phrase's
/// first response note reaches the hit line — purely a listening comfort
/// buffer between "here's the phrase" and "now it's your turn."
const LEAD_BUFFER_SECS: f64 = 0.5;

/// One scheduled call-phrase cue: play `audio` once, the instant the clock
/// reaches `trigger_time`. `fired` is a simple one-shot latch rather than
/// removing the cue from the list, so [`CallCues`] stays a plain `Vec`
/// indexed the same way all song (never rebuilt mid-song).
struct CallCue {
    trigger_time: f64,
    audio: Handle<AudioSource>,
    fired: bool,
}

/// Every call-phrase cue for the current song, built once at song setup
/// (`setup_call_cues`) and fired in time order by `fire_call_cues`. Empty
/// for any chart with no `call: true` items — every chart before this
/// feature existed, and any chart that doesn't use it.
#[derive(Resource, Default)]
pub struct CallCues(Vec<CallCue>);

/// Index ranges into `track` (`start..end`, half-open) of each maximal run
/// of consecutive `call: true` items — one call-and-response phrase group
/// per range. Pure so it's directly testable without a loaded chart; charts
/// are assumed authored with a phrase's items contiguous in track order
/// (the same assumption `resolve_item_time`/scoring already make about
/// track order generally).
pub(super) fn call_phrase_groups(track: &[TrackItem]) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut start = None;
    for (i, item) in track.iter().enumerate() {
        if item.call {
            start.get_or_insert(i);
        } else if let Some(s) = start.take() {
            groups.push((s, i));
        }
    }
    if let Some(s) = start {
        groups.push((s, track.len()));
    }
    groups
}

/// The natural (un-bent) pitch name for `item`'s hole/action, same fallback
/// `gameplay_2d::build_combined_notes`/`gameplay_3d::build_notes_3d` use: the
/// event's own authored `note` if present, else the harmonica's layout.
fn natural_pitch(chart: &HarpChart, event: &crate::song::chart::NoteEvent) -> String {
    event.note.clone().unwrap_or_else(|| {
        chart
            .harmonica
            .wind_direction_label(event.hole, &event.action)
    })
}

/// The expression LFO a call-phrase note's demo audio should carry, mapped
/// 1:1 from the chart modifiers that have an `audio_system::synth::Expr`
/// equivalent. Every other modifier (bend, overblow/overdraw, slide) is
/// already baked into the resolved MIDI pitch the caller passes as `freq`
/// (see `target_pitch`), so there's nothing further to render for them —
/// the demo doesn't need to reproduce *how* a note is played, only what it
/// sounds like.
fn demo_expr(modifiers: &[Modifier]) -> Expr {
    modifiers
        .iter()
        .find_map(|m| match m {
            Modifier::Vibrato { oscillation_hz, .. } => Some(Expr::Vibrato(*oscillation_hz)),
            Modifier::WahWah { oscillation_hz, .. } => Some(Expr::Wah(*oscillation_hz)),
            _ => None,
        })
        .unwrap_or(Expr::None)
}

/// Builds the [`PhraseNote`]s for one call-phrase group (`track[start..end]`,
/// as returned by [`call_phrase_groups`]), on the tick grid `audio_system::
/// synth::render_pcm` expects — ticks relative to the group's own first
/// item, not the song's start. `bpm` is the chart's nominal tempo; a chart
/// with tempo automation mid-phrase renders against that single nominal
/// value, same simplification the song editor itself makes (one tempo
/// field, no mid-song automation).
pub(super) fn build_phrase_notes(
    chart: &HarpChart,
    track_group: &[TrackItem],
    group_start_time: f64,
    bpm: f32,
) -> Vec<PhraseNote> {
    let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
    let mut notes = Vec::new();
    for item in track_group {
        let item_time = resolve_item_time(item, &chart.timing);
        let rel_secs = (item_time - group_start_time).max(0.0) as f32;
        let tick = (rel_secs / secs_per_tick).round() as usize;
        let len = ((item.duration as f32 / secs_per_tick).round() as usize).max(1);
        for event in &item.events {
            let modifiers = event.modifiers.clone().unwrap_or_default();
            let natural = natural_pitch(chart, event);
            let freq = target_pitch(&natural, &modifiers).map(|m| midi_to_freq_hz(m as f32));
            notes.push(PhraseNote {
                tick,
                len,
                freq,
                expr: demo_expr(&modifiers),
            });
        }
    }
    notes
}

/// Synthesizes every call-phrase group in the loaded chart into a scheduled
/// [`CallCue`], timed to finish playing [`LEAD_BUFFER_SECS`] before the
/// group's first response note. Runs once at song setup, same point
/// `gameplay_2d`/`gameplay_3d`'s own `setup` build `SongNotes` — Jam Session
/// has no `SongNotes`/response notes to lead into, so this only needs to run
/// for the two scored modes (see the `run_if` on its registration).
pub(super) fn setup_call_cues(
    mut commands: Commands,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut sources: ResMut<Assets<AudioSource>>,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let chart = &manifest.chart;
    let bpm = chart.song.tempo_bpm;

    let mut cues = Vec::new();
    for (start, end) in call_phrase_groups(&chart.track) {
        let group = &chart.track[start..end];
        let Some(first) = group.first() else { continue };
        let group_start_time = resolve_item_time(first, &chart.timing);

        let phrase_notes = build_phrase_notes(chart, group, group_start_time, bpm);
        let secs_per_tick = 60.0 / bpm.max(1.0) / TICKS_PER_BEAT as f32;
        let pcm = render_pcm(&phrase_notes, secs_per_tick);
        if pcm.is_empty() {
            continue;
        }
        let demo_secs = pcm.len() as f64 / SAMPLE_RATE as f64;
        let wav = encode_wav(&pcm, SAMPLE_RATE);
        let audio = sources.add(AudioSource { bytes: wav.into() });

        cues.push(CallCue {
            trigger_time: group_start_time - demo_secs - LEAD_BUFFER_SECS,
            audio,
            fired: false,
        });
    }
    commands.insert_resource(CallCues(cues));
}

/// Fires each call cue once, the instant the clock reaches its
/// `trigger_time` — a plain fire-and-forget overlay sound, like hit-feedback
/// audio; never touches `GameplayClock` or the music sink (see this
/// module's doc comment on why that matters).
pub(super) fn fire_call_cues(
    mut cues: ResMut<CallCues>,
    clock: Res<GameplayClock>,
    audio: Res<AudioSettings>,
    mut commands: Commands,
) {
    let now = clock.get();
    for cue in &mut cues.0 {
        if cue.fired || now < cue.trigger_time {
            continue;
        }
        cue.fired = true;
        commands.spawn((
            AudioPlayer::<AudioSource>(cue.audio.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
            GameplayRoot,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::chart::{Action, NoteEvent};

    fn item(time: f64, duration: f64, call: bool, holes: &[(u8, Action)]) -> TrackItem {
        TrackItem {
            id: None,
            time: Some(time),
            tick: None,
            duration,
            phrase: None,
            groove: None,
            play_mode: None,
            call,
            events: holes
                .iter()
                .map(|(hole, action)| NoteEvent {
                    hole: *hole,
                    action: action.clone(),
                    note: None,
                    modifiers: None,
                })
                .collect(),
        }
    }

    // ── call_phrase_groups ───────────────────────────────────────────────────

    #[test]
    fn no_call_items_yields_no_groups() {
        let track = [item(0.0, 1.0, false, &[(1, Action::Blow)])];
        assert!(call_phrase_groups(&track).is_empty());
    }

    #[test]
    fn a_single_run_of_call_items_is_one_group() {
        let track = [
            item(0.0, 1.0, true, &[(1, Action::Blow)]),
            item(1.0, 1.0, true, &[(2, Action::Blow)]),
            item(2.0, 1.0, false, &[(1, Action::Blow)]),
        ];
        assert_eq!(call_phrase_groups(&track), vec![(0, 2)]);
    }

    #[test]
    fn two_separate_runs_are_two_groups() {
        let track = [
            item(0.0, 1.0, true, &[(1, Action::Blow)]),
            item(1.0, 1.0, false, &[(1, Action::Blow)]),
            item(2.0, 1.0, true, &[(1, Action::Blow)]),
            item(3.0, 1.0, true, &[(1, Action::Blow)]),
        ];
        assert_eq!(call_phrase_groups(&track), vec![(0, 1), (2, 4)]);
    }

    #[test]
    fn a_trailing_call_run_extends_to_the_end_of_the_track() {
        let track = [
            item(0.0, 1.0, false, &[(1, Action::Blow)]),
            item(1.0, 1.0, true, &[(1, Action::Blow)]),
        ];
        assert_eq!(call_phrase_groups(&track), vec![(1, 2)]);
    }

    // ── build_phrase_notes ───────────────────────────────────────────────────

    fn c_diatonic() -> HarpChart {
        serde_json::from_str(
            r#"{
                "song": {"title":"T","artist":"A","tempo_bpm":120.0,"key":"C","difficulty":"easy"},
                "timing": {"resolution":480,"tempo_map":[{"tick":0,"bpm":120.0}]},
                "harmonica": {"type":"diatonic","holes":10,"bending_profile":"richter_standard",
                    "layout": {"blow":["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"],
                               "draw":["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]}},
                "track": [],
                "scoring": {"perfect_window_ms":50,"good_window_ms":100,"miss_window_ms":130}
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn phrase_notes_start_at_tick_zero_relative_to_the_group() {
        let chart = c_diatonic();
        let group = [item(10.0, 0.5, true, &[(1, Action::Blow)])];
        let notes = build_phrase_notes(&chart, &group, 10.0, 120.0);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].tick, 0);
        assert!(
            notes[0].freq.is_some(),
            "hole 1 blow is a real C diatonic note"
        );
    }

    #[test]
    fn phrase_notes_offset_by_their_time_since_the_group_start() {
        let chart = c_diatonic();
        // 120 bpm, 4 ticks/beat -> 0.125s/tick. A note 0.5s after the group
        // start is 4 ticks in.
        let group = [
            item(10.0, 0.5, true, &[(1, Action::Blow)]),
            item(10.5, 0.5, true, &[(2, Action::Blow)]),
        ];
        let notes = build_phrase_notes(&chart, &group, 10.0, 120.0);
        assert_eq!(notes[0].tick, 0);
        assert_eq!(notes[1].tick, 4);
    }

    #[test]
    fn phrase_notes_map_vibrato_and_wah_modifiers_to_the_matching_expr() {
        let chart = c_diatonic();
        let mut vibrato_item = item(10.0, 0.5, true, &[(1, Action::Blow)]);
        vibrato_item.events[0].modifiers = Some(vec![Modifier::Vibrato {
            oscillation_hz: 5.0,
            intensity: None,
        }]);
        let notes = build_phrase_notes(&chart, &[vibrato_item], 10.0, 120.0);
        assert_eq!(notes[0].expr, Expr::Vibrato(5.0));
    }

    #[test]
    fn phrase_notes_have_no_freq_for_an_unproducible_hole() {
        let chart = c_diatonic();
        // Hole 11 doesn't exist on a 10-hole harp.
        let group = [item(10.0, 0.5, true, &[(11, Action::Blow)])];
        let notes = build_phrase_notes(&chart, &group, 10.0, 120.0);
        assert_eq!(notes[0].freq, None);
    }
}
