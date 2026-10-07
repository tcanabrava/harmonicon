// SPDX-License-Identifier: MIT

//! Personal catalog columns: clickable ratings, mastery, and play counts.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button as WidgetButton};
use harmonicon_app::profile::{PlayerProfile, SongPreferences, save_profile};
use harmonicon_platform::song_library::song_identity;

use super::SongPickerState;

pub(super) fn preferences(profile: &PlayerProfile, path: &str) -> SongPreferences {
    profile.song_preferences.get(song_identity(path)).cloned().unwrap_or_default()
}

#[derive(Component, Clone)]
pub(crate) struct PersonalRating(pub(super) String);

#[derive(Component, Clone)]
pub(crate) enum PersonalStat {
    Mastery(String),
    TimesPlayed(String),
}

pub(super) fn rating_image(server: Option<&AssetServer>, rating: u8) -> Handle<Image> {
    server.map_or_else(Handle::default, |server| {
        server.load(format!("ui/ratings/{}.png", rating.min(10)))
    })
}

#[derive(Component, Clone)]
pub(super) struct RatingChoice {
    pub(super) path: String,
    pub(super) half_stars: u8,
}

fn rating_choice(path: String, half_stars: u8) -> impl Scene {
    bsn! {
        ~{RatingChoice { path, half_stars }}
        WidgetButton
        TabIndex(0)
        Node {
            width: {Val::Percent(10.0)},
            height: {Val::Percent(100.0)},
            flex_shrink: {0.0_f32},
        }
        on(set_rating_choice)
    }
}

pub(super) fn rating_cell(path: String, image: Handle<Image>) -> impl Scene {
    let personal = PersonalRating(path.clone());
    bsn! {
        Node {
            width: {Val::Percent(16.0)},
            flex_shrink: {0.0_f32},
            padding: {UiRect::axes(Val::Px(12.0), Val::Px(12.0))},
            overflow: {Overflow::clip()},
        }
        Children [
            Node { width: {Val::Px(90.0)}, height: {Val::Px(18.0)} }
            Children [
                ~{personal}
                ImageNode { image: {image} }
                Node {
                    position_type: {PositionType::Absolute},
                    width: {Val::Percent(100.0)}, height: {Val::Percent(100.0)},
                }
                ~{bevy::picking::Pickable::IGNORE}
                --
                @rating_choice(path.clone(), 1)
                --
                @rating_choice(path.clone(), 2)
                --
                @rating_choice(path.clone(), 3)
                --
                @rating_choice(path.clone(), 4)
                --
                @rating_choice(path.clone(), 5)
                --
                @rating_choice(path.clone(), 6)
                --
                @rating_choice(path.clone(), 7)
                --
                @rating_choice(path.clone(), 8)
                --
                @rating_choice(path.clone(), 9)
                --
                @rating_choice(path.clone(), 10)
            ]
        ]
    }
}

/// The first click selects a different row. Subsequent clicks set its rating;
/// clicking the current value again clears it back to zero stars.
pub(super) fn choose_rating(
    state: &mut SongPickerState,
    profile: &mut PlayerProfile,
    choice: &RatingChoice,
) -> bool {
    if state.selected.as_deref() != Some(choice.path.as_str()) {
        state.selected = Some(choice.path.clone());
        return false;
    }
    let record =
        profile.song_preferences.entry(song_identity(&choice.path).to_owned()).or_default();
    record.rating = if record.rating == choice.half_stars { 0 } else { choice.half_stars };
    true
}

fn set_rating_choice(
    event: On<Activate>,
    choices: Query<&RatingChoice>,
    mut state: ResMut<SongPickerState>,
    mut profile: ResMut<PlayerProfile>,
) {
    let Ok(choice) = choices.get(event.entity) else { return };
    if choose_rating(&mut state, &mut profile, choice) {
        save_profile(&profile);
    }
}

pub(super) fn mastery_label(personal: &SongPreferences) -> String {
    personal.last_accuracy.map_or_else(|| "—".into(), |a| format!("{:.0}%", a * 100.0))
}

pub(super) fn personal_stat_cell(stat: PersonalStat, label: String) -> impl Scene {
    bsn! {
        Node {
            width: {Val::Percent(10.0)},
            flex_shrink: {0.0_f32},
            padding: {UiRect::axes(Val::Px(12.0), Val::Px(12.0))},
            overflow: {Overflow::clip()},
        }
        ~{bevy::picking::Pickable::IGNORE}
        Children [
            ~{stat}
            Text({label})
            TextFont { font_size: {FontSize::Px(15.0)} }
            TextColor({Color::srgb(0.76, 0.80, 0.87)})
            ~{bevy::picking::Pickable::IGNORE}
        ]
    }
}
