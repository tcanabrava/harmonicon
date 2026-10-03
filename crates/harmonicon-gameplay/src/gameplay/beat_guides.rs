// SPDX-License-Identifier: MIT

//! Horizontal beat and downbeat lines scrolling up the 2D highway, so the
//! player can see where the pulse is between notes instead of inferring it
//! from the notes themselves.
//!
//! **No second timing calculation.** A guide's position comes from exactly
//! the pieces that already place a note:
//!
//! - which ticks are beats — `bars::beat_ticks_in_range`, off
//!   `bars::chart_meter`, so 6/8 gets six eighths to the bar rather than six
//!   quarters;
//! - which ticks the visible window spans, and what second each beat falls
//!   on — `chart::seconds_to_tick`/`tick_to_seconds`, an inverse pair that
//!   already honours the chart's tempo map;
//! - where a second sits on screen — `gameplay_2d::note_attack_pct`,
//!   the same mapping `spawn_visible_notes` gives the notes.
//!
//! Derive any of those from a local `60.0 / bpm` instead and the guides
//! drift away from the notes on exactly the charts a visible pulse would
//! have been worth something on: the ones that change tempo.
//!
//! Entities are a fixed pool, allocated once and repositioned each frame. A
//! guide is a 1px line with no state of its own, so spawning and despawning
//! a handful every frame would be churn for nothing.

use bevy::prelude::*;

use harmonicon_app::app::SelectedSong;
use harmonicon_core::chart::{seconds_to_tick, tick_to_seconds};
use harmonicon_song::song::SongManifest;

use super::bars::{beat_ticks_in_range, chart_meter, pickup_lead_ticks, ticks_per_beat};
use super::clock::GameplayClock;
use super::gameplay_2d::note_attack_pct;
use super::notes::LOOKAHEAD;

/// One line in the pool. Which beat it shows changes every frame; the entity
/// does not.
#[derive(Component)]
pub struct BeatGuide;

/// How many guides to allocate. `LOOKAHEAD` is 3 s, so this covers a beat
/// every ~90 ms — 666 bpm in 4/4, or 166 bpm in 6/8 where a beat is an
/// eighth. Denser than that and the surplus beats simply aren't drawn, which
/// is the right failure: at that spacing the lines are a grey wash anyway.
const POOL: usize = 34;

const BEAT_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.055);
const DOWNBEAT_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.16);

/// Called by `gameplay_2d::setup`, the only place holding the highway entity
/// these have to be parented onto — their positions are percentages of it.
pub(super) fn spawn_beat_guides(commands: &mut Commands, highway: Entity) {
    commands.entity(highway).with_children(|hw| {
        for _ in 0..POOL {
            // **No `GlobalZIndex`.** `GlobalZIndex(0)` would seem to say
            // "behind the notes", but it is global: it drops these below
            // the gameplay root's own `GlobalZIndex(1)` background, which
            // then paints straight over them (the same trap
            // `gameplay_2d::spawn_gameplay_music_score` documents).
            // Ordinary child order already puts them behind the notes,
            // which are added to the highway later.
            // **No `GameplayRoot`.** That marker means "a top-level
            // entity `cleanup_gameplay` sweeps on exit"; these are
            // children of the highway, which is itself a descendant of
            // the `GameplayRoot` node `gameplay_2d::setup` spawns, so
            // that sweep already takes them via its recursive despawn.
            // Tagging them too put them in the sweep's own query as
            // well, and the second despawn then hit an entity its own
            // ancestor had just removed — one `Entity despawned`
            // warning per guide, every time you left Play 2D.
            hw.spawn_empty()
                .apply_scene(bsn! {
                    Node {
                        position_type: {PositionType::Absolute},
                        left: {Val::Percent(0.0)},
                        width: {Val::Percent(100.0)}, height: {Val::Px(1.0)},
                    }
                    BackgroundColor({BEAT_COLOR})
                    ~{Visibility::Hidden}
                    ~{Pickable::IGNORE}
                })
                .insert(BeatGuide);
        }
    });
}

/// Repositions the pool onto the beats currently inside the lookahead
/// window, hiding whatever is left over.
pub(super) fn update_beat_guides(
    clock: Res<GameplayClock>,
    selected: Res<SelectedSong>,
    manifests: Res<Assets<SongManifest>>,
    mut guides: Query<(&mut Node, &mut Visibility, &mut BackgroundColor), With<BeatGuide>>,
) {
    let mut guides = guides.iter_mut();
    let elapsed = clock.get();

    if let Some(manifest) = manifests.get(&selected.0) {
        let timing = &manifest.chart.timing;
        let meter = chart_meter(&manifest.chart);
        let per_beat = ticks_per_beat(timing.resolution, &meter);
        let to_tick =
            |secs: f64| seconds_to_tick(secs.max(0.0), timing.resolution, &timing.tempo_map);

        for (tick, is_downbeat) in beat_ticks_in_range(
            to_tick(elapsed),
            to_tick(elapsed.max(0.0) + LOOKAHEAD),
            per_beat,
            usize::from(meter.numerator.max(1)),
            pickup_lead_ticks(&manifest.chart),
        ) {
            let Some((mut node, mut visibility, mut color)) = guides.next() else {
                break;
            };
            let beat_time = tick_to_seconds(tick, timing.resolution, &timing.tempo_map);
            let bottom = note_attack_pct(beat_time, elapsed, LOOKAHEAD);
            if !(0.0..=100.0).contains(&bottom) {
                set_visibility(&mut visibility, Visibility::Hidden);
                continue;
            }
            node.bottom = Val::Percent(bottom);
            // Only the position moves every frame; the rest changes when a
            // pooled line switches between a beat and a downbeat or shows up.
            let height = Val::Px(if is_downbeat { 2.0 } else { 1.0 });
            if node.height != height {
                node.height = height;
            }
            let tint = if is_downbeat { DOWNBEAT_COLOR } else { BEAT_COLOR };
            if color.0 != tint {
                color.0 = tint;
            }
            set_visibility(&mut visibility, Visibility::Visible);
        }
    }

    // Whatever the pool didn't need this frame — including every guide while
    // the manifest is still loading.
    for (_, mut visibility, _) in guides {
        set_visibility(&mut visibility, Visibility::Hidden);
    }
}

/// Writes only on a change, so an unchanged guide doesn't re-run visibility
/// propagation every frame.
fn set_visibility(visibility: &mut Mut<Visibility>, wanted: Visibility) {
    if **visibility != wanted {
        **visibility = wanted;
    }
}
