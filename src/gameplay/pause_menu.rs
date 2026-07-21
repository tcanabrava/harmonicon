// SPDX-License-Identifier: MIT

//! The in-game pause overlay: a translucent menu with Resume / Restart / Quit,
//! toggled with Escape. Shares the gameplay [`Paused`] flag (every gameplay
//! chain gates on it) and pauses/resumes the song's audio sink.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy::ui_widgets::{Slider, SliderRange, SliderStep, SliderValue, TrackClick, ValueChange};
use bevy_fluent::Localization;

use super::adaptive_difficulty::AdaptiveDifficulty;
use super::{GameplayRoot, LoopConfig, MusicPlayer, Paused};
use crate::app::{AppState, GameplayMode, ReturnToSongList, SelectedSong};
use crate::dialogs::button;
use crate::jam::backing;
use crate::jam::improv::ImprovStats;
use crate::lessons::{LessonContext, PassCriteria, lesson_passed};
use crate::localization::LocalizationExt;
use crate::profile::{PlayerProfile, record_lesson, save_profile};
use crate::song::SongManifest;

/// Root of the pause overlay; toggled between hidden/visible.
#[derive(Component, Default, Clone)]
pub(super) struct PauseMenuRoot;

/// Practice aid: when on, gameplay freezes the instant a playable note
/// reaches the hit line without having been hit, instead of letting it run
/// out and become a miss — resuming the moment it's hit (see
/// `super::note_due_and_unresolved` and `super::tick_clock`). Off by
/// default; a standing player preference (like `JamLoop`), so it isn't reset
/// between restarts/songs.
#[derive(Resource, Default)]
pub struct WaitForNoteMode(pub bool);

/// The "Wait for Note: on/off" readout, kept in step with [`WaitForNoteMode`].
#[derive(Component, Default, Clone)]
pub(super) struct WaitForNoteLabel;

fn on_toggle_wait_mode(_: On<Pointer<Click>>, mut wait_mode: ResMut<WaitForNoteMode>) {
    wait_mode.0 = !wait_mode.0;
}

fn wait_mode_label_text(loc: &Localization, enabled: bool) -> String {
    if enabled {
        loc.msg("pause-wait-for-note-on").into()
    } else {
        loc.msg("pause-wait-for-note-off").into()
    }
}

/// Keeps the "Wait for Note: ..." readout in step with the toggle. Not
/// gated on `Paused` — the button only lives on the (otherwise hidden)
/// pause overlay, so it can only be clicked while already paused, same as
/// `apply_music_volume` intentionally keeps running through a pause.
pub(super) fn update_wait_mode_label(
    wait_mode: Res<WaitForNoteMode>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<WaitForNoteLabel>>,
) {
    if !wait_mode.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(wait_mode_label_text(&loc, wait_mode.0));
    }
}

/// Practice aid: scales how fast the gameplay clock advances. The note
/// highway and metronome read the clock directly, so they slow down for
/// free; real time-stretched audio is a later upgrade, so `tick_clock` just
/// pauses the music sink below 100% instead of playing it pitch-shifted.
/// `1.0` (100%) by default; a standing player preference (like
/// `WaitForNoteMode`), so it isn't reset between restarts/songs.
#[derive(Resource, Clone, Copy, PartialEq)]
pub struct PracticeSpeed(pub f32);

impl Default for PracticeSpeed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// The "Speed: ..." readout beside the practice-speed slider, kept in step
/// with [`PracticeSpeed`].
#[derive(Component, Default, Clone)]
pub(super) struct PracticeSpeedLabel;

/// The fill bar inside the practice-speed slider track.
#[derive(Component, Default, Clone)]
pub(super) struct PracticeSpeedFill;

fn practice_speed_label_text(loc: &Localization, speed: f32) -> String {
    loc.msg_args("pause-speed", &[("pct", format!("{:.0}", speed * 100.0))])
        .into()
}

/// `50%..=100%` in `10%` steps — same range the old click-to-cycle button
/// stepped through.
const PRACTICE_SPEED_MIN: f32 = 0.5;
const PRACTICE_SPEED_MAX: f32 = 1.0;

fn set_practice_speed(ev: On<ValueChange<f32>>, mut speed: ResMut<PracticeSpeed>) {
    speed.0 = ev.value;
}

/// One row: a "Speed" slider (`50%..=100%`) + "Speed: NN%" readout, wired to
/// [`PracticeSpeed`]. The track is a `bsn!` `Slider`; the label/readout stay
/// imperative like every other pause-menu readout (custom font, which `bsn!`
/// can't set in 0.19).
fn spawn_practice_speed_row(
    commands: &mut Commands,
    parent: Entity,
    loc: &Localization,
    value: f32,
) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    commands.entity(row).with_children(|r| {
        r.spawn((
            Text::new("\u{1F422}"),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.70, 0.70, 0.80)),
        ));
    });
    let track = commands
        .spawn_scene(practice_speed_slider_scene(value))
        .insert((
            SliderRange::new(PRACTICE_SPEED_MIN, PRACTICE_SPEED_MAX),
            SliderStep(0.1),
        ))
        .id();
    commands.entity(row).add_child(track);
    commands.entity(row).with_children(|r| {
        r.spawn((
            Text::new(practice_speed_label_text(loc, value)),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.70, 0.70, 0.80)),
            PracticeSpeedLabel,
        ));
    });
    commands.entity(parent).add_child(row);
}

fn practice_speed_slider_scene(value: f32) -> impl Scene {
    let frac = (value - PRACTICE_SPEED_MIN) / (PRACTICE_SPEED_MAX - PRACTICE_SPEED_MIN);
    bsn! {
        Slider { track_click: {TrackClick::Snap} }
        SliderValue({value})
        Node { width: {Val::Px(140.0)}, height: {Val::Px(12.0)} }
        BackgroundColor({Color::srgb(0.14, 0.14, 0.22)})
        on(set_practice_speed)
        Children [
            (
                Node { width: {Val::Percent(frac * 100.0)}, height: {Val::Percent(100.0)} }
                BackgroundColor({Color::srgb(0.35, 0.75, 1.0)})
                PracticeSpeedFill
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

/// Keeps the practice-speed slider's fill and the "Speed: ..." readout in
/// step with [`PracticeSpeed`]. Not gated on `Paused`, same reasoning as
/// `update_wait_mode_label`.
pub(super) fn update_practice_speed_slider(
    speed: Res<PracticeSpeed>,
    loc: Res<Localization>,
    mut fills: Query<&mut Node, With<PracticeSpeedFill>>,
    mut labels: Query<&mut Text, With<PracticeSpeedLabel>>,
) {
    if !speed.is_changed() {
        return;
    }
    let frac = (speed.0 - PRACTICE_SPEED_MIN) / (PRACTICE_SPEED_MAX - PRACTICE_SPEED_MIN);
    for mut node in &mut fills {
        node.width = Val::Percent(frac * 100.0);
    }
    for mut text in &mut labels {
        *text = Text::new(practice_speed_label_text(&loc, speed.0));
    }
}

/// The "Loop: ..." readout, kept in step with [`LoopConfig`].
#[derive(Component, Default, Clone)]
pub(super) struct LoopRangeLabel;

/// The loop range itself is set by click-and-drag directly on the
/// song-progress bar while paused (`song_progress_overlay`'s
/// `ProgressBarMode::Edit` — see its module doc comment); this button only
/// ever clears it.
fn on_clear_loop(_: On<Pointer<Click>>, mut loop_cfg: ResMut<LoopConfig>) {
    *loop_cfg = LoopConfig::default();
}

/// Pure so both possible readouts (off / a valid range) are unit-testable
/// without spinning up an `App`.
fn loop_label_text(loc: &Localization, cfg: &LoopConfig) -> String {
    if cfg.active {
        loc.msg_args(
            "pause-loop-range",
            &[
                ("start", format!("{:.0}", cfg.start_time)),
                ("end", format!("{:.0}", cfg.end_time)),
            ],
        )
        .into()
    } else {
        loc.msg("pause-loop-off").into()
    }
}

/// Keeps the "Loop: ..." readout in step with [`LoopConfig`]. Not gated on
/// `Paused`, same reasoning as `update_wait_mode_label`.
pub(super) fn update_loop_label(
    loop_cfg: Res<LoopConfig>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<LoopRangeLabel>>,
) {
    if !loop_cfg.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(loop_label_text(&loc, &loop_cfg));
    }
}

// ── Adaptive difficulty controls ──────────────────────────────────────────────

/// Which of `AdaptiveDifficulty::sections` the pause menu's phrase selector
/// is currently showing/editing — set by clicking a section's rectangle on
/// the song-progress overlay's phrase strip while paused (see
/// `song_progress_overlay::on_phrase_rect_click`), not from within the pause
/// menu itself. Not reset between restarts (like `WaitForNoteMode`/
/// `PracticeSpeed`) — picking up where you left off is more useful than
/// always snapping back to the first phrase.
#[derive(Resource, Default)]
pub struct SelectedPhraseIndex(pub usize);

/// The "Section: ... — Learned: NN%" readout.
#[derive(Component, Default, Clone)]
pub(super) struct PhraseSelectorLabel;

/// The "Adaptive Difficulty: on/off" readout.
#[derive(Component, Default, Clone)]
pub(super) struct AdaptiveDifficultyLabel;

/// Looks up the current song's `PlayerProfile` record key the same way
/// `results.rs` does (the manifest's own path, stable across restarts).
fn song_key(selected: &SelectedSong, manifests: &Assets<SongManifest>) -> Option<String> {
    manifests
        .get(&selected.0)
        .map(|m| m.path.display().to_string())
}

/// Pure so the readout is unit-testable without a live `AdaptiveDifficulty`.
fn phrase_selector_text(loc: &Localization, name: Option<&str>, learned: f32) -> String {
    match name {
        Some(name) => loc
            .msg_args(
                "pause-phrase-section",
                &[
                    ("name", name.to_string()),
                    ("pct", format!("{:.0}", learned * 100.0)),
                ],
            )
            .into(),
        None => loc.msg("pause-phrase-no-sections").into(),
    }
}

/// Keeps the phrase-selector readout in step with `SelectedPhraseIndex`/
/// `AdaptiveDifficulty`. Not gated on `Paused`, same reasoning as
/// `update_wait_mode_label`.
pub(super) fn update_phrase_selector_label(
    selected: Res<SelectedPhraseIndex>,
    adaptive: Res<AdaptiveDifficulty>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<PhraseSelectorLabel>>,
) {
    if !selected.is_changed() && !adaptive.is_changed() {
        return;
    }
    let section = adaptive.sections.get(selected.0);
    let learned = section
        .map(|_| adaptive.learned.get(selected.0).copied().unwrap_or(0.0))
        .unwrap_or(0.0);
    let text = phrase_selector_text(&loc, section.map(|s| s.name.as_str()), learned);
    for mut label in &mut labels {
        *label = Text::new(text.clone());
    }
}

fn adaptive_difficulty_label_text(loc: &Localization, enabled: bool) -> String {
    if enabled {
        loc.msg("pause-adaptive-difficulty-on").into()
    } else {
        loc.msg("pause-adaptive-difficulty-off").into()
    }
}

pub(super) fn update_adaptive_difficulty_label(
    adaptive: Res<AdaptiveDifficulty>,
    loc: Res<Localization>,
    mut labels: Query<&mut Text, With<AdaptiveDifficultyLabel>>,
) {
    if !adaptive.is_changed() {
        return;
    }
    for mut label in &mut labels {
        *label = Text::new(adaptive_difficulty_label_text(&loc, adaptive.enabled));
    }
}

/// Flips both the live per-session cache (for an immediate mid-song
/// re-unlock, via `resync_notes_on_adaptive_change` reacting to
/// `AdaptiveDifficulty::is_changed()`) and the persisted global setting —
/// a single on/off switch shared by every song, not stored per-song.
fn on_toggle_adaptive_difficulty(
    _: On<Pointer<Click>>,
    mut setting: ResMut<crate::settings::AdaptiveDifficultyEnabled>,
    mut adaptive: ResMut<AdaptiveDifficulty>,
) {
    adaptive.enabled = !adaptive.enabled;
    setting.0 = adaptive.enabled;
}

/// Marks the phrase-learned slider's track entity (the one carrying
/// `SliderValue`), so [`update_phrase_learned_slider`] can re-sync it when
/// the selected phrase changes.
#[derive(Component, Default, Clone)]
pub(super) struct PhraseLearnedSlider;

/// The fill bar inside the phrase-learned slider track.
#[derive(Component, Default, Clone)]
pub(super) struct PhraseLearnedFill;

/// The "NN%" readout beside the phrase-learned slider.
#[derive(Component, Default, Clone)]
pub(super) struct PhraseLearnedValueLabel;

/// Clamps a learned fraction into `0.0..=1.0` — split out for unit testing
/// without the ECS plumbing `set_selected_phrase_learned` needs (a real
/// `SelectedSong`/`Assets<SongManifest>`/`PlayerProfile`).
fn clamp_learned(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// Sets the selected phrase's learned fraction directly to `value` — the
/// slider replacement for the old "-25%"/"+25%" buttons. Updates both the
/// live `AdaptiveDifficulty` (so the progress bar's rectangle re-tints
/// immediately, and so `resync_notes_on_adaptive_change` in
/// `gameplay_2d`/`gameplay_3d` rebuilds the note highway on the very next
/// frame) and the persisted `PlayerProfile` (so it survives a restart too).
fn set_selected_phrase_learned(
    value: f32,
    selected: &SelectedPhraseIndex,
    selected_song: &SelectedSong,
    manifests: &Assets<SongManifest>,
    profile: &mut PlayerProfile,
    adaptive: &mut AdaptiveDifficulty,
) {
    if selected.0 >= adaptive.sections.len() {
        return;
    }
    let value = clamp_learned(value);
    if adaptive.learned.len() <= selected.0 {
        adaptive.learned.resize(selected.0 + 1, 0.0);
    }
    adaptive.learned[selected.0] = value;
    let Some(key) = song_key(selected_song, manifests) else {
        return;
    };
    let record = profile.songs.entry(key).or_default();
    if record.phrase_learned.len() <= selected.0 {
        record.phrase_learned.resize(selected.0 + 1, 0.0);
    }
    record.phrase_learned[selected.0] = value;
    crate::profile::save_profile(profile);
}

fn on_phrase_learned_slider_change(
    ev: On<ValueChange<f32>>,
    selected: Res<SelectedPhraseIndex>,
    selected_song: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut profile: ResMut<PlayerProfile>,
    mut adaptive: ResMut<AdaptiveDifficulty>,
) {
    set_selected_phrase_learned(
        ev.value,
        &selected,
        &selected_song,
        &manifests,
        &mut profile,
        &mut adaptive,
    );
}

/// One row: a "Learned" slider (`0%..=100%`) for the currently selected
/// phrase section + "NN%" readout. Re-synced whenever `SelectedPhraseIndex`
/// changes (clicking a different section on the progress-bar overlay) as
/// well as when the value itself changes — see
/// [`update_phrase_learned_slider`].
fn spawn_phrase_learned_row(commands: &mut Commands, parent: Entity, value: f32) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    commands.entity(row).with_children(|r| {
        r.spawn((
            Text::new("Learned:"),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.70, 0.70, 0.80)),
        ));
    });
    let track = commands
        .spawn_scene(phrase_learned_slider_scene(value))
        .insert((SliderRange::new(0.0, 1.0), SliderStep(0.01)))
        .id();
    commands.entity(row).add_child(track);
    commands.entity(row).with_children(|r| {
        r.spawn((
            Node {
                width: Val::Px(50.0),
                ..default()
            },
            Text::new(format!("{:.0}%", value * 100.0)),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.70, 0.70, 0.80)),
            PhraseLearnedValueLabel,
        ));
    });
    commands.entity(parent).add_child(row);
}

fn phrase_learned_slider_scene(value: f32) -> impl Scene {
    bsn! {
        Slider { track_click: {TrackClick::Snap} }
        SliderValue({value})
        Node { width: {Val::Px(140.0)}, height: {Val::Px(12.0)} }
        BackgroundColor({Color::srgb(0.14, 0.14, 0.22)})
        PhraseLearnedSlider
        on(on_phrase_learned_slider_change)
        Children [
            (
                Node { width: {Val::Percent(value * 100.0)}, height: {Val::Percent(100.0)} }
                BackgroundColor({Color::srgb(0.35, 0.75, 1.0)})
                PhraseLearnedFill
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}

/// Keeps the phrase-learned slider's fill/value and readout in step with
/// the currently selected phrase's learned fraction — needed both when
/// `AdaptiveDifficulty` changes (dragging the slider itself, or any other
/// source) and when `SelectedPhraseIndex` changes (clicking a different
/// section on the progress-bar overlay), since either can mean this slider
/// should now show a different number. Not gated on `Paused`, same
/// reasoning as `update_wait_mode_label`.
pub(super) fn update_phrase_learned_slider(
    selected: Res<SelectedPhraseIndex>,
    adaptive: Res<AdaptiveDifficulty>,
    sliders: Query<Entity, With<PhraseLearnedSlider>>,
    mut fills: Query<&mut Node, With<PhraseLearnedFill>>,
    mut labels: Query<&mut Text, With<PhraseLearnedValueLabel>>,
    mut commands: Commands,
) {
    if !selected.is_changed() && !adaptive.is_changed() {
        return;
    }
    let value = adaptive.learned.get(selected.0).copied().unwrap_or(0.0);
    // `SliderValue` is an immutable component (bevy_ui_widgets), so it can
    // only be replaced wholesale via `insert`, not mutated in place — same
    // as `slider_self_update` does for a drag-driven change.
    for entity in &sliders {
        commands.entity(entity).insert(SliderValue(value));
    }
    for mut node in &mut fills {
        node.width = Val::Percent(value * 100.0);
    }
    for mut text in &mut labels {
        *text = Text::new(format!("{:.0}%", value * 100.0));
    }
}

/// Spawns the (initially hidden) pause overlay. Tagged `GameplayRoot` so it is
/// torn down with the rest of the scene. Two columns, side by side: the left
/// one is transport actions only (Resume/Restart/Quit Song, + Finish Lesson
/// where it applies); the right one is every practice aid, so the two don't
/// visually compete — a slip of the mouse over the "big" actions shouldn't be
/// one misclick away from a tweak knob, or vice versa. Most of the tree is
/// authored declaratively with `bsn!`; sliders and their readouts are
/// imperative (`SliderRange`/`SliderStep` have no `Default`, so they can't be
/// bsn! patches, and labels need the default font, which `bsn!` can't set in
/// 0.19).
///
/// Speed and Wait-for-Note are practice aids for a scored, fixed-length song
/// — Jam Session has no notes to wait for and no fixed pacing to slow down —
/// so they're omitted entirely in that mode rather than shown disabled. The
/// A–B loop controls stay in every mode: dragging a range on the (now
/// always-present, see `song_progress_overlay`) progress bar while paused is
/// exactly "select a part of the song to repeat", which is just as useful
/// for free-play practice as for a scored run.
pub(super) fn setup_pause_menu(
    mut commands: Commands,
    mode: Res<GameplayMode>,
    lesson: Option<Res<LessonContext>>,
    speed: Res<PracticeSpeed>,
    selected_phrase: Res<SelectedPhraseIndex>,
    adaptive: Res<AdaptiveDifficulty>,
    loc: Res<Localization>,
) {
    let is_jam = *mode == GameplayMode::JamSession;
    // A scale-adherence lesson (see `PassCriteria::ScaleAdherence`) is the
    // one lesson type that never reaches the results screen on its own —
    // Jam Session has no natural end — so it needs its own explicit
    // "submit for judgment" action here instead.
    let is_lesson_jam = is_jam && lesson.is_some();
    let learned = adaptive
        .learned
        .get(selected_phrase.0)
        .copied()
        .unwrap_or(0.0);
    let phrase_text = phrase_selector_text(
        &loc,
        adaptive
            .sections
            .get(selected_phrase.0)
            .map(|s| s.name.as_str()),
        learned,
    );

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(48.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            GlobalZIndex(200),
            GameplayRoot,
            PauseMenuRoot,
            Visibility::Hidden,
        ))
        .id();

    // ── Left column: transport actions ──────────────────────────────────
    let actions = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(actions);
    commands.entity(actions).with_children(|col| {
        col.spawn((
            Text::new("PAUSED"),
            TextFont {
                font_size: FontSize::Px(52.0),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
        col.spawn_empty()
            .apply_scene(button::default("Resume", on_resume));
        col.spawn_empty()
            .apply_scene(button::default("Restart", on_restart));
        col.spawn_empty()
            .apply_scene(button::default(&loc.msg("pause-quit-song"), on_quit));
        if is_lesson_jam {
            col.spawn_empty().apply_scene(button::default(
                &loc.msg("pause-finish-lesson"),
                on_finish_lesson,
            ));
        }
    });

    // ── Right column: practice aids ──────────────────────────────────────
    let aids = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Start,
            row_gap: Val::Px(12.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(aids);

    if !is_jam {
        commands.entity(aids).with_children(|children| {
            children.spawn_empty().apply_scene(bsn! {
                Node {
                    flex_direction: {FlexDirection::Row},
                    align_items: {AlignItems::Center},
                    column_gap: {Val::Px(8.0)},
                }
                Children [
                    button::small(&loc.msg("pause-wait-for-note-button"), on_toggle_wait_mode),
                    (
                        Text({wait_mode_label_text(&loc, false)})
                        TextFont { font_size: {FontSize::Px(15.0)} }
                        TextColor({Color::srgb(0.70, 0.70, 0.80)})
                        WaitForNoteLabel
                    ),
                ]
            });
        });
        spawn_practice_speed_row(&mut commands, aids, &loc, speed.0);
        commands.entity(aids).with_children(|children| {
            children.spawn_empty().apply_scene(bsn! {
                Node {
                    flex_direction: {FlexDirection::Row},
                    align_items: {AlignItems::Center},
                    column_gap: {Val::Px(8.0)},
                }
                Children [
                    button::small(&loc.msg("pause-adaptive-difficulty-button"), on_toggle_adaptive_difficulty),
                    (
                        Text({adaptive_difficulty_label_text(&loc, adaptive.enabled)})
                        TextFont { font_size: {FontSize::Px(15.0)} }
                        TextColor({Color::srgb(0.70, 0.70, 0.80)})
                        AdaptiveDifficultyLabel
                    ),
                ]
            });
            // No prev/next buttons: the section itself is picked by clicking
            // its rectangle on the song-progress overlay's phrase strip
            // (`song_progress_overlay::on_phrase_rect_click`), only possible
            // while paused — same as dragging that bar to set a loop range.
            children.spawn_empty().apply_scene(bsn! {
                Text({phrase_text})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.70, 0.70, 0.80)})
                PhraseSelectorLabel
            });
        });
        spawn_phrase_learned_row(&mut commands, aids, learned);
        commands.entity(aids).with_children(|children| {
            children.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg("pause-drag-section-hint"))})
                TextFont { font_size: {FontSize::Px(13.0)} }
                TextColor({Color::srgb(0.55, 0.55, 0.62)})
            });
            children.spawn_empty().apply_scene(bsn! {
                Text({String::from(loc.msg("pause-notes-update-hint"))})
                TextFont { font_size: {FontSize::Px(13.0)} }
                TextColor({Color::srgb(0.55, 0.55, 0.62)})
            });
        });
    }

    commands.entity(aids).with_children(|children| {
        children.spawn_empty().apply_scene(bsn! {
            Node {
                flex_direction: {FlexDirection::Row},
                align_items: {AlignItems::Center},
                column_gap: {Val::Px(8.0)},
            }
            Children [
                button::small(&loc.msg("pause-clear-loop"), on_clear_loop),
                (
                    Text({loop_label_text(&loc, &LoopConfig::default())})
                    TextFont { font_size: {FontSize::Px(15.0)} }
                    TextColor({Color::srgb(0.70, 0.70, 0.80)})
                    LoopRangeLabel
                ),
            ]
        });
        children.spawn_empty().apply_scene(bsn! {
            Text({String::from(loc.msg("pause-drag-loop-hint"))})
            TextFont { font_size: {FontSize::Px(15.0)} }
            TextColor({Color::srgb(0.55, 0.55, 0.62)})
        });
    });

    // Always-visible pause button, bottom-right — Escape's on-screen
    // equivalent. A separate top-level entity (not a child of the overlay
    // above, which starts `Visibility::Hidden`) so it stays visible while
    // playing; it's naturally unreachable to clicks once paused, since the
    // overlay is a full-screen backdrop on top of it. Matters most for a
    // future touch/mobile build, which has no Escape key at all.
    //
    // Needs its own `GlobalZIndex` strictly between the background layer
    // each gameplay mode paints at `GlobalZIndex(1)` (gameplay_2d/3d,
    // jam_session — being a sibling top-level node rather than a child of
    // that background root, it wouldn't otherwise inherit painting above
    // it) and the pause overlay's `GlobalZIndex(200)` (so pausing still
    // visually and click-wise covers it, per the paragraph above).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                bottom: Val::Px(20.0),
                ..default()
            },
            GlobalZIndex(100),
            GameplayRoot,
        ))
        .with_children(|parent| {
            parent
                .spawn_empty()
                .apply_scene(button::small("\u{23F8}", on_pause_button_click));
        });
}

// ── Dedicated button callbacks ────────────────────────────────────────────────

fn on_resume(
    _: On<Pointer<Click>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    apply_resume(&mut paused);
    for mut vis in &mut overlay {
        *vis = Visibility::Hidden;
    }
    for sink in &sinks {
        sink.play();
    }
}

fn on_restart(
    _: On<Pointer<Click>>,
    mut paused: ResMut<Paused>,
    mut next_state: ResMut<NextState<AppState>>,
    generated_jam: Option<Res<backing::GeneratedJamSession>>,
) {
    // A generated jam's `SelectedSong` was built by `Assets::add`, not
    // `AssetServer::load` — it has no tracked `LoadState`, so routing
    // through `SongLoading` would hang there forever waiting on
    // `check_loading`'s `is_loaded_with_dependencies` (see
    // `GeneratedJamSession`'s doc comment). Skip straight back to `Playing`
    // instead; `OnEnter(AppState::Playing)`'s own systems (`reset_score`,
    // `jam::session::setup`, ...) already do the "fresh restart" work that
    // `SongLoading` exists to wait for on the normal, asset-server path.
    let target = if generated_jam.is_some() {
        AppState::Playing
    } else {
        AppState::SongLoading
    };
    apply_restart(&mut paused, &mut next_state, target);
}

fn on_quit(
    _: On<Pointer<Click>>,
    mut paused: ResMut<Paused>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
) {
    apply_quit(&mut paused, &mut next_state, &mut return_to_song_list);
}

/// Picks the one `ImprovStats` fraction relevant to `criteria` — the three
/// jam-based `PassCriteria` variants (`ScaleAdherence`/`ChordToneAdherence`/
/// `PhraseDiscipline`) each read a different tally off the same running
/// stats; `None` for a chart-backed criterion (or no criterion), which never
/// reads `ImprovStats` at all. Pure so it's directly unit-testable.
fn jam_fraction_for(criteria: Option<&PassCriteria>, stats: &ImprovStats) -> Option<f32> {
    match criteria {
        Some(PassCriteria::ScaleAdherence { .. }) => stats.adherence(),
        Some(PassCriteria::ChordToneAdherence { .. }) => stats.chord_tone_adherence(),
        Some(PassCriteria::PhraseDiscipline { .. }) => stats.phrase_discipline(),
        _ => None,
    }
}

/// Judges a jam-based lesson on demand — the only lesson types with no
/// natural end to judge them at (see `PassCriteria::ScaleAdherence`).
/// Records the result and returns to the menu the same way "Quit Song"
/// does; `route_menu_entry` sees the still-present `LessonContext` and
/// routes to the lesson list from there, same as any other lesson.
fn on_finish_lesson(
    _: On<Pointer<Click>>,
    lesson: Res<LessonContext>,
    improv_stats: Res<ImprovStats>,
    mut profile: ResMut<PlayerProfile>,
    mut paused: ResMut<Paused>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
) {
    let fraction = jam_fraction_for(lesson.pass_criteria.as_ref(), &improv_stats);
    let passed = lesson_passed(lesson.pass_criteria.as_ref(), 0.0, &[], fraction);
    let record = profile.lessons.entry(lesson.lesson_id.clone()).or_default();
    record_lesson(record, passed, fraction.unwrap_or(0.0));
    save_profile(&profile);
    apply_quit(&mut paused, &mut next_state, &mut return_to_song_list);
}

// Pure effects, split out so they can be unit-tested without the UI/observers.
fn apply_resume(paused: &mut Paused) {
    paused.0 = false;
}

fn apply_restart(paused: &mut Paused, next_state: &mut NextState<AppState>, target: AppState) {
    paused.0 = false;
    // Re-enter via SongLoading so the whole song setup runs fresh (the asset
    // is already loaded, so it resumes immediately) — or, for a generated
    // jam with no asset-server-tracked load state to wait on, straight back
    // to Playing (see `on_restart`).
    next_state.set(target);
}

fn apply_quit(
    paused: &mut Paused,
    next_state: &mut NextState<AppState>,
    return_to_song_list: &mut ReturnToSongList,
) {
    paused.0 = false;
    // Land back on the song list, not the main menu.
    return_to_song_list.0 = true;
    next_state.set(AppState::Menu);
}

/// Flips [`Paused`], the overlay's visibility, and the song audio — shared
/// by Escape ([`handle_pause_input`]) and the on-screen pause button
/// ([`on_pause_button_click`]) so the two stay in lockstep.
fn toggle_pause(
    paused: &mut Paused,
    overlay: &mut Query<&mut Visibility, With<PauseMenuRoot>>,
    sinks: &Query<&AudioSink, With<MusicPlayer>>,
) {
    paused.0 = !paused.0;
    for mut vis in overlay.iter_mut() {
        *vis = if paused.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for sink in sinks.iter() {
        if paused.0 {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

/// Escape toggles the pause state, the overlay's visibility, and the song audio.
pub(super) fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    toggle_pause(&mut paused, &mut overlay, &sinks);
}

/// The on-screen pause button's click — same effect as Escape (see
/// [`toggle_pause`]). It can only ever be clicked while unpaused, since the
/// pause overlay is a full-screen backdrop on top of it once visible, but
/// toggling (rather than only opening) keeps it a drop-in Escape equivalent.
fn on_pause_button_click(
    _: On<Pointer<Click>>,
    mut paused: ResMut<Paused>,
    mut overlay: Query<&mut Visibility, With<PauseMenuRoot>>,
    sinks: Query<&AudioSink, With<MusicPlayer>>,
) {
    toggle_pause(&mut paused, &mut overlay, &sinks);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── jam_fraction_for ─────────────────────────────────────────────────────

    fn stats(
        chord_tone: u32,
        in_scale: u32,
        out_of_scale: u32,
        rest_violations: u32,
    ) -> ImprovStats {
        ImprovStats {
            chord_tone,
            in_scale,
            out_of_scale,
            rest_violations,
        }
    }

    #[test]
    fn jam_fraction_for_reads_the_matching_stat() {
        let s = stats(3, 5, 2, 1);
        assert_eq!(
            jam_fraction_for(Some(&PassCriteria::ScaleAdherence { threshold: 0.1 }), &s),
            s.adherence()
        );
        assert_eq!(
            jam_fraction_for(
                Some(&PassCriteria::ChordToneAdherence { threshold: 0.1 }),
                &s
            ),
            s.chord_tone_adherence()
        );
        assert_eq!(
            jam_fraction_for(Some(&PassCriteria::PhraseDiscipline { threshold: 0.1 }), &s),
            s.phrase_discipline()
        );
    }

    #[test]
    fn jam_fraction_for_is_none_for_a_non_jam_criterion() {
        let s = stats(3, 5, 2, 1);
        assert_eq!(jam_fraction_for(None, &s), None);
        assert_eq!(
            jam_fraction_for(Some(&PassCriteria::Accuracy { threshold: 0.5 }), &s),
            None
        );
    }

    // A fresh keyboard with Escape registered as just-pressed this frame.
    fn escape_down() -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        keys
    }

    #[test]
    fn escape_pauses_then_resumes() {
        let mut world = World::new();
        world.insert_resource(Paused(false));
        world.insert_resource(escape_down());
        let overlay = world.spawn((PauseMenuRoot, Visibility::Hidden)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(handle_pause_input);

        // First Escape: pause + show overlay.
        schedule.run(&mut world);
        assert!(world.resource::<Paused>().0, "Escape should pause");
        assert_eq!(
            *world.get::<Visibility>(overlay).unwrap(),
            Visibility::Visible
        );

        // Second (fresh) Escape: resume + hide overlay.
        world.insert_resource(escape_down());
        schedule.run(&mut world);
        assert!(!world.resource::<Paused>().0, "Escape again should resume");
        assert_eq!(
            *world.get::<Visibility>(overlay).unwrap(),
            Visibility::Hidden
        );
    }

    fn pending_state(next: &NextState<AppState>) -> Option<AppState> {
        match next {
            NextState::Pending(s) => Some(s.clone()),
            _ => None,
        }
    }

    #[test]
    fn resume_button_unpauses_without_changing_state() {
        let mut paused = Paused(true);
        apply_resume(&mut paused);
        assert!(!paused.0);
    }

    #[test]
    fn restart_button_reloads_the_song() {
        let mut paused = Paused(true);
        let mut next = NextState::<AppState>::Unchanged;
        apply_restart(&mut paused, &mut next, AppState::SongLoading);
        assert!(!paused.0);
        assert_eq!(pending_state(&next), Some(AppState::SongLoading));
    }

    #[test]
    fn restart_can_target_playing_directly_for_a_generated_jam() {
        let mut paused = Paused(true);
        let mut next = NextState::<AppState>::Unchanged;
        apply_restart(&mut paused, &mut next, AppState::Playing);
        assert!(!paused.0);
        assert_eq!(pending_state(&next), Some(AppState::Playing));
    }

    #[test]
    fn quit_song_returns_to_the_song_list() {
        let mut paused = Paused(true);
        let mut next = NextState::<AppState>::Unchanged;
        let mut rtsl = ReturnToSongList(false);
        apply_quit(&mut paused, &mut next, &mut rtsl);
        assert!(!paused.0);
        assert_eq!(pending_state(&next), Some(AppState::Menu));
        assert!(rtsl.0, "should land on the song list");
    }

    // ── loop_label_text ───────────────────────────────────────────────────────
    //
    // `Localization::default()` has no bundle loaded, so `loc.msg(key)`/
    // `loc.msg_args(key, ...)` fall back to the key itself with no `%name%`
    // placeholders to substitute into — these only exercise which key (and,
    // for the active case, which args) the active/inactive dispatch picks,
    // not the translated/formatted text.

    #[test]
    fn loop_label_is_off_by_default() {
        let loc = Localization::default();
        assert_eq!(
            loop_label_text(&loc, &LoopConfig::default()),
            "pause-loop-off"
        );
    }

    #[test]
    fn loop_label_is_off_for_an_inactive_nonzero_range() {
        // A zero-width range (e.g. a degenerate drag) leaves start/end
        // nonzero but inactive — the readout should still read "off".
        let loc = Localization::default();
        let cfg = LoopConfig {
            active: false,
            start_time: 8.0,
            end_time: 8.0,
        };
        assert_eq!(loop_label_text(&loc, &cfg), "pause-loop-off");
    }

    #[test]
    fn loop_label_shows_the_range_once_active() {
        let loc = Localization::default();
        let cfg = LoopConfig {
            active: true,
            start_time: 8.0,
            end_time: 16.0,
        };
        assert_eq!(loop_label_text(&loc, &cfg), "pause-loop-range");
    }

    // ── practice_speed_label_text ──────────────────────────────────────────────

    #[test]
    fn practice_speed_label_picks_the_speed_key() {
        let loc = Localization::default();
        assert_eq!(practice_speed_label_text(&loc, 1.0), "pause-speed");
        assert_eq!(practice_speed_label_text(&loc, 0.7), "pause-speed");
    }

    // ── phrase selector / adaptive difficulty controls ────────────────────────

    #[test]
    fn phrase_selector_text_picks_the_section_key_when_named() {
        let loc = Localization::default();
        assert_eq!(
            phrase_selector_text(&loc, Some("intro"), 0.25),
            "pause-phrase-section"
        );
    }

    #[test]
    fn phrase_selector_text_handles_no_sections() {
        let loc = Localization::default();
        assert_eq!(
            phrase_selector_text(&loc, None, 0.0),
            "pause-phrase-no-sections"
        );
    }

    #[test]
    fn adaptive_difficulty_label_reflects_state() {
        let loc = Localization::default();
        assert_eq!(
            adaptive_difficulty_label_text(&loc, true),
            "pause-adaptive-difficulty-on"
        );
        assert_eq!(
            adaptive_difficulty_label_text(&loc, false),
            "pause-adaptive-difficulty-off"
        );
    }

    #[test]
    fn clamp_learned_clamps_to_zero_and_one() {
        assert_eq!(clamp_learned(-0.5), 0.0);
        assert_eq!(clamp_learned(1.5), 1.0);
        assert_eq!(clamp_learned(0.42), 0.42);
    }
}
