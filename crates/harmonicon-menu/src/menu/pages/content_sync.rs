// SPDX-License-Identifier: MIT

//! The first-run download screen (`AppState::Syncing`): shown only while a
//! configured lesson or song pack has never been downloaded, since the game
//! has nothing to teach or play without them. Opens the menu once every
//! pack is installed; on a failure it shows the error with Retry and Quit.
//!
//! The downloads themselves are `harmonicon_platform::content_sync`'s, which
//! starts them at `Startup`; this screen only watches.

use bevy::{input_focus::tab_navigation::TabGroup, prelude::*, ui_widgets::Activate};

use harmonicon_app::app::AppState;
use harmonicon_packs::repo::RepoSpec;
use harmonicon_platform::content_packs::{ContentPacks, PackEntry, PackStatus};
use harmonicon_platform::content_sync::{InstallPack, PackSync};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_ui::dialogs::button;

#[derive(Component, Default, Clone)]
struct SyncRoot;

#[derive(Component, Default, Clone)]
struct SyncStatusText;

/// Holds Retry/Quit while a download has failed; empty otherwise, so no
/// hidden button is ever a Tab stop.
#[derive(Component, Default, Clone)]
struct SyncActions;

pub struct ContentSyncPlugin;

impl Plugin for ContentSyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Syncing), setup)
            .add_systems(OnExit(AppState::Syncing), cleanup)
            .add_systems(
                Update,
                (update_status, update_actions, open_menu_when_synced)
                    .run_if(in_state(AppState::Syncing)),
            );
    }
}

/// What the player sees a repository called before its `pack.json` has been
/// read: its address without the scheme.
fn display_name(entry: &PackEntry) -> String {
    match &entry.spec {
        RepoSpec::Remote { url, .. } => url
            .split_once("://")
            .map_or(url.as_str(), |(_, rest)| rest)
            .to_string(),
        RepoSpec::Local { path } => path.display().to_string(),
    }
}

/// One line per pack still missing: downloading, or why it failed.
fn status_lines(packs: &ContentPacks, sync: &PackSync, loc: &Localization) -> String {
    packs
        .0
        .iter()
        .filter(|e| e.status == PackStatus::NotInstalled)
        .map(|entry| {
            let name = ("name", display_name(entry));
            match sync.failures.get(&entry.slug) {
                Some(error) if !sync.installing(&entry.slug) => String::from(
                    loc.msg_args("sync-repo-failed", &[name, ("error", error.clone())]),
                ),
                _ => String::from(loc.msg_args("sync-repo-downloading", &[name])),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Stuck: nothing is downloading, yet something is still missing.
fn failed(packs: &ContentPacks, sync: &PackSync) -> bool {
    !sync.busy() && packs.any_missing()
}

fn setup(mut commands: Commands, loc: Res<Localization>) {
    commands.spawn_scene(bsn! {
        SyncRoot
        TabGroup
        Node {
            width: {Val::Percent(100.0)},
            height: {Val::Percent(100.0)},
            flex_direction: {FlexDirection::Column},
            align_items: {AlignItems::Center},
            justify_content: {JustifyContent::Center},
            row_gap: {Val::Px(20.0)},
        }
        BackgroundColor({Color::srgb(0.05, 0.05, 0.08)})
        Children [
            Text({String::from(loc.msg("sync-title"))})
            TextFont { font_size: {FontSize::Px(38.0)} }
            TextColor({Color::WHITE})
            --
            Text({String::from(loc.msg("sync-in-progress"))})
            TextFont { font_size: {FontSize::Px(18.0)} }
            TextColor({Color::srgb(0.62, 0.65, 0.80)})
            TextLayout { justify: {Justify::Center} }
            Node { max_width: {Val::Px(640.0)} }
            --
            SyncStatusText
            Text
            TextFont { font_size: {FontSize::Px(16.0)} }
            TextColor({Color::srgb(0.85, 0.85, 0.90)})
            TextLayout { justify: {Justify::Center} }
            Node { max_width: {Val::Px(900.0)} }
            --
            SyncActions
            Node {
                flex_direction: {FlexDirection::Row},
                column_gap: {Val::Px(16.0)},
            }
        ]
    });
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<SyncRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

fn update_status(
    packs: Res<ContentPacks>,
    sync: Res<PackSync>,
    loc: Res<Localization>,
    mut texts: Query<&mut Text, With<SyncStatusText>>,
) {
    let lines = status_lines(&packs, &sync, &loc);
    for mut text in &mut texts {
        if text.0 != lines {
            text.0.clone_from(&lines);
        }
    }
}

fn update_actions(
    mut commands: Commands,
    packs: Res<ContentPacks>,
    sync: Res<PackSync>,
    loc: Res<Localization>,
    actions: Query<(Entity, Option<&Children>), With<SyncActions>>,
) {
    let show = failed(&packs, &sync);
    for (container, children) in &actions {
        let shown = children.is_some_and(|c| !c.is_empty());
        if show && !shown {
            commands.entity(container).with_children(|row| {
                row.spawn_empty()
                    .apply_scene(button::default(&loc.msg("sync-retry"), retry));
                row.spawn_empty().apply_scene(button::default(
                    &loc.msg("sync-quit"),
                    |_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::Success);
                    },
                ));
            });
        } else if !show && shown {
            commands.entity(container).despawn_related::<Children>();
        }
    }
}

fn retry(_: On<Activate>, packs: Res<ContentPacks>, mut install: MessageWriter<InstallPack>) {
    for entry in packs
        .0
        .iter()
        .filter(|e| e.status == PackStatus::NotInstalled)
    {
        install.write(InstallPack {
            kind: entry.kind,
            spec: entry.spec.clone(),
        });
    }
}

fn open_menu_when_synced(
    packs: Res<ContentPacks>,
    sync: Res<PackSync>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !sync.busy() && !packs.any_missing() {
        next.set(AppState::Menu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonicon_packs::pack::PackKind;

    fn entry(url: &str, status: PackStatus) -> PackEntry {
        let spec = RepoSpec::Remote {
            url: url.into(),
            git_ref: None,
        };
        PackEntry {
            kind: PackKind::Songs,
            slug: spec.slug(),
            spec,
            root: "/x".into(),
            status,
        }
    }

    #[test]
    fn names_a_repository_by_its_address() {
        let e = entry(
            "https://github.com/tcanabrava/harmonicon-songs",
            PackStatus::NotInstalled,
        );
        assert_eq!(display_name(&e), "github.com/tcanabrava/harmonicon-songs");
    }

    #[test]
    fn lists_only_what_is_missing() {
        let packs = ContentPacks(vec![
            entry("https://h/missing", PackStatus::NotInstalled),
            entry("https://h/broken", PackStatus::Unusable("x".into())),
        ]);
        let mut sync = PackSync::default();
        sync.failures
            .insert(packs.0[0].slug.clone(), "offline".into());
        let lines = status_lines(&packs, &sync, &Localization::default());
        assert_eq!(lines.lines().count(), 1);
        assert!(lines.contains("sync-repo-failed"), "{lines}");
        assert!(failed(&packs, &sync));
    }

    #[test]
    fn nothing_missing_is_not_a_failure() {
        let packs = ContentPacks(vec![entry(
            "https://h/broken",
            PackStatus::Unusable("x".into()),
        )]);
        assert!(!failed(&packs, &PackSync::default()));
    }
}
