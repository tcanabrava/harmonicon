// SPDX-License-Identifier: MIT

//! Small always-on badges naming whichever practice aids are active — slowed
//! speed, wait-for-note, an A–B loop — in the live HUD panel.
//!
//! The aids are switched on from the pause menu and then that menu goes
//! away. A player who resumes at 70% with wait-for-note on sees a song that
//! stops at every note and plays no music (speed below 100% pauses the sink;
//! see `clock::tick_clock`), and nothing on screen says why. These say why.
//!
//! Only active aids are shown. A row of "off" badges is furniture; a single
//! "70% · music off" is information.

use bevy::prelude::*;

use harmonicon_platform::localization::{Localization, LocalizationExt};

use super::pause_menu::{PracticeSpeed, WaitForNoteMode};
use super::state::LoopConfig;

#[derive(Component)]
pub struct SpeedBadge;
#[derive(Component)]
pub struct WaitBadge;
#[derive(Component)]
pub struct LoopBadge;

const BADGE_BG: Color = Color::srgba(1.0, 0.85, 0.35, 0.16);
const BADGE_FG: Color = Color::srgb(1.0, 0.88, 0.50);

/// Spawns the three badges, all hidden, as a row under `parent`.
pub(super) fn spawn_practice_badges(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            for marker in [BadgeKind::Speed, BadgeKind::Wait, BadgeKind::Loop] {
                let mut badge = row.spawn_empty();
                badge.apply_scene(bsn! {
                    Node { padding: {UiRect::axes(Val::Px(8.0), Val::Px(3.0))} }
                    BackgroundColor({BADGE_BG})
                    ~{Visibility::Hidden}
                    Children [
                        Text("")
                        TextFont { font_size: {FontSize::Px(13.0)} }
                        TextColor({BADGE_FG})
                    ]
                });
                match marker {
                    BadgeKind::Speed => badge.insert(SpeedBadge),
                    BadgeKind::Wait => badge.insert(WaitBadge),
                    BadgeKind::Loop => badge.insert(LoopBadge),
                };
            }
        });
}

enum BadgeKind {
    Speed,
    Wait,
    Loop,
}

/// Rewrites the badges when any aid changes. Cheap enough to run on every
/// change rather than diffing which one moved: three resources, three
/// strings.
pub(super) fn update_practice_badges(
    speed: Res<PracticeSpeed>,
    wait: Res<WaitForNoteMode>,
    looping: Res<LoopConfig>,
    loc: Res<Localization>,
    mut speed_badge: Query<
        (&mut Visibility, &Children),
        (With<SpeedBadge>, Without<WaitBadge>, Without<LoopBadge>),
    >,
    mut wait_badge: Query<
        (&mut Visibility, &Children),
        (With<WaitBadge>, Without<SpeedBadge>, Without<LoopBadge>),
    >,
    mut loop_badge: Query<
        (&mut Visibility, &Children),
        (With<LoopBadge>, Without<SpeedBadge>, Without<WaitBadge>),
    >,
    mut texts: Query<&mut Text>,
) {
    if !(speed.is_changed() || wait.is_changed() || looping.is_changed()) {
        return;
    }

    let mut set = |visibility: &mut Visibility, children: &Children, label: Option<String>| {
        *visibility = if label.is_some() { Visibility::Visible } else { Visibility::Hidden };
        if let Some(label) = label {
            for child in children.iter() {
                if let Ok(mut text) = texts.get_mut(child) {
                    *text = Text::new(label.clone());
                }
            }
        }
    };

    // `PracticeSpeed` is a standing preference, so it can already be below
    // 100% on the first frame of a song — hence "changed" rather than a
    // toggle, and no assumption that it starts at 1.0.
    let speed_label = (speed.0 < 0.999).then(|| {
        String::from(
            loc.msg_args("gameplay-badge-speed", &[("pct", format!("{:.0}", speed.0 * 100.0))]),
        )
    });
    let wait_label = wait.0.then(|| String::from(loc.msg("gameplay-badge-wait")));
    let loop_label = looping.active.then(|| {
        String::from(loc.msg_args(
            "gameplay-badge-loop",
            &[
                ("start", format!("{:.0}", looping.start_time)),
                ("end", format!("{:.0}", looping.end_time)),
            ],
        ))
    });

    for (mut vis, children) in &mut speed_badge {
        set(&mut vis, children, speed_label.clone());
    }
    for (mut vis, children) in &mut wait_badge {
        set(&mut vis, children, wait_label.clone());
    }
    for (mut vis, children) in &mut loop_badge {
        set(&mut vis, children, loop_label.clone());
    }
}
