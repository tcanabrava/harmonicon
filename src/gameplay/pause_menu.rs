// SPDX-License-Identifier: MIT

//! The in-game pause overlay: a translucent menu with Resume / Restart / Quit,
//! toggled with Escape. Shares the gameplay [`Paused`] flag (every gameplay
//! chain gates on it) and pauses/resumes the song's audio sink.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use super::adaptive_difficulty::AdaptiveDifficulty;
use super::jam_session::ImprovStats;
use super::{GameplayRoot, LoopConfig, MusicPlayer, Paused};
use crate::dialogs::button;
use crate::lessons::{LessonContext, lesson_passed};
use crate::menu::{AppState, GameplayMode, ReturnToSongList, SelectedSong};
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

/// Keeps the "Wait for Note: ..." readout in step with the toggle. Not
/// gated on `Paused` — the button only lives on the (otherwise hidden)
/// pause overlay, so it can only be clicked while already paused, same as
/// `apply_music_volume` intentionally keeps running through a pause.
pub(super) fn update_wait_mode_label(
    wait_mode: Res<WaitForNoteMode>,
    mut labels: Query<&mut Text, With<WaitForNoteLabel>>,
) {
    if !wait_mode.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(if wait_mode.0 {
            "Wait for Note: on"
        } else {
            "Wait for Note: off"
        });
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

/// Discrete steps a click cycles through, fastest first.
const SPEED_STEPS: [f32; 6] = [1.0, 0.9, 0.8, 0.7, 0.6, 0.5];

/// The step after `current` in [`SPEED_STEPS`], wrapping back to the first.
fn next_speed_step(current: f32) -> f32 {
    let idx = SPEED_STEPS
        .iter()
        .position(|&s| (s - current).abs() < 1e-6)
        .unwrap_or(SPEED_STEPS.len() - 1);
    SPEED_STEPS[(idx + 1) % SPEED_STEPS.len()]
}

fn on_cycle_practice_speed(_: On<Pointer<Click>>, mut speed: ResMut<PracticeSpeed>) {
    speed.0 = next_speed_step(speed.0);
}

/// The "Speed: ..." readout, kept in step with [`PracticeSpeed`].
#[derive(Component, Default, Clone)]
pub(super) struct PracticeSpeedLabel;

fn practice_speed_label_text(speed: f32) -> String {
    format!("Speed: {:.0}%", speed * 100.0)
}

/// Keeps the "Speed: ..." readout in step with [`PracticeSpeed`]. Not gated
/// on `Paused`, same reasoning as `update_wait_mode_label`.
pub(super) fn update_practice_speed_label(
    speed: Res<PracticeSpeed>,
    mut labels: Query<&mut Text, With<PracticeSpeedLabel>>,
) {
    if !speed.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(practice_speed_label_text(speed.0));
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
fn loop_label_text(cfg: &LoopConfig) -> String {
    if cfg.active {
        format!("Loop: {:.0}s\u{2013}{:.0}s", cfg.start_time, cfg.end_time)
    } else {
        "Loop: off".to_string()
    }
}

/// Keeps the "Loop: ..." readout in step with [`LoopConfig`]. Not gated on
/// `Paused`, same reasoning as `update_wait_mode_label`.
pub(super) fn update_loop_label(
    loop_cfg: Res<LoopConfig>,
    mut labels: Query<&mut Text, With<LoopRangeLabel>>,
) {
    if !loop_cfg.is_changed() {
        return;
    }
    for mut text in &mut labels {
        *text = Text::new(loop_label_text(&loop_cfg));
    }
}

// ── Adaptive difficulty controls ──────────────────────────────────────────────

/// Which of `AdaptiveDifficulty::sections` the pause menu's phrase selector
/// is currently showing/editing. Not reset between restarts (like
/// `WaitForNoteMode`/`PracticeSpeed`) — picking up where you left off is more
/// useful than always snapping back to the first phrase.
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

/// Next index in `0..count`, wrapping — same wraparound style as
/// `next_speed_step`. `0` for an empty (no-phrase-tags-yet-loaded) song.
fn next_phrase_index(current: usize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    (current + 1) % count
}

/// Previous index in `0..count`, wrapping.
fn prev_phrase_index(current: usize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    (current + count - 1) % count
}

fn on_prev_phrase(
    _: On<Pointer<Click>>,
    adaptive: Res<AdaptiveDifficulty>,
    mut selected: ResMut<SelectedPhraseIndex>,
) {
    selected.0 = prev_phrase_index(selected.0, adaptive.sections.len());
}

fn on_next_phrase(
    _: On<Pointer<Click>>,
    adaptive: Res<AdaptiveDifficulty>,
    mut selected: ResMut<SelectedPhraseIndex>,
) {
    selected.0 = next_phrase_index(selected.0, adaptive.sections.len());
}

/// Pure so the readout is unit-testable without a live `AdaptiveDifficulty`.
fn phrase_selector_text(name: Option<&str>, learned: f32) -> String {
    match name {
        Some(name) => format!("Section: {name} \u{2014} Learned: {:.0}%", learned * 100.0),
        None => "No phrases in this song".to_string(),
    }
}

/// Keeps the phrase-selector readout in step with `SelectedPhraseIndex`/
/// `AdaptiveDifficulty`. Not gated on `Paused`, same reasoning as
/// `update_wait_mode_label`.
pub(super) fn update_phrase_selector_label(
    selected: Res<SelectedPhraseIndex>,
    adaptive: Res<AdaptiveDifficulty>,
    mut labels: Query<&mut Text, With<PhraseSelectorLabel>>,
) {
    if !selected.is_changed() && !adaptive.is_changed() {
        return;
    }
    let section = adaptive.sections.get(selected.0);
    let learned = section
        .map(|_| adaptive.learned.get(selected.0).copied().unwrap_or(0.0))
        .unwrap_or(0.0);
    let text = phrase_selector_text(section.map(|s| s.name.as_str()), learned);
    for mut label in &mut labels {
        *label = Text::new(text.clone());
    }
}

fn adaptive_difficulty_label_text(enabled: bool) -> String {
    format!("Adaptive Difficulty: {}", if enabled { "on" } else { "off" })
}

pub(super) fn update_adaptive_difficulty_label(
    adaptive: Res<AdaptiveDifficulty>,
    mut labels: Query<&mut Text, With<AdaptiveDifficultyLabel>>,
) {
    if !adaptive.is_changed() {
        return;
    }
    for mut label in &mut labels {
        *label = Text::new(adaptive_difficulty_label_text(adaptive.enabled));
    }
}

fn on_toggle_adaptive_difficulty(
    _: On<Pointer<Click>>,
    selected_song: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut profile: ResMut<PlayerProfile>,
    mut adaptive: ResMut<AdaptiveDifficulty>,
) {
    adaptive.enabled = !adaptive.enabled;
    if let Some(key) = song_key(&selected_song, &manifests) {
        profile.songs.entry(key).or_default().adaptive_difficulty_enabled = adaptive.enabled;
        crate::profile::save_profile(&profile);
    }
}

/// Clamps a learned fraction into `0.0..=1.0` after applying `delta` — split
/// out for unit testing without the ECS plumbing.
fn adjust_learned(current: f32, delta: f32) -> f32 {
    (current + delta).clamp(0.0, 1.0)
}

/// Shared by the "-25%"/"+25%" buttons: adjusts the selected phrase's
/// learned fraction in both the live `AdaptiveDifficulty` (so the progress
/// bar's rectangle re-tints immediately, and so `resync_notes_on_adaptive_
/// change` in `gameplay_2d`/`gameplay_3d` rebuilds the note highway on the
/// very next frame) and the persisted `PlayerProfile` (so it survives a
/// restart too).
fn adjust_selected_phrase_learned(
    delta: f32,
    selected: &SelectedPhraseIndex,
    selected_song: &SelectedSong,
    manifests: &Assets<SongManifest>,
    profile: &mut PlayerProfile,
    adaptive: &mut AdaptiveDifficulty,
) {
    if selected.0 >= adaptive.sections.len() {
        return;
    }
    if adaptive.learned.len() <= selected.0 {
        adaptive.learned.resize(selected.0 + 1, 0.0);
    }
    let new_value = adjust_learned(adaptive.learned[selected.0], delta);
    adaptive.learned[selected.0] = new_value;
    let Some(key) = song_key(selected_song, manifests) else {
        return;
    };
    let record = profile.songs.entry(key).or_default();
    if record.phrase_learned.len() <= selected.0 {
        record.phrase_learned.resize(selected.0 + 1, 0.0);
    }
    record.phrase_learned[selected.0] = new_value;
    crate::profile::save_profile(profile);
}

fn on_decrease_phrase_learned(
    _: On<Pointer<Click>>,
    selected: Res<SelectedPhraseIndex>,
    selected_song: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut profile: ResMut<PlayerProfile>,
    mut adaptive: ResMut<AdaptiveDifficulty>,
) {
    adjust_selected_phrase_learned(
        -0.25,
        &selected,
        &selected_song,
        &manifests,
        &mut profile,
        &mut adaptive,
    );
}

fn on_increase_phrase_learned(
    _: On<Pointer<Click>>,
    selected: Res<SelectedPhraseIndex>,
    selected_song: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut profile: ResMut<PlayerProfile>,
    mut adaptive: ResMut<AdaptiveDifficulty>,
) {
    adjust_selected_phrase_learned(
        0.25,
        &selected,
        &selected_song,
        &manifests,
        &mut profile,
        &mut adaptive,
    );
}

/// Spawns the (initially hidden) pause overlay. Tagged `GameplayRoot` so it is
/// torn down with the rest of the scene. The whole tree — including each
/// button's click/hover behaviour — is authored declaratively with `bsn!`.
/// (Labels use the default font: `bsn!` can't set `TextFont.font` in 0.19.)
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
) {
    let is_jam = *mode == GameplayMode::JamSession;
    // A scale-adherence lesson (see `PassCriteria::ScaleAdherence`) is the
    // one lesson type that never reaches the results screen on its own —
    // Jam Session has no natural end — so it needs its own explicit
    // "submit for judgment" action here instead.
    let is_lesson_jam = is_jam && lesson.is_some();
    commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                width: {Val::Percent(100.0)},
                height: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
                justify_content: {JustifyContent::Center},
                row_gap: {Val::Px(20.0)},
            }
            BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.65)})
            GlobalZIndex(200)
            GameplayRoot
            PauseMenuRoot
            Children [
                (
                    Text({"PAUSED"})
                    TextFont { font_size: {FontSize::Px(52.0)} }
                    TextColor({Color::WHITE})
                ),
                button::default("Resume", on_resume),
                button::default("Restart", on_restart),
                button::default("Quit Song", on_quit),
            ]
        })
        // bsn! can't express the `Visibility::Hidden` enum variant; set it here.
        .insert(Visibility::Hidden)
        .with_children(|children| {
            if is_lesson_jam {
                children
                    .spawn_empty()
                    .apply_scene(button::default("Finish Lesson", on_finish_lesson));
            }
            if !is_jam {
                children.spawn_empty().apply_scene(bsn! {
                    Node {
                        flex_direction: {FlexDirection::Row},
                        align_items: {AlignItems::Center},
                        column_gap: {Val::Px(8.0)},
                    }
                    Children [
                        button::small("\u{23F8} Wait for Note", on_toggle_wait_mode),
                        (
                            Text({"Wait for Note: off"})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.70, 0.70, 0.80)})
                            WaitForNoteLabel
                        ),
                    ]
                });
                children.spawn_empty().apply_scene(bsn! {
                    Node {
                        flex_direction: {FlexDirection::Row},
                        align_items: {AlignItems::Center},
                        column_gap: {Val::Px(8.0)},
                    }
                    Children [
                        button::small("\u{1F422} Speed", on_cycle_practice_speed),
                        (
                            Text({"Speed: 100%"})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.70, 0.70, 0.80)})
                            PracticeSpeedLabel
                        ),
                    ]
                });
                children.spawn_empty().apply_scene(bsn! {
                    Node {
                        flex_direction: {FlexDirection::Row},
                        align_items: {AlignItems::Center},
                        column_gap: {Val::Px(8.0)},
                    }
                    Children [
                        button::small("Adaptive Difficulty", on_toggle_adaptive_difficulty),
                        (
                            Text({"Adaptive Difficulty: on"})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.70, 0.70, 0.80)})
                            AdaptiveDifficultyLabel
                        ),
                    ]
                });
                children.spawn_empty().apply_scene(bsn! {
                    Node {
                        flex_direction: {FlexDirection::Row},
                        align_items: {AlignItems::Center},
                        column_gap: {Val::Px(8.0)},
                    }
                    Children [
                        button::small("\u{25C0}", on_prev_phrase),
                        (
                            Text({"No phrases in this song"})
                            TextFont { font_size: {FontSize::Px(15.0)} }
                            TextColor({Color::srgb(0.70, 0.70, 0.80)})
                            PhraseSelectorLabel
                        ),
                        button::small("\u{25B6}", on_next_phrase),
                        button::small("-25%", on_decrease_phrase_learned),
                        button::small("+25%", on_increase_phrase_learned),
                    ]
                });
                children.spawn_empty().apply_scene(bsn! {
                    Text({"Notes update live — resume to see them"})
                    TextFont { font_size: {FontSize::Px(13.0)} }
                    TextColor({Color::srgb(0.55, 0.55, 0.62)})
                });
            }

            children.spawn_empty().apply_scene(bsn! {
                Node {
                    flex_direction: {FlexDirection::Row},
                    align_items: {AlignItems::Center},
                    column_gap: {Val::Px(8.0)},
                }
                Children [
                    button::small("Clear Loop", on_clear_loop),
                    (
                        Text({"Loop: off"})
                        TextFont { font_size: {FontSize::Px(15.0)} }
                        TextColor({Color::srgb(0.70, 0.70, 0.80)})
                        LoopRangeLabel
                    ),
                ]
            });
            children.spawn_empty().apply_scene(bsn! {
                Text({"Drag on the progress bar above to set a loop range"})
                TextFont { font_size: {FontSize::Px(15.0)} }
                TextColor({Color::srgb(0.55, 0.55, 0.62)})
            });
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
) {
    apply_restart(&mut paused, &mut next_state);
}

fn on_quit(
    _: On<Pointer<Click>>,
    mut paused: ResMut<Paused>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
) {
    apply_quit(&mut paused, &mut next_state, &mut return_to_song_list);
}

/// Judges a scale-adherence lesson on demand — the only lesson type with no
/// natural end to judge it at (see `PassCriteria::ScaleAdherence`). Records
/// the result and returns to the menu the same way "Quit Song" does;
/// `route_menu_entry` sees the still-present `LessonContext` and routes to
/// the lesson list from there, same as any other lesson.
fn on_finish_lesson(
    _: On<Pointer<Click>>,
    lesson: Res<LessonContext>,
    improv_stats: Res<ImprovStats>,
    mut profile: ResMut<PlayerProfile>,
    mut paused: ResMut<Paused>,
    mut next_state: ResMut<NextState<AppState>>,
    mut return_to_song_list: ResMut<ReturnToSongList>,
) {
    let adherence = improv_stats.adherence();
    let passed = lesson_passed(lesson.pass_criteria.as_ref(), 0.0, &[], adherence);
    let record = profile.lessons.entry(lesson.lesson_id.clone()).or_default();
    record_lesson(record, passed, adherence.unwrap_or(0.0));
    save_profile(&profile);
    apply_quit(&mut paused, &mut next_state, &mut return_to_song_list);
}

// Pure effects, split out so they can be unit-tested without the UI/observers.
fn apply_resume(paused: &mut Paused) {
    paused.0 = false;
}

fn apply_restart(paused: &mut Paused, next_state: &mut NextState<AppState>) {
    paused.0 = false;
    // Re-enter via SongLoading so the whole song setup runs fresh (the asset is
    // already loaded, so it resumes immediately).
    next_state.set(AppState::SongLoading);
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
    paused.0 = !paused.0;
    for mut vis in &mut overlay {
        *vis = if paused.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for sink in &sinks {
        if paused.0 {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        apply_restart(&mut paused, &mut next);
        assert!(!paused.0);
        assert_eq!(pending_state(&next), Some(AppState::SongLoading));
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

    #[test]
    fn loop_label_is_off_by_default() {
        assert_eq!(loop_label_text(&LoopConfig::default()), "Loop: off");
    }

    #[test]
    fn loop_label_is_off_for_an_inactive_nonzero_range() {
        // A zero-width range (e.g. a degenerate drag) leaves start/end
        // nonzero but inactive — the readout should still read "off".
        let cfg = LoopConfig {
            active: false,
            start_time: 8.0,
            end_time: 8.0,
        };
        assert_eq!(loop_label_text(&cfg), "Loop: off");
    }

    #[test]
    fn loop_label_shows_the_range_once_active() {
        let cfg = LoopConfig {
            active: true,
            start_time: 8.0,
            end_time: 16.0,
        };
        assert_eq!(loop_label_text(&cfg), "Loop: 8s\u{2013}16s");
    }

    // ── next_speed_step / practice_speed_label_text ───────────────────────────

    #[test]
    fn next_speed_step_walks_down_from_full_speed() {
        assert_eq!(next_speed_step(1.0), 0.9);
        assert_eq!(next_speed_step(0.9), 0.8);
        assert_eq!(next_speed_step(0.6), 0.5);
    }

    #[test]
    fn next_speed_step_wraps_back_to_full_speed() {
        assert_eq!(next_speed_step(0.5), 1.0);
    }

    #[test]
    fn next_speed_step_defaults_to_the_slowest_step_for_an_unknown_value() {
        // Shouldn't happen — `PracticeSpeed` only ever holds a `SPEED_STEPS`
        // value — but stay well-defined rather than panicking.
        assert_eq!(next_speed_step(0.42), 1.0);
    }

    #[test]
    fn practice_speed_label_formats_as_a_percentage() {
        assert_eq!(practice_speed_label_text(1.0), "Speed: 100%");
        assert_eq!(practice_speed_label_text(0.7), "Speed: 70%");
    }

    // ── phrase selector / adaptive difficulty controls ────────────────────────

    #[test]
    fn next_phrase_index_wraps_around() {
        assert_eq!(next_phrase_index(0, 3), 1);
        assert_eq!(next_phrase_index(2, 3), 0);
    }

    #[test]
    fn prev_phrase_index_wraps_around() {
        assert_eq!(prev_phrase_index(0, 3), 2);
        assert_eq!(prev_phrase_index(1, 3), 0);
    }

    #[test]
    fn phrase_index_stepping_is_a_no_op_with_no_sections() {
        assert_eq!(next_phrase_index(0, 0), 0);
        assert_eq!(prev_phrase_index(0, 0), 0);
    }

    #[test]
    fn phrase_selector_text_shows_name_and_learned_percent() {
        assert_eq!(
            phrase_selector_text(Some("intro"), 0.25),
            "Section: intro \u{2014} Learned: 25%"
        );
    }

    #[test]
    fn phrase_selector_text_handles_no_sections() {
        assert_eq!(phrase_selector_text(None, 0.0), "No phrases in this song");
    }

    #[test]
    fn adaptive_difficulty_label_reflects_state() {
        assert_eq!(adaptive_difficulty_label_text(true), "Adaptive Difficulty: on");
        assert_eq!(adaptive_difficulty_label_text(false), "Adaptive Difficulty: off");
    }

    #[test]
    fn adjust_learned_clamps_to_zero_and_one() {
        assert_eq!(adjust_learned(0.0, -0.25), 0.0);
        assert_eq!(adjust_learned(1.0, 0.25), 1.0);
        assert!((adjust_learned(0.5, 0.25) - 0.75).abs() < 1e-6);
    }
}
