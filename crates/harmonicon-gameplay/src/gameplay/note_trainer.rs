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
//! fretboard strip or the flute's upright drawing, both from the play
//! highway, shows where the fingers go.
//!
//! What it can't check: detection hears a pitch, not a string or a
//! fingering, so a position drill trusts the player to play it as asked.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

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
use super::flute_visuals::{FluteDrawing, FluteKey, paint_flute_keys, spawn_flute};
use super::guitar_visuals::{FretCell, paint_cell, spawn_fretboard};
use harmonicon_core::instrument::flute::Flute;

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
    landed: u32,
    /// Counts down from [`LANDED_SECS`] after a landing.
    celebrate: f32,
}

impl Practice {
    pub fn new(drills: Vec<Drill>) -> Self {
        Self { drills, ..default() }
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
        let Some(drill) = self.drill().copied() else { return false };
        self.heard = midi.map(|m| hear(&drill, m));
        self.heard_midi = midi;
        match self.heard {
            Some(Heard::Target { cents }) if cents.abs() <= tolerance_cents => self.held += dt,
            _ => self.held = 0.0,
        }
        if self.held < hold_secs {
            return false;
        }
        self.landed += 1;
        self.celebrate = LANDED_SECS;
        self.step_by(1);
        true
    }

    /// How far through the hold the player is, 0 to 1.
    pub fn hold_fraction(&self, hold_secs: f32) -> f32 {
        if hold_secs <= 0.0 { 1.0 } else { (self.held / hold_secs).clamp(0.0, 1.0) }
    }
}

/// The trainer's state for the session: the guitar, the kind on show and
/// its practice. Kept across visits, so leaving and returning resumes.
#[derive(Resource)]
pub struct NoteTrainer {
    instrument: Instrument,
    kind: TechniqueKind,
    practice: Practice,
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
        let practice = Practice::new(drills(&instrument, kind));
        Self { instrument, kind, practice }
    }

    /// Switches to `instrument`, keeping the session when it is the same
    /// one — so leaving and coming back resumes where the player was.
    pub fn choose(&mut self, instrument: Instrument) {
        if self.instrument != instrument {
            *self = Self::for_instrument(instrument);
        }
    }

    fn guitar(&self) -> Option<&Guitar> {
        match &self.instrument {
            Instrument::Guitar(guitar) => Some(guitar),
            Instrument::Harmonica(_) | Instrument::Flute(_) => None,
        }
    }

    fn flute(&self) -> Option<&Flute> {
        match &self.instrument {
            Instrument::Flute(flute) => Some(flute),
            Instrument::Harmonica(_) | Instrument::Guitar(_) => None,
        }
    }

    /// The kinds the tab bar offers, in its order: the instrument's own.
    fn kinds(&self) -> &'static [TechniqueKind] {
        self.instrument.techniques()
    }

    fn set_kind(&mut self, kind: TechniqueKind) {
        if self.kind != kind {
            self.kind = kind;
            self.practice = Practice::new(drills(&self.instrument, kind));
        }
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
        TechniqueKind::Register if flute => "note-trainer-kind-register-flute",
        TechniqueKind::Register => "note-trainer-kind-register",
        TechniqueKind::Position if flute => "note-trainer-kind-position-flute",
        TechniqueKind::Position => "note-trainer-kind-position",
    }
}

/// What to play, in words, on whichever instrument the trainer has.
fn instruction(loc: &Localization, instrument: &Instrument, drill: &Drill) -> String {
    match instrument {
        Instrument::Guitar(guitar) => guitar_instruction(loc, guitar, drill),
        Instrument::Flute(_) => flute_instruction(loc, drill),
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
    if practice.celebrate > 0.0 {
        let next = practice.drill().map(|d| note_name(d.target)).unwrap_or_default();
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
                ("heard", practice.heard_midi.map(note_name).unwrap_or_default()),
                ("note", note_name(drill.target)),
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

pub(super) fn setup(
    mut commands: Commands,
    trainer: Res<NoteTrainer>,
    mut pitch_range: ResMut<PitchRange>,
    loc: Res<Localization>,
) {
    if trainer.guitar().is_none() && trainer.flute().is_none() {
        return;
    }
    if let Some((low, high)) = trainer.instrument.range() {
        *pitch_range = PitchRange::from_freqs(
            [midi_to_freq_hz(f32::from(low)), midi_to_freq_hz(f32::from(high))],
            PITCH_RANGE_MARGIN_SEMITONES,
        );
    }
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            GameplayRoot,
        ))
        .id();
    let title = commands
        .spawn_scene(title_column_scene(String::from(loc.msg(if trainer.flute().is_some() {
            "note-trainer-flute-title"
        } else {
            "note-trainer-guitar-title"
        }))))
        .id();
    let header = commands.spawn_scene(header_scene()).id();
    commands.entity(header).add_child(title);
    commands.entity(root).add_child(header);
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("back"),
        |_: On<Activate>,
         mut next: ResMut<NextState<AppState>>,
         mut ret_play: ResMut<harmonicon_app::app::ReturnToPlay>| {
            ret_play.0 = true;
            next.set(AppState::Menu);
        },
    );

    let tabs = commands
        .spawn(Node { padding: UiRect::axes(Val::Px(24.0), Val::Px(8.0)), ..default() })
        .id();
    commands.entity(root).add_child(tabs);
    let kinds = trainer.kinds();
    let labels: Vec<String> =
        kinds.iter().map(|&k| String::from(loc.msg(kind_key(&trainer.instrument, k)))).collect();
    let selected = kinds.iter().position(|&k| k == trainer.kind).unwrap_or(0);
    spawn_tab_bar(
        &mut commands,
        tabs,
        &labels,
        selected,
        |ev: On<TabSelect>, mut trainer: ResMut<NoteTrainer>| {
            if let Some(&kind) = trainer.kinds().get(ev.index) {
                trainer.set_kind(kind);
            }
        },
    );

    let previous = String::from(loc.msg("note-trainer-previous"));
    let next = String::from(loc.msg("note-trainer-next"));
    let listen = String::from(loc.msg("note-trainer-listen"));
    commands.entity(root).with_children(|root| {
        root.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(12.0),
            padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
            ..default()
        })
        .with_children(|card| {
            card.spawn((
                InstructionLabel,
                Text::new(""),
                TextFont { font_size: FontSize::Px(28.0), ..default() },
                TextColor(Color::WHITE),
            ));
            card.spawn((
                StatusLabel,
                Text::new(""),
                TextFont { font_size: FontSize::Px(22.0), ..default() },
                TextColor(Color::srgb(0.75, 0.85, 1.0)),
            ));
            card.spawn((
                Node { width: Val::Px(320.0), height: Val::Px(10.0), ..default() },
                BackgroundColor(Color::srgb(0.12, 0.13, 0.18)),
            ))
            .with_children(|track| {
                track.spawn((
                    HoldBar,
                    Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(Color::srgb(0.35, 0.85, 0.45)),
                ));
            });
            card.spawn(Node { column_gap: Val::Px(8.0), ..default() }).with_children(|row| {
                row.spawn_empty().apply_scene(button::small(
                    &previous,
                    |_: On<Activate>, mut t: ResMut<NoteTrainer>| {
                        t.practice.step_by(-1);
                    },
                ));
                row.spawn_empty().apply_scene(button::small(&listen, play_reference));
                row.spawn_empty().apply_scene(button::small(
                    &next,
                    |_: On<Activate>, mut t: ResMut<NoteTrainer>| {
                        t.practice.step_by(1);
                    },
                ));
            });
            card.spawn((
                LandedLabel,
                Text::new(""),
                TextFont { font_size: FontSize::Px(16.0), ..default() },
                TextColor(Color::srgb(0.6, 0.65, 0.75)),
            ));
        });
        root.spawn(Node {
            // The upright flute needs the height its fourteen keys take.
            height: Val::Px(if trainer.flute().is_some() { 520.0 } else { 320.0 }),
            padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
            ..default()
        })
        .with_children(|strip| match (trainer.guitar(), trainer.flute()) {
            (Some(guitar), _) => spawn_fretboard(strip, guitar, 560.0),
            (_, Some(flute)) => spawn_flute(strip, flute),
            _ => {}
        });
    });
}

/// Sounds the current drill's target in the instrument's own voice, at the
/// trainer's A4 reference.
fn play_reference(
    _: On<Activate>,
    trainer: Res<NoteTrainer>,
    settings: Res<BendingTrainerSettings>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
) {
    use harmonicon_core::synth::{Expr, PhraseNote, SAMPLE_RATE, render_pcm_with};
    let Some(drill) = trainer.practice.drill() else { return };
    let freq = settings.a4_hz * 2f32.powf((drill.target - 69.0) / 12.0);
    let note = PhraseNote { tick: 0, len: 1, freq: Some(freq), expr: Expr::None };
    let pcm = render_pcm_with(&[note], 1.5, trainer.instrument.synth_voice());
    let wav = harmonicon_core::wav::encode_wav(&pcm, SAMPLE_RATE);
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
    time: Res<Time>,
    mut last: Local<Vec<PitchInfo>>,
) {
    if let Some(ev) = reader.read().last() {
        last.clone_from(&ev.0);
    }
    let Some(target) = trainer.practice.drill().map(|d| d.target) else { return };
    let midi = nearest_midi(&last, target, settings.a4_hz);
    trainer.practice.listen(midi, time.delta_secs(), settings.tolerance_cents, settings.hold_secs);
}

pub(super) fn update_labels(
    trainer: Res<NoteTrainer>,
    settings: Res<BendingTrainerSettings>,
    loc: Res<Localization>,
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
    let what =
        practice.drill().map(|d| instruction(&loc, &trainer.instrument, d)).unwrap_or_default();
    for mut text in &mut texts.p0() {
        set(&mut text, what.clone());
    }
    let now = status(&loc, practice, settings.tolerance_cents);
    for mut text in &mut texts.p1() {
        set(&mut text, now.clone());
    }
    let count = String::from(loc.msg_args(
        "note-trainer-count",
        &[
            ("landed", practice.landed.to_string()),
            ("index", (practice.index + 1).to_string()),
            ("total", practice.drills.len().to_string()),
        ],
    ));
    for mut text in &mut texts.p2() {
        set(&mut text, count.clone());
    }
    let width = Val::Percent(practice.hold_fraction(settings.hold_secs) * 100.0);
    for mut node in &mut bars {
        if node.width != width {
            node.width = width;
        }
    }
}

pub(super) fn update_fretboard(
    trainer: Res<NoteTrainer>,
    mut cells: Query<(&FretCell, &mut Text, &mut BackgroundColor)>,
) {
    let targets = lit(trainer.practice.drill());
    for (cell, mut text, mut color) in &mut cells {
        paint_cell(cell, &targets, &mut text, &mut color);
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
        trainer.set_kind(TechniqueKind::Glide);
        assert_eq!(trainer.practice.index, 0);
        assert!(trainer.practice.drill().is_some_and(|d| d.kind == TechniqueKind::Glide));
    }
}
