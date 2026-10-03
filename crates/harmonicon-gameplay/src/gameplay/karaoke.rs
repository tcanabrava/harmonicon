// SPDX-License-Identifier: MIT

//! The karaoke strip: a chart's lyrics under the notation staff, two lines
//! at a time. The current line fills in syllable by syllable as each one's
//! note is played; the next line waits dimmed below it, so it can be read
//! ahead. Play 2D and 3D both show it, whenever the chart has lyrics
//! (`hud_panel::contextual_panels`).
//!
//! All the lyric logic — lines, word joins, where the singer is — is
//! `harmonicon_core::lyrics`; this module only draws its answer.

use bevy::prelude::*;

use harmonicon_app::app::{AppState, GameplayMode};
use harmonicon_core::lyrics::{KaraokePosition, LyricLine, karaoke_at};

use super::{GameplayClock, GameplayLogic, GameplayRoot};

const SUNG_COLOR: Color = Color::srgb(0.98, 0.80, 0.25);
const UNSUNG_COLOR: Color = Color::WHITE;
const NEXT_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.55);
const CURRENT_FONT: f32 = 26.0;
const NEXT_FONT: f32 = 18.0;

pub(super) struct KaraokePlugin;

impl Plugin for KaraokePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_karaoke
                .after(GameplayLogic)
                .run_if(in_state(AppState::Playing).and_then(playing_2d_or_3d)),
        );
    }
}

fn playing_2d_or_3d(mode: Res<GameplayMode>) -> bool {
    matches!(*mode, GameplayMode::Play2D | GameplayMode::Play3D)
}

/// The strip's root, holding the lines it draws and the position last drawn,
/// so text is only rewritten when a syllable is actually reached.
#[derive(Component)]
pub(super) struct KaraokeStrip {
    lines: Vec<LyricLine>,
    shown: Option<KaraokePosition>,
}

/// The sung half of the current line.
#[derive(Component, Default, Clone)]
struct KaraokeSung;

/// The rest of the current line.
#[derive(Component, Default, Clone)]
struct KaraokeUnsung;

/// The line after it.
#[derive(Component, Default, Clone)]
struct KaraokeNext;

/// Where the strip starts: under the song-progress bar, and under the staff
/// too when one is drawn.
pub(super) fn strip_top(staff: bool) -> f32 {
    let staff_height = if staff { harmonicon_ui::music_score::PANEL_HEIGHT } else { 0.0 };
    super::song_progress_overlay::BAR_HEIGHT + staff_height
}

/// Spawns the strip `top` px below the window's top edge, full width —
/// a sibling top-level entity at the staff's `GlobalZIndex(100)` for the
/// same reason as `gameplay_2d::spawn_gameplay_music_score`. Nothing is
/// spawned for a chart without lyrics.
pub(super) fn spawn_karaoke(commands: &mut Commands, lines: Vec<LyricLine>, top: f32) {
    if lines.is_empty() {
        return;
    }
    commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                top: {Val::Px(top)},
                left: {Val::Px(0.0)},
                width: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Column},
                align_items: {AlignItems::Center},
            }
            GlobalZIndex(100)
            GameplayRoot
            Children [
                Node {
                    padding: {UiRect::axes(Val::Px(14.0), Val::Px(4.0))},
                    flex_direction: {FlexDirection::Column},
                    align_items: {AlignItems::Center},
                }
                BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.45)})
                Children [
                    Text("")
                    TextFont { font_size: {FontSize::Px(CURRENT_FONT)} }
                    Children [
                        TextSpan("") TextFont { font_size: {FontSize::Px(CURRENT_FONT)} }
                        TextColor({SUNG_COLOR}) KaraokeSung
                        --
                        TextSpan("") TextFont { font_size: {FontSize::Px(CURRENT_FONT)} }
                        TextColor({UNSUNG_COLOR}) KaraokeUnsung
                    ]
                    --
                    Text("") TextFont { font_size: {FontSize::Px(NEXT_FONT)} }
                    TextColor({NEXT_COLOR}) KaraokeNext
                ]
            ]
        })
        .insert(KaraokeStrip { lines, shown: None });
}

/// Moves the highlight to wherever the clock is. Runs every frame but only
/// writes text when the karaoke position changes — a syllable reached, a
/// line turned, or a loop rewinding to an earlier one.
fn update_karaoke(
    clock: Res<GameplayClock>,
    mut strips: Query<&mut KaraokeStrip>,
    mut sung: Query<&mut TextSpan, (With<KaraokeSung>, Without<KaraokeUnsung>)>,
    mut unsung: Query<&mut TextSpan, (With<KaraokeUnsung>, Without<KaraokeSung>)>,
    mut next: Query<&mut Text, With<KaraokeNext>>,
) {
    let Ok(mut strip) = strips.single_mut() else {
        return;
    };
    let Some(position) = karaoke_at(&strip.lines, clock.get()) else {
        return;
    };
    if strip.shown == Some(position) {
        return;
    }
    strip.shown = Some(position);
    let (done, rest) = strip.lines[position.line].split_at(position.sung);
    if let Ok(mut span) = sung.single_mut() {
        span.0 = done;
    }
    if let Ok(mut span) = unsung.single_mut() {
        span.0 = rest;
    }
    if let Ok(mut text) = next.single_mut() {
        text.0 = strip.lines.get(position.line + 1).map(LyricLine::text).unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use harmonicon_core::lyrics::Syllable;

    fn line(words: &[(&str, f64)]) -> LyricLine {
        LyricLine {
            syllables: words
                .iter()
                .map(|&(text, start)| Syllable { text: text.into(), start, joins_next: false })
                .collect(),
        }
    }

    fn spans(world: &mut World) -> (String, String, String) {
        let sung =
            world.query_filtered::<&TextSpan, With<KaraokeSung>>().single(world).unwrap().0.clone();
        let unsung = world
            .query_filtered::<&TextSpan, With<KaraokeUnsung>>()
            .single(world)
            .unwrap()
            .0
            .clone();
        let next =
            world.query_filtered::<&Text, With<KaraokeNext>>().single(world).unwrap().0.clone();
        (sung, unsung, next)
    }

    #[test]
    fn the_strip_follows_the_clock() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::scene::ScenePlugin));
        let world = app.world_mut();
        world.insert_resource(GameplayClock::default());
        let lines =
            vec![line(&[("how", 1.0), ("sweet", 2.0)]), line(&[("the", 3.0), ("sound", 4.0)])];
        world
            .run_system_once(move |mut commands: Commands| {
                spawn_karaoke(&mut commands, lines.clone(), 0.0);
            })
            .unwrap();

        let at = |world: &mut World, t: f64| {
            world.resource_mut::<GameplayClock>().set_free(t);
            world.run_system_once(update_karaoke).unwrap();
            spans(world)
        };
        assert_eq!(at(world, 0.0), (String::new(), "how sweet".into(), "the sound".into()));
        assert_eq!(at(world, 1.5), ("how ".into(), "sweet".into(), "the sound".into()));
        assert_eq!(at(world, 3.5), ("the ".into(), "sound".into(), String::new()));
        // A loop rewinding the clock brings the first line back.
        assert_eq!(at(world, 1.0), ("how ".into(), "sweet".into(), "the sound".into()));
    }

    #[test]
    fn a_chart_without_lyrics_spawns_nothing() {
        let mut world = World::new();
        world
            .run_system_once(|mut commands: Commands| spawn_karaoke(&mut commands, Vec::new(), 0.0))
            .unwrap();
        assert_eq!(world.query::<&KaraokeStrip>().iter(&world).count(), 0);
    }
}
