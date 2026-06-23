// SPDX-License-Identifier: MIT

//! The in-game pause overlay: a translucent menu with Resume / Restart / Quit,
//! toggled with Escape. Shares the gameplay [`Paused`] flag (every gameplay
//! chain gates on it) and pauses/resumes the song's audio sink.

use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;

use crate::menu::{AppState, ReturnToSongList};
use crate::dialogs::button;
use super::{GameplayRoot, MusicPlayer, Paused};

const BTN_IDLE: Color = Color::srgb(0.14, 0.14, 0.22);
const BTN_HOVER: Color = Color::srgb(0.20, 0.20, 0.32);

/// Root of the pause overlay; toggled between hidden/visible.
#[derive(Component, Default, Clone)]
pub(super) struct PauseMenuRoot;

/// Spawns the (initially hidden) pause overlay. Tagged `GameplayRoot` so it is
/// torn down with the rest of the scene. The whole tree — including each
/// button's click/hover behaviour — is authored declaratively with `bsn!`.
/// (Labels use the default font: `bsn!` can't set `TextFont.font` in 0.19.)
pub(super) fn setup_pause_menu(mut commands: Commands) {
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
        .insert(Visibility::Hidden);
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
}
