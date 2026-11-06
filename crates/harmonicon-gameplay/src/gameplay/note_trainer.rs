// SPDX-License-Identifier: MIT

//! The Note Trainer for a guitar or a flute: the drills
//! `harmonicon_core::note_trainer` lists, one at a time, against a live
//! tuner.
//!
//! The kinds are the instrument's own `techniques()`, chosen from a tab
//! bar. A guitar: notes to hit and hold, bends to push up to pitch, natural
//! harmonics, and a note to find on one particular string. A flute: notes,
//! octaves (one fingering overblown to the note above), and alternate
//! fingerings. A drill is landed by holding its pitch within the trainer's
//! tolerance for its hold time (both from `BendingTrainerSettings`, so the
//! trainers judge alike); the next drill follows on its own. The guitar's
//! fretboard strip or the flute's horizontal drawing, both from the play
//! highway, shows where the fingers go.
//!
//! Progress is kept in the profile per instrument and kind
//! (`profile::NoteTrainerRecord`): how many drills have been landed, and
//! which drill to resume on. A landing saves at once; leaving the trainer
//! saves where the player was.
//!
//! What it can't check: detection hears a pitch, not a string or a
//! fingering, so a position drill trusts the player to play it as asked.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use harmonicon_audio::AudioSettings;

use harmonicon_app::app::AppState;
use harmonicon_audio::pitch_detect::{
    PITCH_RANGE_MARGIN_SEMITONES, PitchEvent, PitchInfo, PitchRange,
};
use harmonicon_core::instrument::guitar::{FretAssignment, Guitar, GuitarBody, StringTechnique};
use harmonicon_core::instrument::{Instrument, InstrumentModel, PlayPosition, TechniqueKind};
use harmonicon_core::midi::{midi_to_freq_hz, midi_to_note};
use harmonicon_core::note_trainer::{Drill, Heard, drills, hear};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::BendingTrainerSettings;
use harmonicon_ui::dialogs::button;
use harmonicon_ui::dialogs::page_chrome::{header_scene, spawn_back_button, title_column_scene};
use harmonicon_ui::dialogs::tab_bar::{TabSelect, spawn_tab_bar};

use super::GameplayRoot;
use super::flute_visuals::{FluteDrawing, FluteKey, paint_flute_keys};
mod bend;
mod chords;
mod clarinet;
pub(super) use chords::update as update_chords;
pub(super) use clarinet::update as update_clarinet;
mod flute_picker;
mod flute_setup;
pub(super) use flute_picker::update as update_flute_picker;
mod layout;
mod piano;
mod selection;
pub(super) use bend::update as update_bend;
pub(super) use layout::setup;
pub(super) use selection::sync_controls;
pub(super) mod teaching;
use super::guitar_visuals::spawn_trainer_neck;
use harmonicon_core::instrument::flute::Flute;
pub(super) use piano::update_hands;
pub(super) use teaching::sync_score;

/// Seconds the "Landed!" line stays up before the live status returns.
const LANDED_SECS: f32 = 1.2;

/// One kind's drills and where the player is in them.
#[derive(Clone, Debug, Default)]
pub struct Practice {
    drills: Vec<Drill>,
    index: usize,
    /// Seconds held on target, within tolerance, without a break.
    held: f32,
    heard: Option<Heard>,
    /// What was heard instead, when it was elsewhere — named in the status.
    heard_midi: Option<f32>,
    /// Drills landed on this kind, ever: the profile's count plus this
    /// session's.
    landed: u32,
    /// Counts down from [`LANDED_SECS`] after a landing.
    celebrate: f32,
    /// Whether the profile's record has been applied yet
    /// (`restore_progress`).
    restored: bool,
    revealed: bool,
    auto_advance: bool,
    awaiting_release: bool,
    started: bool,
    start_held: f32,
    silence_secs: f32,
    trace_tick: f32,
    selection_notice: Option<(&'static str, f32)>,
    trace: std::collections::VecDeque<Option<f32>>,
}

impl Practice {
    pub fn new(drills: Vec<Drill>) -> Self {
        Self { drills, auto_advance: true, ..default() }
    }

    pub fn drill(&self) -> Option<&Drill> {
        self.drills.get(self.index)
    }

    /// Moves `delta` drills along, wrapping, and starts the hold over.
    pub fn step_by(&mut self, delta: isize) {
        if self.drills.is_empty() {
            return;
        }
        let len = self.drills.len() as isize;
        self.index = (self.index as isize + delta).rem_euclid(len) as usize;
        self.held = 0.0;
        self.heard = None;
        self.heard_midi = None;
        self.revealed = false;
        self.awaiting_release = false;
        self.started = false;
        self.start_held = 0.0;
        self.silence_secs = 0.0;
        self.trace.clear();
        self.trace_tick = 0.0;
        self.selection_notice = None;
    }

    /// Feeds one frame: the pitch heard nearest the target (`None` in
    /// silence) and how long the frame was. Landing — `hold_secs` on target
    /// within `tolerance_cents` — counts it and moves to the next drill.
    /// Returns whether this frame landed one.
    pub fn listen(
        &mut self,
        midi: Option<f32>,
        dt: f32,
        tolerance_cents: f32,
        hold_secs: f32,
    ) -> bool {
        self.celebrate = (self.celebrate - dt).max(0.0);
        if let Some((key, remaining)) = self.selection_notice {
            self.selection_notice = (remaining > dt).then_some((key, remaining - dt));
        }
        let Some(drill) = self.drill().copied() else { return false };
        self.heard = midi.map(|m| hear(&drill, m));
        self.heard_midi = midi;
        self.silence_secs = if midi.is_none() { self.silence_secs + dt } else { 0.0 };
        if self.silence_secs >= 0.2 {
            self.started = false;
            self.start_held = 0.0;
        }
        if drill.kind == TechniqueKind::Glide
            && let Some(start) = drill.start
        {
            self.trace_tick += dt;
            if self.trace_tick >= 0.05 {
                self.trace_tick = 0.0;
                let point = midi
                    .map(|m| (m - start) / (drill.target - start))
                    .filter(|v| (-0.5..=1.5).contains(v));
                self.trace.push_back(point);
                if self.trace.len() > 80 {
                    self.trace.pop_front();
                }
            }
        }
        if self.awaiting_release {
            if !matches!(self.heard, Some(Heard::Target { cents }) if cents.abs() <= tolerance_cents)
            {
                self.awaiting_release = false;
                self.started = false;
                self.start_held = 0.0;
            }
            return false;
        }
        if drill.kind == TechniqueKind::Glide && !self.started {
            let at_start = drill
                .start
                .zip(midi)
                .is_some_and(|(start, m)| (m - start).abs() * 100.0 <= tolerance_cents);
            self.start_held = if at_start { self.start_held + dt } else { 0.0 };
            self.started = self.start_held >= 0.15;
            self.held = 0.0;
            return false;
        }
        match self.heard {
            Some(Heard::Target { cents }) if cents.abs() <= tolerance_cents => self.held += dt,
            _ => self.held = 0.0,
        }
        if self.held < hold_secs {
            return false;
        }
        self.landed += 1;
        self.celebrate = LANDED_SECS;
        if self.auto_advance {
            self.step_by(1);
        } else {
            self.held = 0.0;
            self.awaiting_release = true;
        }
        true
    }

    /// Picks up a saved session: its landed count and the drill it was on
    /// (wrapped, in case the list has since changed length).
    pub fn resume(&mut self, landed: u32, position: u32) {
        self.landed = landed;
        if !self.drills.is_empty() {
            self.index = position as usize % self.drills.len();
        }
        self.held = 0.0;
    }

    /// What the profile keeps of this practice.
    fn record(&self) -> harmonicon_app::profile::NoteTrainerRecord {
        harmonicon_app::profile::NoteTrainerRecord {
            landed: self.landed,
            position: u32::try_from(self.index).unwrap_or(0),
        }
    }

    /// How far through the hold the player is, 0 to 1.
    pub fn hold_fraction(&self, hold_secs: f32) -> f32 {
        if self.awaiting_release || hold_secs <= 0.0 {
            1.0
        } else {
            (self.held / hold_secs).clamp(0.0, 1.0)
        }
    }
}

/// The trainer's state for the session: the guitar, the kind on show and
/// its practice. Kept across visits, so leaving and returning resumes.
#[derive(Resource)]
pub struct NoteTrainer {
    instrument: Instrument,
    kind: TechniqueKind,
    practice: Practice,
    chord: Option<chords::ChordPractice>,
    reference_remaining: f32,
}

impl Default for NoteTrainer {
    fn default() -> Self {
        Self::for_instrument(Guitar::standard(GuitarBody::Electric).into())
    }
}

impl NoteTrainer {
    /// A fresh session on `instrument`, on its first kind of drill.
    pub fn for_instrument(instrument: Instrument) -> Self {
        let kind = instrument.techniques().first().copied().unwrap_or(TechniqueKind::Target);
        let mut practice = Practice::new(drills(&instrument, kind));
        practice.auto_advance = !matches!(instrument, Instrument::Fretted(_));
        Self { instrument, kind, practice, chord: None, reference_remaining: 0.0 }
    }

    /// Switches to `instrument`, keeping the session when it is the same
    /// one — so leaving and coming back resumes where the player was.
    pub fn choose(&mut self, instrument: Instrument) {
        if self.instrument != instrument {
            *self = Self::for_instrument(instrument);
        }
    }

    pub(super) fn guitar_targets(&self) -> Vec<FretAssignment> {
        if let Some(chord) = &self.chord {
            return chord.shape().positions();
        }
        if self.practice.revealed {
            match self.practice.drill().map(|d| d.position) {
                Some(PlayPosition::Fret(at)) => vec![at],
                _ => Vec::new(),
            }
        } else {
            lit(self.practice.drill())
        }
    }

    fn offers_chords(&self) -> bool {
        self.guitar().and_then(chords::shapes_for).is_some()
    }

    fn step_by(&mut self, delta: isize) {
        if let Some(chord) = &mut self.chord {
            chord.step_by(delta);
        } else {
            self.practice.step_by(delta);
        }
    }

    fn guitar(&self) -> Option<&Guitar> {
        match &self.instrument {
            Instrument::Fretted(guitar) => Some(guitar),
            Instrument::Harmonica(_)
            | Instrument::Flute(_)
            | Instrument::Clarinet(_)
            | Instrument::Piano(_) => None,
        }
    }

    fn piano(&self) -> Option<&harmonicon_core::instrument::piano::Piano> {
        match &self.instrument {
            Instrument::Piano(piano) => Some(piano),
            _ => None,
        }
    }

    fn flute(&self) -> Option<&Flute> {
        match &self.instrument {
            Instrument::Flute(flute) => Some(flute),
            Instrument::Harmonica(_)
            | Instrument::Fretted(_)
            | Instrument::Clarinet(_)
            | Instrument::Piano(_) => None,
        }
    }

    /// Where the profile keeps this instrument's progress on its current
    /// kind. `None` for a harmonica, which keeps the Bending Trainer's.
    fn record_key(&self) -> Option<String> {
        let instrument = match &self.instrument {
            Instrument::Fretted(setup) => setup.kind.instrument_kind().slug(),
            Instrument::Flute(_) => "flute",
            Instrument::Clarinet(_) => "clarinet",
            Instrument::Piano(_) => "piano",
            Instrument::Harmonica(_) => return None,
        };
        Some(harmonicon_app::profile::note_trainer_key(instrument, kind_id(self.kind)))
    }

    /// Writes this kind's progress into `profile`. Returns whether there was
    /// anything to write.
    fn remember(&self, profile: &mut harmonicon_app::profile::PlayerProfile) -> bool {
        let Some(key) = self.record_key().filter(|_| self.practice.restored) else {
            return false;
        };
        profile.note_trainer.insert(key, self.practice.record());
        true
    }

    /// The kinds the tab bar offers, in its order: the instrument's own.
    fn kinds(&self) -> &'static [TechniqueKind] {
        self.instrument.techniques()
    }

    /// Shows `kind`, after keeping the one being left in `profile`.
    /// Returns whether `profile` changed, for the caller to save.
    fn set_kind(
        &mut self,
        kind: TechniqueKind,
        profile: &mut harmonicon_app::profile::PlayerProfile,
    ) -> bool {
        let leaving_chord = self.chord.take().is_some();
        if self.kind == kind {
            if leaving_chord {
                self.practice.step_by(0);
            }
            return false;
        }
        let remembered = self.remember(profile);
        self.kind = kind;
        let auto_advance = self.practice.auto_advance;
        self.practice = Practice::new(drills(&self.instrument, kind));
        self.practice.auto_advance = auto_advance;
        remembered
    }
}

/// A kind's name in the profile's keys. Stable: changing one loses the
/// progress saved under it.
fn kind_id(kind: TechniqueKind) -> &'static str {
    match kind {
        TechniqueKind::Target => "target",
        TechniqueKind::Glide => "glide",
        TechniqueKind::Register => "register",
        TechniqueKind::Position => "position",
    }
}

/// Applies the profile's record for the kind on show, the first frame it
/// is shown.
pub(super) fn restore_progress(
    mut trainer: ResMut<NoteTrainer>,
    profile: Res<harmonicon_app::profile::PlayerProfile>,
) {
    if trainer.practice.restored {
        return;
    }
    let record = trainer.record_key().and_then(|key| profile.note_trainer.get(&key).copied());
    if let Some(record) = record {
        trainer.practice.resume(record.landed, record.position);
    }
    trainer.practice.restored = true;
}

/// Keeps where the player was, on leaving the trainer.
pub(super) fn save_progress(
    trainer: Res<NoteTrainer>,
    mut profile: ResMut<harmonicon_app::profile::PlayerProfile>,
) {
    if trainer.remember(&mut profile) {
        harmonicon_app::profile::save_profile(&profile);
    }
}

/// The pitch, as a fractional MIDI number against `a4_hz`, of whichever
/// detected note sits nearest `target`. Detection can report several at
/// once (a ringing open string under the fretted one); the drill only
/// cares about the one the player is aiming at.
pub(super) fn nearest_midi(pitches: &[PitchInfo], target: f32, a4_hz: f32) -> Option<f32> {
    pitches
        .iter()
        .filter(|p| p.frequency > 0.0)
        .map(|p| 69.0 + 12.0 * (p.frequency / a4_hz).log2())
        .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
}

fn note_name(midi: f32) -> String {
    midi_to_note(midi.round() as i32)
}

/// The localized name of a kind, as the tab bar shows it: a flute's
/// register jump is its octaves, and its positions its alternate
/// fingerings.
fn kind_key(instrument: &Instrument, kind: TechniqueKind) -> &'static str {
    let flute = matches!(instrument, Instrument::Flute(_));
    match kind {
        TechniqueKind::Target => "note-trainer-kind-target",
        TechniqueKind::Glide => "note-trainer-kind-glide",
        TechniqueKind::Register if matches!(instrument, Instrument::Clarinet(_)) => {
            "clarinet-register-tab"
        }
        TechniqueKind::Register if flute => "note-trainer-kind-register-flute",
        TechniqueKind::Register => "note-trainer-kind-register",
        TechniqueKind::Position if instrument.fingering_chart().is_some() => {
            "note-trainer-kind-position-flute"
        }
        TechniqueKind::Position => "note-trainer-kind-position",
    }
}

/// What to play, in words, on whichever instrument the trainer has.
fn instruction(loc: &Localization, instrument: &Instrument, drill: &Drill) -> String {
    match instrument {
        Instrument::Fretted(guitar) => guitar_instruction(loc, guitar, drill),
        Instrument::Flute(_) => flute_instruction(loc, drill),
        Instrument::Clarinet(clarinet) => {
            let written = |pitch: f32| {
                midi_to_note(i32::from(
                    clarinet.notation().transposition.written_semitone(pitch.round() as u8),
                ))
            };
            let note = written(drill.target);
            if let Some(start) = drill.start {
                loc.msg_args(
                    "clarinet-register-instruction",
                    &[("start", written(start)), ("note", note)],
                )
                .to_string()
            } else {
                let alternate = match drill.position {
                    PlayPosition::Fingering(at) => at.alternate + 1,
                    _ => 1,
                };
                loc.msg_args(
                    "clarinet-target-instruction",
                    &[("note", note), ("fingering", alternate.to_string())],
                )
                .to_string()
            }
        }
        // A key is its note: there is nothing to say but which.
        Instrument::Piano(_) => String::from(
            loc.msg_args("note-trainer-flute-target", &[("note", note_name(drill.target))]),
        ),
        Instrument::Harmonica(_) => String::new(),
    }
}

/// A flute drill in words: the note; for an octave, the note it is
/// overblown from on the same keys; for a position, which fingering.
fn flute_instruction(loc: &Localization, drill: &Drill) -> String {
    let PlayPosition::Fingering(at) = drill.position else { return String::new() };
    let note = note_name(drill.target);
    let start = drill.start.map(note_name).unwrap_or_default();
    match (drill.kind, at.alternate) {
        (TechniqueKind::Register, _) => String::from(
            loc.msg_args("note-trainer-flute-register", &[("note", note), ("start", start)]),
        ),
        (TechniqueKind::Position, 0) => {
            String::from(loc.msg_args("note-trainer-flute-position-standard", &[("note", note)]))
        }
        (TechniqueKind::Position, alternate) => String::from(loc.msg_args(
            "note-trainer-flute-position",
            &[("note", note), ("alternate", alternate.to_string())],
        )),
        _ => String::from(loc.msg_args("note-trainer-flute-target", &[("note", note)])),
    }
}

/// What to play on a guitar: string, fret and what to do there. A position
/// drill names only the string — finding the fret is the drill.
fn guitar_instruction(loc: &Localization, guitar: &Guitar, drill: &Drill) -> String {
    let PlayPosition::Fret(at) = drill.position else { return String::new() };
    let open = guitar.string_labels().get(usize::from(at.string - 1)).cloned().unwrap_or_default();
    let args = [
        ("string", at.string.to_string()),
        ("open", open),
        ("fret", at.fret.to_string()),
        ("note", note_name(drill.target)),
        ("start", drill.start.map(note_name).unwrap_or_default()),
    ];
    let key = match (drill.kind, at.technique) {
        (TechniqueKind::Glide, StringTechnique::Bend(depth)) if depth < 1.5 => {
            "note-trainer-glide-half"
        }
        (TechniqueKind::Glide, _) => "note-trainer-glide-whole",
        (TechniqueKind::Register, _) => "note-trainer-register",
        (TechniqueKind::Position, _) => "note-trainer-position",
        (TechniqueKind::Target, _) => "note-trainer-target",
    };
    String::from(loc.msg_args(key, &args))
}

/// The live line under the instruction: what the tuner hears against the
/// drill. A landing outranks it for a moment.
fn status(loc: &Localization, practice: &Practice, tolerance_cents: f32) -> String {
    status_with_names(loc, practice, tolerance_cents, note_name)
}

fn status_with_names(
    loc: &Localization,
    practice: &Practice,
    tolerance_cents: f32,
    name: impl Fn(f32) -> String,
) -> String {
    if let Some((key, _)) = practice.selection_notice {
        return loc.msg(key).to_string();
    }
    if let Some(drill) = practice.drill().filter(|d| d.kind == TechniqueKind::Glide) {
        if !practice.started && !practice.awaiting_release {
            return loc
                .msg_args(
                    "trainer-bend-start-first",
                    &[("start", drill.start.map(&name).unwrap_or_default())],
                )
                .to_string();
        }
        if practice
            .heard_midi
            .is_some_and(|m| m > drill.target + tolerance_cents / 100.0 && m < drill.target + 3.0)
        {
            return loc
                .msg_args(
                    "trainer-bend-too-high",
                    &[(
                        "cents",
                        format!("{:+.0}", (practice.heard_midi.unwrap() - drill.target) * 100.0),
                    )],
                )
                .to_string();
        }
    }
    if practice.awaiting_release {
        return loc.msg("trainer-release-repeat").to_string();
    }
    if practice.celebrate > 0.0 {
        let next = practice.drill().map(|d| name(d.target)).unwrap_or_default();
        return String::from(loc.msg_args("note-trainer-landed", &[("note", next)]));
    }
    let Some(drill) = practice.drill() else { return String::new() };
    let cents = |c: f32| format!("{c:+.0}");
    let msg = match practice.heard {
        None => loc.msg("note-trainer-listening"),
        Some(Heard::Start) => loc.msg("note-trainer-start"),
        Some(Heard::Gliding { progress }) => {
            loc.msg_args("note-trainer-gliding", &[("percent", format!("{:.0}", progress * 100.0))])
        }
        Some(Heard::Target { cents: c }) if c.abs() <= tolerance_cents => {
            loc.msg_args("note-trainer-holding", &[("cents", cents(c))])
        }
        Some(Heard::Target { cents: c }) => {
            loc.msg_args("note-trainer-close", &[("cents", cents(c))])
        }
        Some(Heard::Elsewhere) => loc.msg_args(
            "note-trainer-elsewhere",
            &[
                ("heard", practice.heard_midi.map(&name).unwrap_or_default()),
                ("note", name(drill.target)),
            ],
        ),
    };
    String::from(msg)
}

/// Where the fretboard lights: the drill's own fret — for a harmonic, the
/// fret to touch over. Nothing for a position drill, whose fret is the
/// thing to find.
fn lit(drill: Option<&Drill>) -> Vec<FretAssignment> {
    match drill {
        Some(Drill { kind: TechniqueKind::Position, .. }) | None => Vec::new(),
        Some(Drill { position: PlayPosition::Fret(at), .. }) => vec![*at],
        Some(_) => Vec::new(),
    }
}

#[derive(Component)]
pub struct InstructionLabel;
#[derive(Component)]
pub struct StatusLabel;
#[derive(Component)]
pub struct HoldBar;
#[derive(Component)]
pub struct LandedLabel;

/// Sounds the current drill's target in the instrument's own voice, at the
/// trainer's A4 reference.
fn play_reference(
    _: On<Activate>,
    mut trainer: ResMut<NoteTrainer>,
    settings: Res<BendingTrainerSettings>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
) {
    use harmonicon_core::synth::{Expr, PhraseNote, SAMPLE_RATE, render_pcm_with};
    let targets: Vec<f32> = if let (Some(chord), Some(guitar)) = (&trainer.chord, trainer.guitar())
    {
        chord.shape().pitches(guitar).into_iter().map(f32::from).collect()
    } else {
        trainer.practice.drill().map(|d| vec![d.target]).unwrap_or_default()
    };
    let notes: Vec<PhraseNote> = targets
        .into_iter()
        .map(|target| PhraseNote {
            tick: 0,
            len: 1,
            freq: Some(settings.a4_hz * 2f32.powf((target - 69.0) / 12.0)),
            expr: Expr::None,
        })
        .collect();
    let pcm = render_pcm_with(&notes, 1.5, trainer.instrument.synth_voice());
    let wav = harmonicon_core::wav::encode_wav(&pcm, SAMPLE_RATE);
    trainer.reference_remaining = 1.8;
    commands.spawn((
        AudioPlayer::<AudioSource>(sources.add(AudioSource { bytes: wav.into() })),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.6)),
    ));
}

/// Feeds the newest detection to the practice.
pub(super) fn listen(
    mut reader: MessageReader<PitchEvent>,
    mut trainer: ResMut<NoteTrainer>,
    settings: Res<BendingTrainerSettings>,
    audio: Res<AudioSettings>,
    time: Res<Time>,
    mut last: Local<Vec<PitchInfo>>,
    mut last_at: Local<f64>,
    mut profile: ResMut<harmonicon_app::profile::PlayerProfile>,
) {
    if let Some(ev) = reader.read().last() {
        last.clone_from(&ev.0);
        *last_at = time.elapsed_secs_f64();
    }
    if time.elapsed_secs_f64() - *last_at > 0.25 {
        last.clear();
    }
    trainer.reference_remaining = (trainer.reference_remaining - time.delta_secs()).max(0.0);
    if trainer.reference_remaining > 0.0 {
        trainer.practice.held = 0.0;
        trainer.practice.heard = None;
        trainer.practice.heard_midi = None;
        trainer.practice.started = false;
        trainer.practice.start_held = 0.0;
        if let Some(chord) = &mut trainer.chord {
            chord.held = 0.0;
        }
        last.clear();
        return;
    }
    if let Some(guitar) = trainer.guitar().cloned() {
        let auto = trainer.practice.auto_advance;
        if let Some(chord) = &mut trainer.chord {
            let expected = chord.shape().pitches(&guitar);
            let heard: Vec<f32> = last
                .iter()
                .filter(|p| p.frequency.is_finite() && p.frequency > 0.0)
                .map(|p| 69.0 + 12.0 * (p.frequency / settings.a4_hz).log2())
                .collect();
            if chord.listen(
                &expected,
                &heard,
                time.delta_secs(),
                settings.tolerance_cents,
                settings.hold_secs,
                audio.pitch_algorithm.is_polyphonic(),
            ) && auto
            {
                chord.step_by(1);
                chord.awaiting_release = true;
            }
            return;
        }
    }
    let Some(target) = trainer.practice.drill().map(|d| d.target) else { return };
    let midi = nearest_midi(&last, target, settings.a4_hz);
    let landed = trainer.practice.listen(
        midi,
        time.delta_secs(),
        settings.tolerance_cents,
        settings.hold_secs,
    );
    // A landing is rare and worth keeping at once, as the Bending Trainer
    // keeps a drill's record.
    if landed && trainer.remember(&mut profile) {
        harmonicon_app::profile::save_profile(&profile);
    }
}

pub(super) fn update_labels(
    trainer: Res<NoteTrainer>,
    settings: Res<BendingTrainerSettings>,
    loc: Res<Localization>,
    audio: Res<AudioSettings>,
    mut texts: ParamSet<(
        Query<&mut Text, With<InstructionLabel>>,
        Query<&mut Text, With<StatusLabel>>,
        Query<&mut Text, With<LandedLabel>>,
    )>,
    mut bars: Query<&mut Node, With<HoldBar>>,
) {
    let practice = &trainer.practice;
    let set = |text: &mut Text, value: String| {
        if text.0 != value {
            text.0 = value;
        }
    };
    let what = if let Some(chord) = &trainer.chord {
        loc.msg_args("trainer-chord-play", &[("chord", chord.shape().name.to_string())]).to_string()
    } else {
        practice.drill().map(|d| instruction(&loc, &trainer.instrument, d)).unwrap_or_default()
    };
    for mut text in &mut texts.p0() {
        set(&mut text, what.clone());
    }
    let now = if trainer.reference_remaining > 0.0 {
        loc.msg("trainer-reference-playing").to_string()
    } else if let (Some(chord), Some(guitar)) = (&trainer.chord, trainer.guitar()) {
        chord.status(guitar, &audio, &loc)
    } else {
        let shift = -f32::from(trainer.instrument.notation().transposition.semitones);
        if matches!(trainer.instrument, Instrument::Clarinet(_)) {
            status_with_names(&loc, practice, settings.tolerance_cents, |pitch| {
                note_name(pitch + shift)
            })
        } else {
            status(&loc, practice, settings.tolerance_cents)
        }
    };
    for mut text in &mut texts.p1() {
        set(&mut text, now.clone());
    }
    let count = if let Some(chord) = &trainer.chord {
        loc.msg_args("trainer-chord-count", &[("count", chord.landed.to_string())]).to_string()
    } else {
        String::from(loc.msg_args(
            "note-trainer-count",
            &[
                ("landed", practice.landed.to_string()),
                ("index", (practice.index + 1).to_string()),
                ("total", practice.drills.len().to_string()),
            ],
        ))
    };
    for mut text in &mut texts.p2() {
        set(&mut text, count.clone());
    }
    let fraction = if let Some(chord) = &trainer.chord {
        if chord.awaiting_release {
            1.0
        } else {
            (chord.held / settings.hold_secs.max(0.01)).clamp(0.0, 1.0)
        }
    } else {
        practice.hold_fraction(settings.hold_secs)
    };
    let width = Val::Percent(fraction * 100.0);
    for mut node in &mut bars {
        if node.width != width {
            node.width = width;
        }
    }
}

#[derive(Component)]
pub struct PositionHint;

pub(super) fn update_position_hint(
    trainer: Res<NoteTrainer>,
    mut hints: Query<&mut harmonicon_ui::dialogs::drawer::Drawer, With<PositionHint>>,
) {
    for mut hint in &mut hints {
        hint.open = trainer.chord.is_none()
            && trainer.kind == TechniqueKind::Position
            && !trainer.practice.revealed;
    }
}

/// Lights the key the drill asks for on the piano's keyboard.
pub(super) fn update_piano_keys(
    trainer: Res<NoteTrainer>,
    keyboards: Query<Entity, With<harmonicon_ui::dialogs::piano_keys::PianoKeyboard>>,
    mut keys: Query<(&harmonicon_ui::dialogs::piano_keys::PianoKey, &mut BackgroundColor)>,
) {
    let target = trainer.practice.drill().and_then(|d| match d.position {
        PlayPosition::Key(key) => Some(key),
        _ => None,
    });
    for keyboard in &keyboards {
        harmonicon_ui::dialogs::piano_keys::paint_piano_keys(keyboard, &mut keys, |midi| {
            if Some(midi) == target {
                Some(Color::srgb(0.98, 0.80, 0.25))
            } else if trainer.practice.heard_midi.is_some_and(|heard| heard.round() as u8 == midi) {
                Some(Color::srgb(0.3, 0.65, 1.0))
            } else {
                None
            }
        });
    }
}

/// Lights the flute drawing's keys for the drill's fingering. A position
/// drill shows the alternate it asks for, which is the thing to learn.
pub(super) fn update_flute_drawing(
    trainer: Res<NoteTrainer>,
    drawing: Query<&FluteDrawing>,
    mut keys: Query<(&FluteKey, &mut BackgroundColor)>,
) {
    let Ok(FluteDrawing(flute)) = drawing.single() else { return };
    let now = trainer.practice.drill().and_then(|d| match d.position {
        PlayPosition::Fingering(at) => Some(at),
        _ => None,
    });
    paint_flute_keys(flute, now, None, &mut keys);
}

/// Esc returns to the Play menu, as the Back button does.
pub(super) fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    mut ret_play: ResMut<harmonicon_app::app::ReturnToPlay>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        ret_play.0 = true;
        next.set(AppState::Menu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn practice() -> Practice {
        let guitar: Instrument = Guitar::standard(GuitarBody::Electric).into();
        Practice::new(drills(&guitar, TechniqueKind::Target))
    }

    fn pitch(freq: f32) -> PitchInfo {
        PitchInfo { midi: 0, note: String::new(), octave: 0, frequency: freq }
    }

    #[test]
    fn holding_the_target_lands_it_and_moves_on() {
        let mut p = practice();
        let first = *p.drill().unwrap();
        assert_eq!(first.target, 64.0, "the open high E string comes first");
        for _ in 0..5 {
            assert!(!p.listen(Some(64.05), 0.1, 20.0, 0.6));
        }
        assert!((p.hold_fraction(0.6) - 5.0 / 6.0).abs() < 0.01);
        assert!(p.listen(Some(64.05), 0.1, 20.0, 0.6));
        assert_eq!(p.landed, 1);
        assert_eq!(p.drill().unwrap().target, 65.0, "then its first fret");
        assert_eq!(p.hold_fraction(0.6), 0.0);
    }

    #[test]
    fn a_break_or_an_out_of_tune_frame_starts_the_hold_over() {
        let mut p = practice();
        p.listen(Some(64.0), 0.4, 20.0, 0.6);
        p.listen(None, 0.1, 20.0, 0.6);
        assert_eq!(p.hold_fraction(0.6), 0.0, "silence");
        p.listen(Some(64.0), 0.4, 20.0, 0.6);
        p.listen(Some(64.3), 0.1, 20.0, 0.6);
        assert_eq!(p.hold_fraction(0.6), 0.0, "30 cents sharp is close, not held");
        assert!(matches!(p.heard, Some(Heard::Target { .. })));
        assert_eq!(p.landed, 0);
    }

    #[test]
    fn stepping_wraps_both_ways() {
        let mut p = practice();
        let count = p.drills.len() as isize;
        p.step_by(-1);
        assert_eq!(p.index as isize, count - 1);
        p.step_by(1);
        assert_eq!(p.index, 0);
    }

    /// An open string ringing under the fretted note mustn't hide it.
    #[test]
    fn the_pitch_nearest_the_target_is_the_one_heard() {
        let heard = [pitch(82.41), pitch(329.63), pitch(0.0)];
        let midi = nearest_midi(&heard, 64.0, 440.0).unwrap();
        assert!((midi - 64.0).abs() < 0.01, "E4 over E2, got {midi}");
        let flat_reference = nearest_midi(&[pitch(329.63)], 64.0, 444.0).unwrap();
        assert!(flat_reference < 64.0, "against A4 = 444 an E4 at 329.63 Hz reads flat");
        assert!(nearest_midi(&[], 64.0, 440.0).is_none());
    }

    #[test]
    fn a_position_drill_keeps_its_fret_off_the_fretboard() {
        let guitar: Instrument = Guitar::standard(GuitarBody::Electric).into();
        let found = drills(&guitar, TechniqueKind::Position);
        assert!(lit(found.first()).is_empty());
        let notes = drills(&guitar, TechniqueKind::Target);
        assert_eq!(lit(notes.first()).len(), 1);
        let mut trainer = NoteTrainer { practice: Practice::new(found), ..default() };
        assert!(trainer.guitar_targets().is_empty());
        trainer.practice.revealed = true;
        assert_eq!(trainer.guitar_targets().len(), 1);
        trainer.practice.step_by(1);
        assert!(trainer.guitar_targets().is_empty(), "the next question hides its answer");
    }

    #[test]
    fn a_flute_session_offers_its_own_kinds_and_choosing_keeps_a_session() {
        let flute: Instrument = Flute::default().into();
        let mut trainer = NoteTrainer::for_instrument(flute.clone());
        assert_eq!(
            trainer.kinds(),
            [TechniqueKind::Target, TechniqueKind::Register, TechniqueKind::Position]
        );
        assert_eq!(trainer.practice.drill().map(|d| d.target), Some(60.0), "C4 first");
        trainer.practice.step_by(5);
        trainer.choose(flute);
        assert_eq!(trainer.practice.index, 5, "the same flute resumes");
        trainer.choose(Guitar::standard(GuitarBody::Electric).into());
        assert_eq!(trainer.practice.index, 0, "another instrument starts afresh");
        assert!(trainer.guitar().is_some());
    }

    #[test]
    fn switching_kind_starts_that_kinds_drills_from_the_top() {
        let mut trainer = NoteTrainer::default();
        trainer.practice.step_by(3);
        let mut profile = harmonicon_app::profile::PlayerProfile::default();
        assert!(!trainer.set_kind(TechniqueKind::Glide, &mut profile), "nothing restored yet");
        assert_eq!(trainer.practice.index, 0);
        assert!(trainer.practice.drill().is_some_and(|d| d.kind == TechniqueKind::Glide));
    }

    /// A kind's progress goes into the profile under its instrument and
    /// kind, and comes back wrapped onto the list it resumes.
    #[test]
    fn progress_is_kept_per_instrument_and_kind_and_resumes() {
        let mut trainer = NoteTrainer::default();
        let mut profile = harmonicon_app::profile::PlayerProfile::default();
        assert!(!trainer.remember(&mut profile), "not before the saved one is applied");
        trainer.practice.restored = true;
        trainer.practice.step_by(2);
        trainer.practice.landed = 4;
        assert!(trainer.set_kind(TechniqueKind::Glide, &mut profile));
        let record = profile.note_trainer["guitar:target"];
        assert_eq!((record.landed, record.position), (4, 2));

        let mut back = NoteTrainer::default();
        let total = back.practice.drills.len() as u32;
        back.practice.resume(record.landed, record.position + total);
        assert_eq!((back.practice.landed, back.practice.index), (4, 2), "wrapped onto the list");
        assert_eq!(
            NoteTrainer::for_instrument(Flute::default().into()).record_key().as_deref(),
            Some("flute:target")
        );
    }
}

#[cfg(test)]
mod bend_progress_tests {
    use super::*;
    #[test]
    fn a_bend_must_begin_at_its_unbent_pitch_before_the_target_counts() {
        let instrument = Instrument::Fretted(Guitar::standard(GuitarBody::Electric));
        let mut practice = Practice::new(drills(&instrument, TechniqueKind::Glide));
        practice.auto_advance = false;
        let drill = *practice.drill().unwrap();
        assert!(!practice.listen(Some(drill.target), 1.0, 20.0, 0.6));
        assert!(!practice.listen(drill.start, 0.2, 20.0, 0.6));
        assert!(practice.started);
        practice.listen(None, 0.3, 20.0, 0.6);
        assert!(!practice.started, "a stopped note starts a new gesture");
        assert!(!practice.listen(Some(drill.target), 0.7, 20.0, 0.6));
        practice.listen(drill.start, 0.2, 20.0, 0.6);
        assert!(practice.listen(Some(drill.target), 0.7, 20.0, 0.6));
        assert!(practice.awaiting_release);
        practice.listen(None, 0.1, 20.0, 0.6);
        assert!(!practice.started, "another bend must establish its starting pitch again");
    }
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    #[test]
    fn reference_playback_cannot_complete_an_attempt_through_the_microphone() {
        let mut app = App::new();
        let trainer = NoteTrainer { reference_remaining: 1.8, ..default() };
        let target = trainer.practice.drill().unwrap().target;
        app.insert_resource(trainer)
            .init_resource::<BendingTrainerSettings>()
            .init_resource::<AudioSettings>()
            .init_resource::<Time>()
            .init_resource::<harmonicon_app::profile::PlayerProfile>()
            .add_message::<PitchEvent>()
            .add_systems(Update, listen);
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(0.1));
            app.world_mut().write_message(PitchEvent(vec![PitchInfo {
                midi: 0,
                note: String::new(),
                octave: 0,
                frequency: midi_to_freq_hz(target),
            }]));
            app.update();
        }
        let trainer = app.world().resource::<NoteTrainer>();
        assert_eq!(trainer.practice.landed, 0);
        assert_eq!(trainer.practice.held, 0.0);
    }
}

#[cfg(test)]
mod bass_session_tests {
    use super::*;
    #[test]
    fn bass_has_separate_progress_and_never_offers_guitar_chord_shapes() {
        let bass = NoteTrainer::for_instrument(Guitar::bass().into());
        let guitar = NoteTrainer::default();
        assert_eq!(bass.record_key().as_deref(), Some("bass:target"));
        assert_eq!(guitar.record_key().as_deref(), Some("guitar:target"));
        assert!(!bass.offers_chords());
        assert!(!bass.kinds().contains(&TechniqueKind::Glide));
        assert!(bass.guitar().is_some(), "both use the shared fretted controls");
        let mut retuned = Guitar::bass();
        retuned.strings = harmonicon_core::instrument::guitar::STANDARD_TUNING.to_vec();
        assert!(!NoteTrainer::for_instrument(retuned.into()).offers_chords());
    }
}
