// SPDX-License-Identifier: MIT

//! The adaptive drill: per-target hit/miss stats persisted on the profile,
//! the weighted pick of the next target, the hold-to-advance / timeout loop,
//! and the drill's own labels and button state.

use super::*;

// ── Adaptive drill mode ─────────────────────────────────────────────────────────

/// Per hole/technique practice evidence, used to weight which targets the
/// drill serves up next. Deliberately *not* a lifetime hit rate: an early
/// run of beginner failures would otherwise dominate that target's weight
/// forever, so the selection reads recent control, recent steadiness and
/// staleness instead, and keeps the lifetime counters only for the progress
/// map's accuracy readout.
#[derive(Default, Clone, Copy)]
pub struct DrillStat {
    pub attempts: u32,
    pub hits: u32,
    pub skips: u32,
    pub recent_control: f32,
    pub recent_samples: u32,
    pub recent_stability_cents: f32,
    pub practiced_at: u32,
}

/// Weight of the recency half of the moving estimates — one attempt moves
/// them a quarter of the way to what it observed.
const RECENT_WEIGHT: f32 = 0.25;
/// Attempts elsewhere before a target counts as fully stale. Roughly one
/// pass through the largest scope, so every target resurfaces on its own
/// without the player having to go find it.
pub(super) const STALE_SPAN: f32 = 12.0;

impl DrillStat {
    /// Selection weight: never-seen targets lead, then weak ones, then ones
    /// held unsteadily, then ones not practiced for a while. A target the
    /// player controls, holds steady and saw recently fades toward the 1.0
    /// floor. `sequence` is the drill's current ordinal (see
    /// [`next_sequence`]).
    pub(super) fn weight(&self, sequence: u32) -> f32 {
        if self.attempts == 0 && self.skips == 0 {
            return 2.5;
        }
        let control = if self.recent_samples > 0 {
            self.recent_control
        } else if self.attempts > 0 {
            self.hits as f32 / self.attempts as f32
        } else {
            // Only ever skipped: nothing was heard, so nothing is known.
            return 2.5;
        };
        let wobble = (self.recent_stability_cents / UNSTABLE_RESIDUAL_CENTS).clamp(0.0, 1.0);
        let stale =
            (sequence.saturating_sub(self.practiced_at) as f32 / STALE_SPAN).clamp(0.0, 1.0);
        1.0 + 3.0 * (1.0 - control) + wobble + stale
    }

    /// Folds one completed attempt into both the lifetime counters and the
    /// moving estimates. `stability_cents` is the line-fit residual of the
    /// held pitch, or `None` when the attempt never produced a measurable
    /// hold — in which case the previous steadiness estimate stands rather
    /// than being reset by an absence of evidence.
    pub(super) fn record_attempt(
        &mut self,
        hit: bool,
        stability_cents: Option<f32>,
        sequence: u32,
    ) {
        self.attempts += 1;
        self.hits += u32::from(hit);
        self.practiced_at = sequence;
        let value = if hit { 1.0 } else { 0.0 };
        self.recent_control = if self.recent_samples == 0 {
            value
        } else {
            self.recent_control * (1.0 - RECENT_WEIGHT) + value * RECENT_WEIGHT
        };
        self.recent_samples = self.recent_samples.saturating_add(1).min(20);
        if let Some(cents) = stability_cents {
            self.recent_stability_cents = if self.recent_stability_cents == 0.0 {
                cents
            } else {
                self.recent_stability_cents * (1.0 - RECENT_WEIGHT) + cents * RECENT_WEIGHT
            };
        }
    }

    /// A skip — the attempt timed out with nothing credible heard on the
    /// selected hole. Counted, and stamped as practiced so the drill doesn't
    /// keep serving the same target while the player is away, but it moves
    /// no estimate of control: there is no evidence either way.
    pub(super) fn record_skip(&mut self, sequence: u32) {
        self.skips += 1;
        self.practiced_at = sequence;
    }
}

/// The drill's monotonic attempt ordinal, derived from the stats themselves
/// rather than stored alongside them — one less field to keep in step across
/// save/load, and it cannot drift from the `practiced_at` stamps it is
/// compared against.
pub(super) fn next_sequence(stats: &std::collections::HashMap<(u8, Technique), DrillStat>) -> u32 {
    stats.values().map(|stat| stat.practiced_at).max().unwrap_or(0) + 1
}

/// Which slice of the harp the drill draws targets from. The default is
/// deliberately the smallest: a novice must never be handed an overdraw
/// before they can control hole 2, so overbends are reachable only by
/// asking for them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrillScope {
    #[default]
    FirstBends,
    AllBends,
    BlowBends,
    Overbends,
    Custom,
}

impl DrillScope {
    const ALL: [Self; 5] =
        [Self::FirstBends, Self::AllBends, Self::BlowBends, Self::Overbends, Self::Custom];
    fn next(self) -> Self {
        let index = Self::ALL.iter().position(|scope| *scope == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// Auto-advancing ear-training drill: picks a random hole/technique, waits
/// for the player to sustain it in tune, then moves on. Tracks a running
/// hit rate per target so weak spots come up more often than mastered ones.
#[derive(Resource, Default)]
pub struct DrillState {
    pub enabled: bool,
    pub stats: std::collections::HashMap<(u8, Technique), DrillStat>,
    /// How long the current target has been held in tune, in seconds.
    pub hold_secs: f32,
    /// How long the current target has been active at all, in seconds —
    /// resets the drill to a fresh target if the player gets stuck.
    pub elapsed_secs: f32,
    pub streak: u32,
    pub scope: DrillScope,
    /// True after any pitch in the selected hole's family was heard.
    pub attempted: bool,
    /// Cells the [`DrillScope::Custom`] scope draws from, toggled by clicking
    /// the diagram while that scope is showing. Empty falls back to whatever
    /// cell is selected, so switching to Custom always drills *something*.
    pub custom: HashSet<(u8, Technique)>,
}

#[derive(Component)]
pub struct DrillScopeLabel;

pub fn cycle_drill_scope(
    _: On<Activate>,
    key: Res<TrainerKey>,
    mut target: ResMut<TrainerTarget>,
    mut drill: ResMut<DrillState>,
) {
    drill.scope = drill.scope.next();
    drill.hold_secs = 0.0;
    drill.elapsed_secs = 0.0;
    drill.attempted = false;
    if drill.enabled {
        let harp = key.harp();
        if let Some(next) =
            pick_next_target(harp, &drill.stats, Some(*target), drill.scope, &drill.custom, *target)
        {
            *target = next;
        }
    }
}

/// The scope readout's own text, separated from the button so the Custom
/// scope can say how many cells it holds — the cell set is only visible on
/// the diagram, and a count is what tells the player their clicks landed.
pub(super) fn scope_status(loc: &Localization, scope: DrillScope, custom_cells: usize) -> String {
    let label = match scope {
        DrillScope::FirstBends => loc.msg("bending-scope-first").to_string(),
        DrillScope::AllBends => loc.msg("bending-scope-all").to_string(),
        DrillScope::BlowBends => loc.msg("bending-scope-blow").to_string(),
        DrillScope::Overbends => loc.msg("bending-scope-over").to_string(),
        DrillScope::Custom if custom_cells == 0 => {
            loc.msg("bending-scope-custom-selected").to_string()
        }
        DrillScope::Custom => loc
            .msg_args("bending-scope-custom-count", &[("count", custom_cells.to_string())])
            .to_string(),
    };
    loc.msg_args("bending-scope-status", &[("scope", label)]).to_string()
}

pub fn update_drill_scope_label(
    drill: Res<DrillState>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<DrillScopeLabel>>,
) {
    if !drill.is_changed() {
        return;
    }
    let status = scope_status(&loc, drill.scope, drill.custom.len());
    for mut text in &mut labels {
        *text = Text::new(status.clone());
    }
}

/// A `"{hole}:{technique}"` key for [`PlayerProfile::drills`] — stats aren't
/// keyed by [`TrainerKey`], since the physical skill a (hole, technique) pair
/// drills is the same regardless of which key harp it's practiced on.
pub(super) fn drill_key(hole: u8, technique: Technique) -> String {
    format!("{hole}:{}", technique.storage_key())
}

/// Snapshot of in-memory drill stats into the flat, string-keyed shape
/// `PlayerProfile::drills` persists.
pub(super) fn stats_to_profile(
    stats: &std::collections::HashMap<(u8, Technique), DrillStat>,
) -> std::collections::HashMap<String, DrillRecord> {
    stats
        .iter()
        .map(|(&(hole, technique), stat)| {
            (
                drill_key(hole, technique),
                DrillRecord {
                    attempts: stat.attempts,
                    hits: stat.hits,
                    skips: stat.skips,
                    recent_control: stat.recent_control,
                    recent_samples: stat.recent_samples,
                    recent_stability_cents: stat.recent_stability_cents,
                    practiced_at: stat.practiced_at,
                },
            )
        })
        .collect()
}

/// Inverse of [`stats_to_profile`], for loading. Entries with a key that
/// doesn't parse (a hand-edited or future-version `profile.json`) are
/// silently dropped rather than failing the whole load.
pub(super) fn stats_from_profile(
    drills: &std::collections::HashMap<String, DrillRecord>,
) -> std::collections::HashMap<(u8, Technique), DrillStat> {
    drills
        .iter()
        .filter_map(|(key, record)| {
            let (hole_str, tech_str) = key.split_once(':')?;
            let hole: u8 = hole_str.parse().ok()?;
            let technique = Technique::from_storage_key(tech_str)?;
            Some((
                (hole, technique),
                DrillStat {
                    attempts: record.attempts,
                    hits: record.hits,
                    skips: record.skips,
                    recent_control: record.recent_control,
                    recent_samples: record.recent_samples,
                    recent_stability_cents: record.recent_stability_cents,
                    practiced_at: record.practiced_at,
                },
            ))
        })
        .collect()
}

/// A (hole, technique)'s hit-rate, or `None` if it's never been attempted —
/// kept distinct from a `0.0` accuracy so [`progress_tint`] can tell "never
/// tried" (neutral) apart from "tried and always missed" (red).
pub(super) fn drill_accuracy(stat: Option<&DrillStat>) -> Option<f32> {
    let stat = stat?;
    if stat.attempts == 0 {
        return None;
    }
    Some(stat.hits as f32 / stat.attempts as f32)
}

/// Idle-cell background color for a (hole, technique)'s drill progress: the
/// diagram's ordinary idle color for "never attempted", blending from a dim
/// red (weak) to a dim green (mastered) as hit-rate climbs — so the same
/// diagram used to pick a drill target also doubles as a progress map, no
/// separate screen needed.
pub(super) fn progress_tint(accuracy: Option<f32>) -> Color {
    let Some(acc) = accuracy else {
        return CELL_DEFAULT;
    };
    let acc = acc.clamp(0.0, 1.0);
    let weak = Color::srgb(0.32, 0.12, 0.12).to_srgba();
    let strong = Color::srgb(0.14, 0.34, 0.16).to_srgba();
    Color::srgb(
        weak.red + (strong.red - weak.red) * acc,
        weak.green + (strong.green - weak.green) * acc,
        weak.blue + (strong.blue - weak.blue) * acc,
    )
}

/// Paints every selectable diagram cell: the live mic highlight while its
/// pitch sounds, else its drill accuracy (see [`progress_tint`]). The sole
/// writer of these cells' `BackgroundColor` —
/// [`harmonica_overlay::update_harmonica_overlay`] skips them — so it writes
/// only when a cell's colour actually changes.
pub fn update_drill_progress_tint(
    active: Res<ActivePitches>,
    drill: Res<DrillState>,
    mut cells: Query<(&HarpOverlayCell, &DiagramCellTarget, &mut BackgroundColor)>,
) {
    for (cell, target, mut bg) in &mut cells {
        let lit = cell.midi.is_some_and(|m| active.0.iter().any(|p| p.midi == m));
        let color = if lit {
            CELL_LIT
        } else {
            let accuracy = row_to_technique(target.row)
                .and_then(|technique| drill.stats.get(&(target.hole, technique)));
            progress_tint(drill_accuracy(accuracy))
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

/// Every (hole, technique) pair the current harp can actually produce.
pub(super) fn valid_targets(harp: &Harmonica) -> Vec<TrainerTarget> {
    (1..=10)
        .flat_map(|hole| {
            ALL_TECHNIQUES.iter().map(move |&technique| TrainerTarget { hole, technique })
        })
        .filter(|t| target_note(harp, *t).is_some())
        .collect()
}

/// The targets `scope` draws from on `harp`. `custom` is the player's own
/// cell set and `selected` the currently picked cell; an empty custom set
/// falls back to `selected` so [`DrillScope::Custom`] is never an empty pool
/// (which would strand the drill with nothing to serve).
pub(super) fn targets_for_scope(
    harp: &Harmonica,
    scope: DrillScope,
    custom: &HashSet<(u8, Technique)>,
    selected: TrainerTarget,
) -> Vec<TrainerTarget> {
    valid_targets(harp)
        .into_iter()
        .filter(|target| match scope {
            DrillScope::FirstBends => {
                matches!(
                    (target.hole, target.technique),
                    (2 | 3, Technique::Bend1) | (3, Technique::Bend2)
                )
            }
            DrillScope::AllBends => {
                matches!(target.technique, Technique::Bend1 | Technique::Bend2 | Technique::Bend3)
            }
            DrillScope::BlowBends => {
                target.hole >= 7
                    && matches!(
                        target.technique,
                        Technique::Bend1 | Technique::Bend2 | Technique::Bend3
                    )
            }
            DrillScope::Overbends => target.technique == Technique::Over,
            DrillScope::Custom if custom.is_empty() => *target == selected,
            DrillScope::Custom => custom.contains(&(target.hole, target.technique)),
        })
        .collect()
}

/// Weighted-random pick of the next drill target within `scope`, biased
/// toward targets the player controls least well, holds least steadily, or
/// hasn't seen for a while (see [`DrillStat::weight`]), and avoiding an
/// immediate repeat of `avoid` when another option exists.
pub(super) fn pick_next_target(
    harp: &Harmonica,
    stats: &std::collections::HashMap<(u8, Technique), DrillStat>,
    avoid: Option<TrainerTarget>,
    scope: DrillScope,
    custom: &HashSet<(u8, Technique)>,
    selected: TrainerTarget,
) -> Option<TrainerTarget> {
    let mut pool = targets_for_scope(harp, scope, custom, selected);
    if pool.len() > 1 {
        pool.retain(|&t| Some((t.hole, t.technique)) != avoid.map(|a| (a.hole, a.technique)));
    }
    if pool.is_empty() {
        return None;
    }
    let sequence = next_sequence(stats);
    let weights: Vec<f32> = pool
        .iter()
        .map(|t| stats.get(&(t.hole, t.technique)).copied().unwrap_or_default().weight(sequence))
        .collect();
    let total: f32 = weights.iter().sum();
    let mut roll = rand::random_range(0.0..total);
    for (target, w) in pool.iter().zip(weights.iter()) {
        if roll < *w {
            return Some(*target);
        }
        roll -= w;
    }
    pool.last().copied()
}

/// Note name for the current target on `harp`, or `None` if that hole doesn't
/// have that technique (e.g. hole 5 has no bend, most holes have no overblow).
pub(super) fn target_note(harp: &Harmonica, target: TrainerTarget) -> Option<String> {
    let holes = hole_notes(harp, target.hole);
    target.technique.note(&holes).map(str::to_string)
}

/// Frequency in Hz for a note label like `"C#4"`.
pub(super) fn note_freq_hz(note: &str) -> Option<f32> {
    harmonicon_core::midi::note_to_freq_hz(note)
}

/// A short, clean reference tone (fundamental + two soft harmonics) — plain
/// enough to make the *pitch* the whole focus, unlike the full harmonica
/// synth used elsewhere, which is deliberately breathy/textured.
pub(super) fn synth_reference_tone(freq: f32) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 44_100;
    const DUR_SECS: f32 = 1.1;
    let n = (SAMPLE_RATE as f32 * DUR_SECS) as usize;
    let mut buf = vec![0.0f32; n];
    for (i, sample) in buf.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        let attack = (t / 0.02).min(1.0);
        let release = ((DUR_SECS - t) / 0.15).clamp(0.0, 1.0);
        let env = attack.min(release);
        let tau = std::f32::consts::TAU;
        let s = (tau * freq * t).sin()
            + 0.30 * (tau * freq * 2.0 * t).sin()
            + 0.12 * (tau * freq * 3.0 * t).sin();
        *sample = env * 0.28 * s;
    }
    encode_wav(&buf, SAMPLE_RATE)
}

pub(super) fn play_reference_note(
    note: &str,
    sources: &mut Assets<AudioSource>,
    commands: &mut Commands,
) {
    let Some(freq) = note_freq_hz(note) else {
        return;
    };
    let wav = synth_reference_tone(freq);
    let handle = sources.add(AudioSource { bytes: wav.into() });
    commands.spawn((
        AudioPlayer::<AudioSource>(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.6)),
    ));
}

/// How an attempt on the current drill target ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DrillOutcome {
    /// The requested gesture was performed.
    Controlled,
    /// The player attacked the right hole but never completed the gesture.
    Missed,
    /// The attempt timed out with nothing credible heard — the instrument
    /// was down, not the bend wrong. Never lowers accuracy.
    Skipped,
}

/// Decides whether the current attempt is over, and what it was worth.
/// `None` means "still in progress" — the drill must not swap the target
/// out from under a gesture the player is halfway through, which is also
/// what keeps one noisy frame from churning the target: the terminal state
/// is [`GesturePhase::Complete`], not any single in-tune frame.
///
/// Only [`PracticeShape::Free`] advances on a bare hold; every structured
/// shape asks for a whole gesture (settle, travel, hold, release), so its
/// hold is a *stage*, not the finish line.
pub(super) fn drill_outcome(
    shape: PracticeShape,
    phase: GesturePhase,
    hold_secs: f32,
    elapsed_secs: f32,
    attempted: bool,
    settings: &BendingTrainerSettings,
) -> Option<DrillOutcome> {
    let controlled = if shape == PracticeShape::Free {
        hold_secs >= settings.hold_secs
    } else {
        phase == GesturePhase::Complete
    };
    if controlled {
        return Some(DrillOutcome::Controlled);
    }
    if elapsed_secs < settings.timeout_secs {
        return None;
    }
    Some(if attempted { DrillOutcome::Missed } else { DrillOutcome::Skipped })
}

/// Drives the adaptive drill while it's on: waits for the current practice
/// shape to complete on the current target, credits the attempt, and picks
/// the next target — weighted toward whatever the player controls least
/// well, holds least steadily, or hasn't seen for a while. A target that
/// goes `BendingTrainerSettings::timeout_secs` without completing is a miss
/// if the player
/// was audibly trying and a *skip* if nothing was heard at all, so putting
/// the harp down to answer the door costs no progress.
///
/// Runs after `update_gesture_practice`, whose [`GesturePhase`] it reads
/// (ordered in `GameplayPlugin::build`); that system in turn resets the
/// gesture whenever the target changes, so the swap below starts the next
/// attempt clean.
pub fn drill_update(
    key: Res<TrainerKey>,
    mut target: ResMut<TrainerTarget>,
    active: Res<ActivePitches>,
    trace: Res<BendTrace>,
    practice: Res<GesturePractice>,
    settings: Res<BendingTrainerSettings>,
    mut drill: ResMut<DrillState>,
    time: Res<Time>,
) {
    if !drill.enabled {
        return;
    }
    let harp = key.harp();
    if target_note(harp, *target).is_none() {
        return;
    }
    let dt = time.delta_secs();
    // The per-attempt timers move every frame but nothing displays them, so
    // they bypass change detection: every drill label, the Skip/intro
    // drawers and the progress line gate on `DrillState::is_changed()` to
    // mean "the drill's visible state moved", which `finish_attempt` below
    // still triggers through an ordinary mutable borrow.
    let timers = drill.bypass_change_detection();
    timers.elapsed_secs += dt;

    let shift = reference_shift_cents(&settings, key.name(), target.hole);
    let observation = tuner_observation(harp, *target, &active, shift);
    if matches!(observation, Some(TunerObservation::TargetFamily(_))) {
        timers.attempted = true;
    }
    let in_tune = !trace.unstable
        && matches!(
            observation,
            Some(TunerObservation::TargetFamily(cents)) if cents.abs() <= settings.tolerance_cents
        );
    timers.hold_secs = if in_tune { timers.hold_secs + dt } else { 0.0 };

    let Some(outcome) = drill_outcome(
        practice.shape,
        practice.phase,
        drill.hold_secs,
        drill.elapsed_secs,
        drill.attempted,
        &settings,
    ) else {
        return;
    };

    finish_attempt(&mut drill, &mut target, harp, outcome, trace.stability_cents);
}

/// Records how the current attempt ended and serves the next target. The
/// one path both an automatic ending ([`drill_update`]) and a player's own
/// Skip ([`skip_drill_target`]) go through, so the bookkeeping — streak,
/// evidence, the next pick, the per-attempt reset — can't differ between
/// them.
pub(super) fn finish_attempt(
    drill: &mut DrillState,
    target: &mut TrainerTarget,
    harp: &Harmonica,
    outcome: DrillOutcome,
    stability: Option<f32>,
) {
    let sequence = next_sequence(&drill.stats);
    let stat = drill.stats.entry((target.hole, target.technique)).or_default();
    match outcome {
        DrillOutcome::Controlled => {
            stat.record_attempt(true, stability, sequence);
            drill.streak += 1;
        }
        DrillOutcome::Missed => {
            stat.record_attempt(false, stability, sequence);
            drill.streak = 0;
        }
        DrillOutcome::Skipped => {
            stat.record_skip(sequence);
            drill.streak = 0;
        }
    }
    if let Some(next) =
        pick_next_target(harp, &drill.stats, Some(*target), drill.scope, &drill.custom, *target)
    {
        *target = next;
    }
    drill.hold_secs = 0.0;
    drill.elapsed_secs = 0.0;
    drill.attempted = false;
}

/// Starts or stops the drill. Starting serves a target from the current
/// scope straight away rather than drilling whatever happened to be
/// selected, which may not be in scope at all.
pub fn toggle_drill(
    _: On<Activate>,
    key: Res<TrainerKey>,
    mut target: ResMut<TrainerTarget>,
    mut drill: ResMut<DrillState>,
) {
    drill.enabled = !drill.enabled;
    drill.hold_secs = 0.0;
    drill.elapsed_secs = 0.0;
    drill.attempted = false;
    if drill.enabled
        && let Some(next) = pick_next_target(
            key.harp(),
            &drill.stats,
            Some(*target),
            drill.scope,
            &drill.custom,
            *target,
        )
    {
        *target = next;
    }
}

/// The player's own "not this one" — ends the attempt as a skip and serves
/// the next target.
///
/// Always a skip, even after the player has been audibly trying: choosing
/// to move on is not evidence of failing to bend, and counting it as a miss
/// would make Skip a button that punishes the player for using it. The
/// timeout is what records a genuine unfinished attempt as a miss.
pub fn skip_drill_target(
    _: On<Activate>,
    key: Res<TrainerKey>,
    mut target: ResMut<TrainerTarget>,
    mut drill: ResMut<DrillState>,
) {
    if !drill.enabled {
        return;
    }
    finish_attempt(&mut drill, &mut target, key.harp(), DrillOutcome::Skipped, None);
}

/// Persists the session's drill hit-rates to `profile.json` on the way out
/// of the trainer — paired with `setup`'s restore on the way in. Saved once
/// per visit rather than on every drill-target completion, the same
/// "meaningful lifecycle boundary" policy `results::setup` uses for song
/// bests (see `profile.rs`'s module doc comment).
pub fn save_drill_progress(drill: Res<DrillState>, mut profile: ResMut<PlayerProfile>) {
    profile.drills = stats_to_profile(&drill.stats);
    harmonicon_app::profile::save_profile(&profile);
}

/// Keeps the "Drill: ..." readout in step with on/off state and streak.
pub fn update_drill_label(
    drill: Res<DrillState>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<DrillLabel>>,
) {
    if !drill.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(String::from(if drill.enabled {
            loc.msg_args("bending-drill-on", &[("streak", drill.streak.to_string())])
        } else {
            loc.msg("bending-drill-off")
        }));
    }
}

/// A plain dark amber, distinct from every other button's resting
/// [`button::color_default`], for the Drill button while the drill is
/// running.
pub(super) const DRILL_ACTIVE_COLOR: Color = Color::srgb(0.45, 0.35, 0.10);

/// Highlights the Drill toggle button itself while the drill is running,
/// alongside [`update_drill_label`]'s text — a toggle should visibly look
/// pressed, not only say so in small text next to it. Writes
/// [`BaseButtonColor`], not `BackgroundColor` directly: `dialogs::button`'s
/// own hover/press layer owns `BackgroundColor` and would fight a system
/// that wrote it here (see `BaseButtonColor`'s doc comment).
pub fn update_drill_button_visual(
    drill: Res<DrillState>,
    mut buttons: Query<&mut BaseButtonColor, With<DrillToggleButton>>,
) {
    if !drill.is_changed() {
        return;
    }
    let color = if drill.enabled { DRILL_ACTIVE_COLOR } else { button::color_default() };
    for mut base in &mut buttons {
        base.0 = color;
    }
}
