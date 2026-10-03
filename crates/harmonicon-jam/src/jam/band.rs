// SPDX-License-Identifier: MIT

//! The generated band listening to the player without grading them.
//!
//! [`BandListener`] keeps a short rolling log of coarse activity — how many
//! fresh attacks landed in each beat and whether anything was sounding —
//! and makes two kinds of restrained decision from it, always at musical
//! boundaries and never about pitch: at the top of each four-bar phrase,
//! whether the comping should thin out for the coming phrase (it does after
//! a dense one, and stays put after silence — the pocket is kept rather than
//! filled); and at beat 3 of a phrase's last bar, whether to answer a
//! phrase the player has just finished with a two-beat drum fill or chord
//! push ([`backing::BandAnswer`]). Both are pure functions of the beat log,
//! so a recorded activity stream replays the same reactions; the comping
//! change eases in over a bar and answers are rate-capped, so microphone
//! noise cannot make the arrangement twitch.
//!
//! Nothing here is coaching: no text, no counts, no history the player can
//! see — the behaviour is meant to be heard, and [`AdaptiveBand`] switches
//! it off. Only a `GeneratedJamSession` has this band; a picked song's
//! backing is a recording.

use std::collections::VecDeque;

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use harmonicon_app::app::{JamProgression, SelectedSong};
use harmonicon_audio::AudioSettings;
use harmonicon_core::harmonica::progression_bars;
use harmonicon_core::wav::encode_wav;
use harmonicon_gameplay::gameplay::{ActivePitches, CurrentBar, GameplayClock, GameplayRoot};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_song::song::SongManifest;

use super::backing::{BandAnswer, JamGenre, SAMPLE_RATE, render_band_answer};
use super::improv::ImprovStats;

/// Generated jams are always 4/4 over a four-bar phrase structure.
const BEATS_PER_BAR: usize = 4;
const BARS_PER_PHRASE: usize = 4;
const BEATS_PER_PHRASE: usize = BEATS_PER_BAR * BARS_PER_PHRASE;
/// Beats the log remembers — two phrases, enough to tell a long silence
/// from a breath.
const LOG_BEATS: usize = 2 * BEATS_PER_PHRASE;

/// Attacks per bar at which a phrase counts as dense — roughly steady
/// eighths and up.
const DENSE_ATTACKS_PER_BAR: u32 = 6;
/// Fewest attacks in a phrase for the band to treat it as a phrase worth
/// answering rather than a stray note or mic noise.
const MIN_PHRASE_ATTACKS: u32 = 4;
/// Beats of quiet before beat 3 of the last bar that read as "the phrase
/// has ended" — beats 1 and 2 of that bar.
const RELEASE_BEATS: usize = 2;
/// Fewest bars between two answers, so the band replies now and then, not
/// at every phrase end.
const ANSWER_MIN_GAP_BARS: usize = 8;
/// The comping's gain for the phrase after a dense one.
const THINNED_COMPING: f32 = 0.5;

/// Whether the band reacts at all. On by default; off makes the generated
/// backing play exactly as rendered.
#[derive(Resource)]
pub struct AdaptiveBand(pub bool);

impl Default for AdaptiveBand {
    fn default() -> Self {
        Self(true)
    }
}

/// The "Adaptive band: ..." readout beside its toggle.
#[derive(Component, Default, Clone)]
pub struct AdaptiveBandLabel;

/// What the player did during one beat — counts and presence only, never
/// which notes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct BeatActivity {
    pub attacks: u32,
    pub sounding: bool,
}

/// How busy a phrase was. Descriptive: none of these is better than another.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhraseDensity {
    Silent,
    Sparse,
    Dense,
}

/// Classifies `beats` (a whole phrase, or whatever part of one has passed)
/// by attacks per bar.
pub fn phrase_density(beats: &[BeatActivity]) -> PhraseDensity {
    let attacks: u32 = beats.iter().map(|b| b.attacks).sum();
    if attacks == 0 {
        return PhraseDensity::Silent;
    }
    let bars = (beats.len() as f32 / BEATS_PER_BAR as f32).max(0.25);
    if attacks as f32 / bars >= DENSE_ATTACKS_PER_BAR as f32 {
        PhraseDensity::Dense
    } else {
        PhraseDensity::Sparse
    }
}

/// The comping gain for the phrase following one of `previous` density:
/// thinned after a dense phrase, otherwise full — silence included, so a
/// player who stops for a while hears the pocket held, not filled.
pub fn comping_target_after(previous: PhraseDensity) -> f32 {
    match previous {
        PhraseDensity::Dense => THINNED_COMPING,
        PhraseDensity::Sparse | PhraseDensity::Silent => 1.0,
    }
}

/// The pure decision-maker: fed one completed beat at a time, in order.
#[derive(Resource, Debug)]
pub struct BandListener {
    beats: VecDeque<BeatActivity>,
    comping_target: f32,
    last_answer_beat: Option<usize>,
    answers: usize,
}

impl Default for BandListener {
    fn default() -> Self {
        Self {
            beats: VecDeque::with_capacity(LOG_BEATS),
            comping_target: 1.0,
            last_answer_beat: None,
            answers: 0,
        }
    }
}

impl BandListener {
    /// Records the beat with index `beat` as complete and returns any answer
    /// due as the next beat begins. `beat` counts from the start of the jam;
    /// the caller must feed every beat exactly once, in order.
    pub fn observe(&mut self, beat: usize, activity: BeatActivity) -> Option<BandAnswer> {
        if self.beats.len() == LOG_BEATS {
            self.beats.pop_front();
        }
        self.beats.push_back(activity);
        let next = beat + 1;

        if next.is_multiple_of(BEATS_PER_PHRASE) {
            let start = self.beats.len().saturating_sub(BEATS_PER_PHRASE);
            let phrase = &self.beats.make_contiguous()[start..];
            self.comping_target = comping_target_after(phrase_density(phrase));
        }

        let bar_in_phrase = (next / BEATS_PER_BAR) % BARS_PER_PHRASE;
        let beat_in_bar = next % BEATS_PER_BAR;
        if bar_in_phrase == BARS_PER_PHRASE - 1
            && beat_in_bar == RELEASE_BEATS
            && self.phrase_just_ended()
            && self.answer_allowed(next)
        {
            self.last_answer_beat = Some(next);
            self.answers += 1;
            return Some(if self.answers % 2 == 1 { BandAnswer::Drums } else { BandAnswer::Chord });
        }
        None
    }

    /// Records `beat` as complete with `activity`, then every beat before
    /// `current` as silent — a frame long enough to skip a beat still feeds
    /// each one, so no phrase boundary is missed. A stall longer than the
    /// log replays only the beats the log can hold. Returns the latest
    /// answer due.
    pub fn observe_until(
        &mut self,
        beat: usize,
        activity: BeatActivity,
        current: usize,
    ) -> Option<BandAnswer> {
        let mut answer = self.observe(beat, activity);
        for skipped in (beat + 1).max(current.saturating_sub(LOG_BEATS))..current {
            answer = self.observe(skipped, BeatActivity::default()).or(answer);
        }
        answer
    }

    /// The comping gain the band currently wants (1.0 = as rendered).
    pub fn comping_target(&self) -> f32 {
        self.comping_target
    }

    /// Whether the phrase so far had enough in it to be a phrase, and the
    /// last [`RELEASE_BEATS`] were quiet — the player has stopped, not
    /// merely paused between notes.
    fn phrase_just_ended(&self) -> bool {
        let n = self.beats.len();
        if n < RELEASE_BEATS {
            return false;
        }
        // The phrase so far: everything since its first beat, which at beat
        // 3 of the last bar is `BEATS_PER_PHRASE - RELEASE_BEATS` beats.
        let start = n.saturating_sub(BEATS_PER_PHRASE - RELEASE_BEATS);
        let split = n - RELEASE_BEATS;
        let body_attacks: u32 = self.beats.range(start..split).map(|b| b.attacks).sum();
        let released = self.beats.range(split..).all(|b| b.attacks == 0 && !b.sounding);
        body_attacks >= MIN_PHRASE_ATTACKS && released
    }

    fn answer_allowed(&self, beat: usize) -> bool {
        self.last_answer_beat.is_none_or(|last| beat - last >= ANSWER_MIN_GAP_BARS * BEATS_PER_BAR)
    }
}

/// Per-frame accumulation into the current beat, plus the eased comping
/// gain the sinks actually get.
#[derive(Resource, Default)]
pub struct BandTracker {
    beat_index: Option<usize>,
    current: BeatActivity,
    last_attack_total: u32,
    /// The comping gain in force right now, eased toward the listener's
    /// target over about a bar.
    pub comping_gain: f32,
}

impl BandTracker {
    /// A fresh tracker for a new jam, comping at full.
    pub fn fresh() -> Self {
        Self { comping_gain: 1.0, ..Self::default() }
    }
}

/// The index of the beat `clock_secs` falls in at `bpm`; `None` during the
/// countdown.
pub fn beat_index(clock_secs: f64, bpm: f32) -> Option<usize> {
    if clock_secs < 0.0 {
        return None;
    }
    Some((clock_secs * f64::from(bpm.max(1.0)) / 60.0).floor() as usize)
}

/// Moves `gain` toward `target` at a rate that crosses the whole thinning
/// range in `ramp_secs`.
pub fn ease_gain(gain: f32, target: f32, dt_secs: f32, ramp_secs: f32) -> f32 {
    let step = (1.0 - THINNED_COMPING) * dt_secs / ramp_secs.max(1e-3);
    if gain < target { (gain + step).min(target) } else { (gain - step).max(target) }
}

/// Keeps the "Adaptive band: ..." readout in step with the toggle.
pub fn update_adaptive_band_label(
    adaptive: Res<AdaptiveBand>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<AdaptiveBandLabel>>,
) {
    if !adaptive.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(String::from(if adaptive.0 {
            loc.msg("jam-adaptive-band-on")
        } else {
            loc.msg("jam-adaptive-band-off")
        }));
    }
}

/// Feeds the listener one beat at a time from the jam clock and the
/// always-on improv tally (fresh attacks) plus live pitch presence, eases
/// the comping gain toward what the listener wants, and fires any answer
/// it returns — fire-and-forget, like the call-and-response phrase. With
/// the band switched off the log still runs (so switching it back on has
/// context) but the comping returns to full and nothing answers.
pub fn listen_and_react(
    adaptive: Res<AdaptiveBand>,
    clock: Res<GameplayClock>,
    time: Res<Time>,
    stats: Res<ImprovStats>,
    active: Res<ActivePitches>,
    current: Res<CurrentBar>,
    progression: Res<JamProgression>,
    genre: Res<JamGenre>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    audio: Res<AudioSettings>,
    mut listener: ResMut<BandListener>,
    mut tracker: ResMut<BandTracker>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
) {
    let Some(manifest) = manifests.get(&selected.0) else {
        return;
    };
    let bpm = manifest.chart.song.tempo_bpm;

    let total = stats.total();
    tracker.current.attacks += total.saturating_sub(tracker.last_attack_total);
    tracker.last_attack_total = total;
    tracker.current.sounding |= !active.0.is_empty();

    let mut answer = None;
    if let Some(beat) = beat_index(clock.get(), bpm) {
        if let Some(prev) = tracker.beat_index
            && beat > prev
        {
            let completed = std::mem::take(&mut tracker.current);
            answer = listener.observe_until(prev, completed, beat);
        }
        tracker.beat_index = Some(beat);
    }

    let target = if adaptive.0 { listener.comping_target() } else { 1.0 };
    let secs_per_bar = 60.0 / bpm.max(1.0) * BEATS_PER_BAR as f32;
    tracker.comping_gain = ease_gain(tracker.comping_gain, target, time.delta_secs(), secs_per_bar);

    let Some(answer) = answer.filter(|_| adaptive.0) else {
        return;
    };
    let (root, quality) = &progression_bars(&manifest.chart.song.key, progression.0)[current.0];
    let pcm = render_band_answer(answer, genre.0, root, *quality, bpm);
    if pcm.is_empty() {
        return;
    }
    let source = sources.add(AudioSource { bytes: encode_wav(&pcm, SAMPLE_RATE).into() });
    commands.spawn((
        AudioPlayer::<AudioSource>(source),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(audio.music_volume)),
        GameplayRoot,
    ));
}

#[cfg(test)]
mod tests;
