// SPDX-License-Identifier: MIT

//! A click track for Record/Play/Practice, reusing `gameplay::
//! metronome_overlay`'s pure tick math, shared click-audio resources
//! (`MetronomeSounds`), and the same `MetronomeTempo`/`MetronomeFeel`/
//! `MetronomeMuted` globals gameplay and the Bending Trainer already use —
//! so a player's mute preference carries over into the editor instead of
//! resetting. Not gameplay's own click-driving systems though, which are
//! tied to `GameplayClock`: the editor has its own clock
//! (`playback::Playhead`), so [`click_metronome`] reads from that,
//! sharing only the pure click-selection logic
//! (`metronome_overlay::play_click_if_due`).
//!
//! Also owns the Record transport's count-in: pressing Play to start a
//! *fresh* take (not resuming a paused one) doesn't start capturing
//! immediately — [`CountIn`] ticks off one bar of clicks first, then
//! [`finish_count_in`] hands off to `record::start_record` for real, so
//! nothing is lost or misjudged by counting in first.

use bevy::audio::AudioSource;
use bevy::prelude::*;

use harmonicon_audio::AudioSettings;
use harmonicon_audio::pitch_detect::PitchRange;
use harmonicon_gameplay::gameplay::metronome_overlay::{
    MetronomeFeel, MetronomeMuted, MetronomeSounds, MetronomeTempo, play_click_if_due,
};
use harmonicon_ui::music_score::{MeterMap, MusicScoreMeter};

use super::TICKS_PER_BEAT;
use super::playback::{EditorAudio, PendingMusicSeek, Playhead, secs_per_tick};
use super::record::{RecordState, start_record};
use super::state::EditorState;

/// The last click tick played this editing session — the editor's own
/// counterpart to `metronome_overlay::LastClickedTick`, kept separate so
/// switching between gameplay and the editor can't leave a stale tick
/// index behind that suppresses (or double-fires) the first click in
/// whichever context comes next.
#[derive(Resource, Default)]
pub(super) struct EditorLastClickedTick(Option<i64>);

/// A pending count-in before a fresh Record take actually starts
/// capturing — one bar of metronome clicks, giving the player a beat to
/// prepare, same as any DAW's count-in. Counts down in real (wall-clock)
/// time via `Time::delta`, not `Playhead::elapsed`: the take and its
/// clock haven't started yet, so there's nothing for `Playhead` to count.
#[derive(Resource, Default)]
pub(super) struct CountIn {
    total_secs: f32,
    remaining_secs: f32,
    active: bool,
    meter: MusicScoreMeter,
}

impl CountIn {
    pub(super) fn active(&self) -> bool {
        self.active
    }

    /// Seconds left, for the status bar — `None` whenever there's nothing
    /// to show (mirrors `active`, but as the value a caller actually wants
    /// to display rather than a bare bool).
    pub(super) fn remaining_secs_display(&self) -> Option<f32> {
        self.active.then_some(self.remaining_secs)
    }

    /// Seconds already elapsed since the count-in started — what
    /// `play_click_if_due` needs as its "clock" to click on beat.
    fn elapsed_secs(&self) -> f32 {
        self.total_secs - self.remaining_secs
    }

    fn start(&mut self, total_secs: f32, meter: MusicScoreMeter) {
        self.total_secs = total_secs;
        self.remaining_secs = total_secs;
        self.active = true;
        self.meter = meter;
    }

    pub(super) fn stop(&mut self) {
        self.active = false;
    }
}

/// Seconds of count-in: one bar of the chart's own meter, off the same
/// `MusicScoreMeter` the click and the ruler use. A 3/4 chart counts in
/// three quarters; a 6/8 chart counts in six eighths — one second at 180,
/// not the two a quarter-note reading gave.
pub(super) fn count_in_secs(bpm: f32, meter: MusicScoreMeter) -> f32 {
    meter.bar_secs(f64::from(bpm.max(1.0))) as f32
}

/// `EditorState::tempo`'s flat nominal BPM, recovered from `playback::
/// secs_per_tick` rather than re-parsing `state.tempo` directly, so the
/// two can't drift onto different fallback/clamp behaviour.
pub(super) fn tempo_bpm(state: &EditorState) -> f32 {
    60.0 / (secs_per_tick(state) * TICKS_PER_BEAT as f32)
}

/// Starts a count-in for a fresh Record take — called by the Record Play
/// button instead of `record::start_record` directly when there's no take
/// already in flight to resume. See [`CountIn`]'s own doc comment.
///
/// One bar of the meter in force *where the take starts*
/// (`playhead_secs`, the parked playhead), not the opening meter: a take
/// punched in after a 4/4 → 3/4 change counts three, not four.
pub(super) fn begin_count_in(state: &EditorState, playhead_secs: f32, count_in: &mut CountIn) {
    let tick = (playhead_secs.max(0.0) / secs_per_tick(state)).round() as u64;
    let meter = state.meter_map().meter_at(tick);
    count_in.start(count_in_secs(tempo_bpm(state), meter), meter);
}

/// Clock and meter local to the active meter segment. Resetting the clock at
/// a change makes that change's first beat a downbeat even when its tick was
/// not a bar boundary in the preceding meter. A pickup starts the first
/// segment's clock part-way into its bar, so the pickup clicks the bar's
/// last beats and bar 1 gets the accent.
fn segment_clock(map: &MeterMap, secs_per_tick: f32, elapsed_secs: f32) -> (f64, MusicScoreMeter) {
    let tick = (elapsed_secs.max(0.0) / secs_per_tick).round() as u64;
    let segment = map.segment_at(tick);
    (
        (tick - segment.start_tick + segment.phase_ticks) as f64 * f64::from(secs_per_tick),
        segment.meter,
    )
}

pub(super) struct MeterClockCache {
    opening: String,
    changes: Vec<(usize, String)>,
    pickup: String,
    map: MeterMap,
}

impl MeterClockCache {
    fn from_state(state: &EditorState) -> Self {
        Self {
            opening: state.time_signature.clone(),
            changes: state.meter_changes.clone(),
            pickup: state.pickup_beats.clone(),
            map: state.meter_map(),
        }
    }

    fn matches(&self, state: &EditorState) -> bool {
        self.opening == state.time_signature
            && self.changes == state.meter_changes
            && self.pickup == state.pickup_beats
    }

    pub(super) fn map_for<'a>(cache: &'a mut Option<Self>, state: &EditorState) -> &'a MeterMap {
        if cache.as_ref().is_none_or(|entry| !entry.matches(state)) {
            *cache = Some(Self::from_state(state));
        }
        &cache.as_ref().unwrap().map
    }
}

/// Keeps `MetronomeTempo` in step with the chart currently being edited —
/// the editor's own counterpart to
/// `metronome_overlay::set_tempo_from_song`, run continuously (rather than
/// once `OnEnter`) since the tempo field is itself live-editable. Only one
/// of gameplay/the Bending Trainer/the editor is ever active at a time
/// (different `AppState`s), so this can't fight the others over the same
/// shared resource — whichever context is entered next reseeds it.
pub(super) fn sync_tempo(state: Res<EditorState>, mut tempo: ResMut<MetronomeTempo>) {
    let bpm = tempo_bpm(&state);
    let meter = state.meter();
    if tempo.bpm != bpm || tempo.meter != meter {
        tempo.bpm = bpm;
        tempo.meter = meter;
    }
}

/// Plays the metronome clicks for the editor's own clock (`Playhead`)
/// while it's actually running — Record, Play, and Practice all drive the
/// same `Playhead`, so gating on it covers whichever is active without
/// needing to know which. Silent during a count-in ([`tick_count_in`]
/// clicks instead, against its own clock) and while nothing is playing.
pub(super) fn click_metronome(
    state: Res<EditorState>,
    playhead: Res<Playhead>,
    count_in: Res<CountIn>,
    tempo: Res<MetronomeTempo>,
    muted: Res<MetronomeMuted>,
    feel: Res<MetronomeFeel>,
    sounds: Res<MetronomeSounds>,
    audio: Res<AudioSettings>,
    mut last: ResMut<EditorLastClickedTick>,
    mut meter_cache: Local<Option<MeterClockCache>>,
    mut commands: Commands,
) {
    if count_in.active() || !playhead.playing || playhead.paused {
        return;
    }
    let (clock, meter) = segment_clock(
        MeterClockCache::map_for(&mut meter_cache, &state),
        secs_per_tick(&state),
        playhead.elapsed,
    );
    let active_tempo = MetronomeTempo {
        bpm: tempo.bpm,
        meter,
        // `segment_clock` already counts from a bar line, pickup included.
        lead_beats: 0.0,
    };
    play_click_if_due(
        clock,
        &active_tempo,
        *feel,
        muted.0,
        &sounds,
        &audio,
        &mut last.0,
        &mut commands,
    );
}

/// Ticks a pending count-in: clicks against its own elapsed-since-start
/// clock (separate from `Playhead`, which isn't running yet — see
/// [`CountIn`]'s doc comment) and counts `remaining_secs` down by the
/// frame delta. Split from [`finish_count_in`] (which reacts once it
/// reaches zero) purely to stay under a single Bevy system's parameter
/// limit — the two always run back to back.
pub(super) fn tick_count_in(
    time: Res<Time>,
    mut count_in: ResMut<CountIn>,
    mut last: ResMut<EditorLastClickedTick>,
    tempo: Res<MetronomeTempo>,
    muted: Res<MetronomeMuted>,
    feel: Res<MetronomeFeel>,
    sounds: Res<MetronomeSounds>,
    audio: Res<AudioSettings>,
    mut commands: Commands,
) {
    if !count_in.active() {
        return;
    }
    let count_in_tempo = MetronomeTempo { bpm: tempo.bpm, meter: count_in.meter, lead_beats: 0.0 };
    play_click_if_due(
        count_in.elapsed_secs() as f64,
        &count_in_tempo,
        *feel,
        muted.0,
        &sounds,
        &audio,
        &mut last.0,
        &mut commands,
    );
    count_in.remaining_secs -= time.delta_secs();
}

/// Hands off to `record::start_record` the instant [`tick_count_in`]'s
/// countdown reaches zero — see [`tick_count_in`]'s doc comment for why
/// this is a separate system rather than one.
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_count_in(
    mut count_in: ResMut<CountIn>,
    mut last: ResMut<EditorLastClickedTick>,
    state: Res<EditorState>,
    mut sources: ResMut<Assets<AudioSource>>,
    settings: Res<AudioSettings>,
    playing: Query<Entity, With<EditorAudio>>,
    mut record: ResMut<RecordState>,
    mut playhead: ResMut<Playhead>,
    mut pitch_range: ResMut<PitchRange>,
    mut music_seek: ResMut<PendingMusicSeek>,
    mut commands: Commands,
) {
    if !count_in.active() || count_in.remaining_secs > 0.0 {
        return;
    }
    count_in.stop();
    last.0 = None;
    start_record(
        &state,
        &mut sources,
        &settings,
        &playing,
        &mut record,
        &mut playhead,
        &mut pitch_range,
        &mut music_seek,
        &mut commands,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── CountIn ───────────────────────────────────────────────────────────

    #[test]
    fn count_in_starts_inactive() {
        let count_in = CountIn::default();
        assert!(!count_in.active());
        assert_eq!(count_in.remaining_secs_display(), None);
    }

    #[test]
    fn starting_activates_and_seeds_remaining_time() {
        let mut count_in = CountIn::default();
        count_in.start(2.0, meter("4/4"));
        assert!(count_in.active());
        assert_eq!(count_in.remaining_secs_display(), Some(2.0));
        assert_eq!(count_in.elapsed_secs(), 0.0);
    }

    #[test]
    fn elapsed_grows_as_remaining_shrinks() {
        let mut count_in = CountIn::default();
        count_in.start(2.0, meter("4/4"));
        count_in.remaining_secs = 0.5;
        assert_eq!(count_in.elapsed_secs(), 1.5);
    }

    #[test]
    fn stopping_deactivates() {
        let mut count_in = CountIn::default();
        count_in.start(2.0, meter("4/4"));
        count_in.stop();
        assert!(!count_in.active());
        assert_eq!(count_in.remaining_secs_display(), None);
    }

    // ── count_in_secs ─────────────────────────────────────────────────────

    fn meter(s: &str) -> MusicScoreMeter {
        harmonicon_ui::music_score::parse_time_signature(s)
    }

    #[test]
    fn count_in_secs_is_one_bar_at_the_given_bpm() {
        // 4 beats at 120bpm (0.5s/beat) = 2.0s.
        assert_eq!(count_in_secs(120.0, meter("4/4")), 2.0);
    }

    #[test]
    fn count_in_secs_scales_inversely_with_bpm() {
        assert_eq!(count_in_secs(60.0, meter("4/4")), 4.0);
        assert_eq!(count_in_secs(240.0, meter("4/4")), 1.0);
    }

    #[test]
    fn count_in_secs_counts_one_bar_of_the_charts_own_meter() {
        // A 3/4 chart counts in three beats, not four.
        assert_eq!(count_in_secs(120.0, meter("3/4")), 1.5);
        // And a 6/8 chart six *eighths* — one second at 180, where reading
        // the meter as whole quarters gave 3 × (60/180) = 1 s by luck but
        // 3/8 (1.5 quarters, rounded to 2) counted in 1 s for a 0.75 s bar.
        assert!((count_in_secs(180.0, meter("6/8")) - 1.0).abs() < 1e-6);
        assert!((count_in_secs(120.0, meter("3/8")) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn count_in_secs_clamps_a_nonpositive_bpm() {
        // Falls back to treating bpm as 1, rather than dividing by zero or
        // going negative.
        let m = meter("4/4");
        assert_eq!(count_in_secs(0.0, m), count_in_secs(1.0, m));
        assert_eq!(count_in_secs(-10.0, m), count_in_secs(1.0, m));
    }

    // ── tempo_bpm ─────────────────────────────────────────────────────────

    #[test]
    fn tempo_bpm_reads_the_editors_own_tempo_field() {
        let state = EditorState { tempo: "90".into(), ..Default::default() };
        assert!((tempo_bpm(&state) - 90.0).abs() < 1e-3);
    }

    #[test]
    fn tempo_bpm_falls_back_to_120_for_an_unparseable_tempo() {
        let state = EditorState { tempo: "not a number".into(), ..Default::default() };
        assert!((tempo_bpm(&state) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn metronome_clock_restarts_on_a_meter_changes_downbeat() {
        let state = EditorState {
            tempo: "120".into(),
            meter_changes: vec![(84, "3/4".into())],
            ..Default::default()
        };
        let map = state.meter_map();
        let spt = secs_per_tick(&state);
        let (at_change, changed_meter) = segment_clock(&map, spt, 3.5);
        assert!(at_change.abs() < 1e-6);
        assert_eq!(changed_meter, meter("3/4"));
        let (one_quarter_later, _) = segment_clock(&map, spt, 4.0);
        assert!((one_quarter_later - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_pickup_clicks_the_last_beat_and_accents_bar_one() {
        use harmonicon_ui::dialogs::metronome::{MetronomeFeel, is_downbeat, tick_index};
        // One-beat pickup in 4/4 at 120: bar 1 starts half a second in.
        let state =
            EditorState { tempo: "120".into(), pickup_beats: "1".into(), ..Default::default() };
        let map = state.meter_map();
        let spt = secs_per_tick(&state);
        let beat = |elapsed: f32| {
            let (clock, _) = segment_clock(&map, spt, elapsed);
            tick_index(clock, 0.5, MetronomeFeel::Straight).unwrap()
        };
        assert!(!is_downbeat(beat(0.0), 4.0), "the pickup is beat 4");
        assert!(is_downbeat(beat(0.5), 4.0), "bar 1's first beat is accented");
    }

    #[test]
    fn count_in_remembers_the_meter_at_the_parked_playhead() {
        let state = EditorState {
            tempo: "120".into(),
            meter_changes: vec![(48, "3/4".into())],
            ..Default::default()
        };
        let mut count_in = CountIn::default();
        begin_count_in(&state, 2.0, &mut count_in);
        assert_eq!(count_in.meter, meter("3/4"));
        assert_eq!(count_in.total_secs, 1.5);
    }
}
