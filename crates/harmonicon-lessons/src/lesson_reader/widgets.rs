// SPDX-License-Identifier: MIT

//! The runtime behind a lesson's interactive widgets — the metronome with
//! its patterns and audio, the grid / form / rhythm / phrase-looper
//! highlights that follow it, and the circle-of-fifths state — as opposed
//! to the page assembly in `mod.rs` that spawns them.

use super::*;

#[derive(Component)]
pub(crate) struct LessonMetronome {
    pub(super) clock: MetronomeClock,
    pub(super) bpm: f32,
    pub(super) displayed_bpm: u32,
    pub(super) initial_bpm: f32,
    pub(super) tempo_steps: Vec<f32>,
    pub(super) tempo_step: usize,
    pub(super) bars_per_step: usize,
    pub(super) pattern: LessonMetronomePattern,
    pub(super) beats_per_bar: usize,
    pub(super) muted: bool,
    pub(super) sync_group: Option<String>,
    pub(super) label: Entity,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LessonMetronomePattern {
    Straight,
    Shuffle,
    Triplet,
}

impl LessonMetronomePattern {
    pub(super) const fn clock_feel(self) -> MetronomeFeel {
        match self {
            Self::Straight => MetronomeFeel::Straight,
            Self::Shuffle | Self::Triplet => MetronomeFeel::Shuffle,
        }
    }

    pub(super) const fn click(self, tick: i64, beats_per_bar: f64) -> Option<(bool, f32)> {
        match self {
            Self::Straight => click_for_tick(tick, beats_per_bar, MetronomeFeel::Straight),
            Self::Shuffle => click_for_tick(tick, beats_per_bar, MetronomeFeel::Shuffle),
            Self::Triplet => Some((
                is_downbeat(tick.div_euclid(3), beats_per_bar) && tick.rem_euclid(3) == 0,
                if tick.rem_euclid(3) == 0 { 1.0 } else { 0.55 },
            )),
        }
    }

    pub(super) const fn next(self) -> Self {
        match self {
            Self::Straight => Self::Shuffle,
            Self::Shuffle => Self::Triplet,
            Self::Triplet => Self::Straight,
        }
    }

    pub(super) const fn ticks_per_beat(self) -> i64 {
        match self {
            Self::Straight => 1,
            Self::Shuffle | Self::Triplet => 3,
        }
    }
}

pub(super) fn scheduled_tempo(
    tick: i64,
    beats_per_bar: usize,
    bars_per_step: usize,
    pattern: LessonMetronomePattern,
    step: usize,
    tempo_steps: &[f32],
) -> Option<f32> {
    let ticks_per_step =
        beats_per_bar.max(1) as i64 * bars_per_step.max(1) as i64 * pattern.ticks_per_beat();
    (tick > 0 && tick.rem_euclid(ticks_per_step) == 0)
        .then(|| tempo_steps.get(step).copied())
        .flatten()
}

#[derive(Component)]
pub(crate) struct LessonGrid {
    pub(super) cells: Vec<Entity>,
    pub(super) key: String,
    pub(super) progression: Progression,
    pub(super) sync_group: Option<String>,
    pub(super) current_bar: usize,
}

#[derive(Component)]
pub(super) struct LessonFormMap {
    pub(super) cells: Vec<Entity>,
    pub(super) sections: Vec<String>,
    pub(super) current_section: usize,
}

#[derive(Component)]
pub(super) struct LessonRhythmPattern {
    pub(super) cells: Vec<Entity>,
    pub(super) steps: Vec<RhythmStep>,
    pub(super) current_step: usize,
}

#[derive(Component)]
pub(crate) struct LessonPhraseLooper {
    pub(super) clock: PhraseLoopClock,
    pub(super) cells: Vec<Entity>,
    pub(super) bpm: f32,
    pub(super) displayed_bpm: u32,
    pub(super) beats_per_step: f32,
    pub(super) label: Entity,
}

#[derive(Component)]
pub(crate) struct LessonMetronomeAudio;

pub(crate) fn cleanup_lesson_audio(
    mut commands: Commands,
    audio: Query<Entity, With<LessonMetronomeAudio>>,
) {
    for entity in &audio {
        commands.entity(entity).despawn();
    }
}

/// Paints each of `cells` with `color_for(index)`, writing only cells whose
/// colour actually changes — the highlights below repaint a whole row on
/// every step or beat, but only two cells move.
fn paint_cells(
    cells: &[Entity],
    backgrounds: &mut Query<&mut BackgroundColor>,
    mut color_for: impl FnMut(usize) -> Color,
) {
    for (index, entity) in cells.iter().enumerate() {
        if let Ok(mut bg) = backgrounds.get_mut(*entity) {
            let color = color_for(index);
            if bg.0 != color {
                bg.0 = color;
            }
        }
    }
}

pub(super) fn highlight_lesson_grid(
    grid: &LessonGrid,
    bar: usize,
    colors: harmonicon_platform::theme::TwelveBarColors,
    backgrounds: &mut Query<&mut BackgroundColor>,
) {
    paint_cells(&grid.cells, backgrounds, |index| {
        if index == bar {
            Color::srgba(0.75, 0.55, 0.08, 0.95)
        } else {
            bar_bg(index, &grid.key, grid.progression, colors)
        }
    });
}

pub(super) const fn stepped_bar(current: usize, delta: i32) -> usize {
    (current as i32 + delta).rem_euclid(12) as usize
}

pub(super) fn stepped_section(current: usize, delta: i32, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    (current as i32 + delta).rem_euclid(count as i32) as usize
}

pub(super) fn highlight_form_section(
    map: &LessonFormMap,
    section: usize,
    backgrounds: &mut Query<&mut BackgroundColor>,
) {
    paint_cells(&map.cells, backgrounds, |index| {
        if index == section {
            Color::srgba(0.82, 0.62, 0.10, 1.0)
        } else {
            section_bg(&map.sections[index])
        }
    });
}

pub(super) fn highlight_rhythm_step(
    pattern: &LessonRhythmPattern,
    selected: usize,
    backgrounds: &mut Query<&mut BackgroundColor>,
) {
    paint_cells(&pattern.cells, backgrounds, |index| {
        if index == selected { RHYTHM_ACTIVE_BG } else { step_bg(&pattern.steps[index]) }
    });
}

pub(super) fn highlight_phrase_step(
    looper: &LessonPhraseLooper,
    selected: usize,
    backgrounds: &mut Query<&mut BackgroundColor>,
) {
    paint_cells(&looper.cells, backgrounds, |index| {
        if index == selected { LOOP_ACTIVE_BG } else { LOOP_CELL_BG }
    });
}

pub(crate) fn update_lesson_phrase_loopers(
    time: Res<Time>,
    mut loopers: Query<&mut LessonPhraseLooper>,
    mut labels: Query<&mut Text>,
    mut backgrounds: Query<&mut BackgroundColor>,
) {
    for mut looper in &mut loopers {
        let bpm_label = looper.bpm as u32;
        if looper.displayed_bpm != bpm_label {
            if let Ok(mut text) = labels.get_mut(looper.label) {
                *text = Text::new(format!("♩ = {bpm_label}"));
            }
            looper.displayed_bpm = bpm_label;
        }
        let bpm = looper.bpm;
        let beats_per_step = looper.beats_per_step;
        let step_count = looper.cells.len();
        if let Some(selected) = looper.clock.advance(
            time.delta_secs_f64(),
            f64::from(bpm),
            f64::from(beats_per_step),
            step_count,
        ) {
            highlight_phrase_step(&looper, selected, &mut backgrounds);
        }
    }
}

#[derive(Component)]
pub(super) struct LessonCircle {
    pub(super) diagram: Entity,
    pub(super) host: Entity,
    pub(super) harp_key: String,
    pub(super) positions: Vec<Position>,
}

pub(crate) fn update_lesson_metronomes(
    time: Res<Time>,
    mut metronomes: Query<&mut LessonMetronome>,
    mut grids: Query<&mut LessonGrid>,
    mut labels: Query<&mut Text>,
    mut backgrounds: Query<&mut BackgroundColor>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioSettings>,
    theme: Res<LoadedTheme>,
    mut commands: Commands,
) {
    for mut metronome in &mut metronomes {
        let bpm = metronome.bpm;
        let pattern = metronome.pattern;
        let feel = pattern.clock_feel();
        let tick = metronome
            .clock
            // A lesson's metronome widget has a BPM and a beat count and
            // no meter, so its beat is its BPM beat — `60 / bpm` is the
            // honest answer here, not an x/4 assumption.
            .advance(time.delta_secs_f64(), 60.0 / f64::from(bpm), feel);
        let bpm_label = bpm as u32;
        if metronome.displayed_bpm != bpm_label {
            if let Ok(mut text) = labels.get_mut(metronome.label) {
                *text = Text::new(format!("\u{2669} = {bpm_label}"));
            }
            metronome.displayed_bpm = bpm_label;
        }
        let Some(tick) = tick else { continue };
        if !metronome.muted
            && let Some((accent, gain)) = pattern.click(tick, metronome.beats_per_bar as f64)
        {
            let sample =
                if accent { "sounds/metronome_high.ogg" } else { "sounds/metronome_low.ogg" };
            commands.spawn((
                AudioPlayer::<AudioSource>(asset_server.load(sample)),
                PlaybackSettings::DESPAWN
                    .with_volume(Volume::Linear(audio.metronome_volume * gain)),
                LessonMetronomeAudio,
            ));
        }
        if tick == 0 || (feel == MetronomeFeel::Shuffle && tick.rem_euclid(3) != 0) {
            continue;
        }
        let bar = twelve_bar_for_tick(tick, metronome.beats_per_bar, feel);
        for mut grid in &mut grids {
            if grid.sync_group.is_none() || grid.sync_group != metronome.sync_group {
                continue;
            }
            grid.current_bar = bar;
            highlight_lesson_grid(&grid, bar, theme.twelve_bar_colors(), &mut backgrounds);
        }
        if let Some(next_bpm) = scheduled_tempo(
            tick,
            metronome.beats_per_bar,
            metronome.bars_per_step,
            pattern,
            metronome.tempo_step,
            &metronome.tempo_steps,
        ) {
            metronome.bpm = next_bpm;
            metronome.tempo_step += 1;
            metronome.clock.reset();
            metronome.clock.running = true;
        }
    }
}
