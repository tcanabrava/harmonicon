// SPDX-License-Identifier: MIT

//! Options → Lessons & songs: the configured lesson and song repositories,
//! what state each is in, and the controls to check, update, add and remove
//! them (`docs/content_packs_plan.md`).
//!
//! Nothing here downloads on its own: an update only happens when the
//! player confirms one. The page writes `ContentSources` (which persists and
//! makes `content_packs` re-read every pack) and `InstallPack` /
//! `CheckPackUpdates` messages; `harmonicon_platform::content_sync` does the
//! work and the page redraws its lists whenever the result changes.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use harmonicon_packs::pack::PackKind;
use harmonicon_packs::repo::RepoSpec;
use harmonicon_platform::content_packs::{
    ContentPacks, ContentSources, PackEntry, PackStatus, RepoInputError, parse_repo_input,
};
use harmonicon_platform::content_sync::{
    CheckPackUpdates, InstallPack, PackSync, UpdateState, uninstall,
};
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;
use harmonicon_ui::dialogs::button;
use harmonicon_ui::dialogs::confirm_dialog::{ConfirmChosen, DialogId, OpenConfirmDialog};
use harmonicon_ui::dialogs::text_input::{TextInputCommitted, spawn_text_input};

use crate::menu::routing::MenuPage;
use crate::menu::scene::{cleanup_menu, spawn_back_button, spawn_menu_root};

const UPDATE_PURPOSE: DialogId = DialogId("content_sources_update");
const REMOVE_PURPOSE: DialogId = DialogId("content_sources_remove");

const ROW_WIDTH: f32 = 860.0;
const DIM: Color = Color::srgb(0.62, 0.65, 0.80);
const WARN: Color = Color::srgb(0.95, 0.72, 0.45);

/// The rows of one kind's repositories; rebuilt whenever they change.
#[derive(Component, Clone, Copy)]
struct PackList(PackKind);

/// Why the last attempt to add a `kind` repository failed, under its box.
#[derive(Component, Clone, Copy)]
struct AddError(PackKind);

/// What has been typed into each kind's "add" box, committed on Enter or
/// when the box loses focus — which clicking Add does first.
#[derive(Resource, Default)]
struct AddDrafts {
    songs: String,
    lessons: String,
}

impl AddDrafts {
    fn of_mut(&mut self, kind: PackKind) -> &mut String {
        match kind {
            PackKind::Songs => &mut self.songs,
            PackKind::Lessons => &mut self.lessons,
        }
    }
}

/// The update or removal waiting on the confirm dialog.
#[derive(Resource, Default)]
struct PendingAction(Option<(PackKind, RepoSpec)>);

pub struct ContentSourcesPlugin;

impl Plugin for ContentSourcesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AddDrafts>()
            .init_resource::<PendingAction>()
            .add_systems(OnEnter(MenuPage::ContentSources), setup)
            .add_systems(OnExit(MenuPage::ContentSources), cleanup_menu)
            .add_systems(Update, rebuild_lists.run_if(in_state(MenuPage::ContentSources)))
            .add_systems(Update, handle_confirm);
    }
}

// ── What a row says ──────────────────────────────────────────────────────────

/// One repository's situation, combining what is on disk with what the
/// background sync knows.
#[derive(Debug, Clone, PartialEq)]
enum RowStatus {
    Downloading,
    DownloadFailed(String),
    NotInstalled,
    Unusable(String),
    Local {
        version: String,
    },
    Checking {
        version: String,
    },
    UpToDate {
        version: String,
    },
    UpdateAvailable {
        version: String,
    },
    CheckFailed {
        version: String,
        error: String,
    },
    /// Installed, not checked yet.
    Installed {
        version: String,
    },
}

fn row_status(entry: &PackEntry, sync: &PackSync) -> RowStatus {
    if sync.installing(&entry.slug) {
        return RowStatus::Downloading;
    }
    match &entry.status {
        PackStatus::NotInstalled => match sync.failures.get(&entry.slug) {
            Some(error) => RowStatus::DownloadFailed(error.clone()),
            None => RowStatus::NotInstalled,
        },
        PackStatus::Unusable(why) => RowStatus::Unusable(why.clone()),
        PackStatus::Ready { manifest, .. } => {
            let version = manifest.version.to_string();
            if matches!(entry.spec, RepoSpec::Local { .. }) {
                return RowStatus::Local { version };
            }
            if let Some(error) = sync.failures.get(&entry.slug) {
                return RowStatus::CheckFailed { version, error: error.clone() };
            }
            match sync.updates.get(&entry.slug) {
                Some(UpdateState::Checking) => RowStatus::Checking { version },
                Some(UpdateState::UpToDate) => RowStatus::UpToDate { version },
                Some(UpdateState::Available { .. }) => RowStatus::UpdateAvailable { version },
                Some(UpdateState::Failed(error)) => {
                    RowStatus::CheckFailed { version, error: error.clone() }
                }
                None => RowStatus::Installed { version },
            }
        }
    }
}

fn status_text(status: &RowStatus, loc: &Localization) -> String {
    let v = |version: &String| ("version", version.clone());
    String::from(match status {
        RowStatus::Downloading => loc.msg("content-status-downloading"),
        RowStatus::DownloadFailed(e) => {
            loc.msg_args("content-status-download-failed", &[("error", e.clone())])
        }
        RowStatus::NotInstalled => loc.msg("content-status-not-installed"),
        RowStatus::Unusable(why) => {
            loc.msg_args("content-status-unusable", &[("reason", why.clone())])
        }
        RowStatus::Local { version } => loc.msg_args("content-status-local", &[v(version)]),
        RowStatus::Checking { version } => loc.msg_args("content-status-checking", &[v(version)]),
        RowStatus::UpToDate { version } => loc.msg_args("content-status-up-to-date", &[v(version)]),
        RowStatus::UpdateAvailable { version } => {
            loc.msg_args("content-status-update-available", &[v(version)])
        }
        RowStatus::CheckFailed { version, error } => {
            loc.msg_args("content-status-check-failed", &[v(version), ("error", error.clone())])
        }
        RowStatus::Installed { version } => loc.msg_args("content-status-installed", &[v(version)]),
    })
}

/// The pack's own name once its `pack.json` has been read, else its address.
fn display_name(entry: &PackEntry) -> String {
    match &entry.status {
        PackStatus::Ready { manifest, .. } => manifest.name.clone(),
        _ => address(&entry.spec),
    }
}

fn address(spec: &RepoSpec) -> String {
    match spec {
        RepoSpec::Remote { url, git_ref: None } => url.clone(),
        RepoSpec::Remote { url, git_ref: Some(r) } => format!("{url} ({r})"),
        RepoSpec::Local { path } => path.display().to_string(),
    }
}

// ── Page ─────────────────────────────────────────────────────────────────────

fn setup(mut commands: Commands, loc: Res<Localization>, theme: Res<LoadedTheme>) {
    let (root, header, _page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("content-title"),
        Some(&loc.msg("content-subtitle")),
        &theme,
        "ContentSources",
    );
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("content-back-tooltip"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Options),
    );

    let check = loc.msg("content-check-updates");
    let column = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            width: Val::Px(ROW_WIDTH),
            ..default()
        })
        .id();
    commands.entity(root).add_child(column);
    // In a row of its own: the column stretches its children across, and a
    // full-width button reads as a heading.
    let check_row = commands.spawn(Node::default()).id();
    commands.entity(column).add_child(check_row);
    commands.entity(check_row).with_children(|c| {
        c.spawn_empty().apply_scene(button::small(
            &check,
            |_: On<Activate>, mut checks: MessageWriter<CheckPackUpdates>| {
                checks.write(CheckPackUpdates);
            },
        ));
    });

    for (kind, heading) in
        [(PackKind::Songs, "content-songs"), (PackKind::Lessons, "content-lessons")]
    {
        spawn_section(&mut commands, column, kind, &loc.msg(heading), &loc);
    }

    let note = String::from(loc.msg("content-trust-note"));
    commands.entity(column).with_children(|c| {
        c.spawn_empty().apply_scene(bsn! {
            Text({note})
            TextFont { font_size: {FontSize::Px(14.0)} }
            TextColor({DIM})
        });
    });
}

fn spawn_section(
    commands: &mut Commands,
    parent: Entity,
    kind: PackKind,
    heading: &str,
    loc: &Localization,
) {
    let heading = heading.to_string();
    let add_label = String::from(loc.msg("content-add"));
    let add_hint = String::from(loc.msg("content-add-hint"));
    commands.entity(parent).with_children(|c| {
        c.spawn_empty().apply_scene(bsn! {
            Text({heading})
            TextFont { font_size: {FontSize::Px(24.0)} }
            TextColor({Color::WHITE})
            Node { margin: {UiRect::top(Val::Px(10.0))} }
        });
        c.spawn((
            PackList(kind),
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() },
        ));
    });

    // The add box: type an address or a folder, then Add.
    let add_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    commands.entity(parent).add_child(add_row);
    commands.entity(add_row).with_children(|r| {
        r.spawn_empty().apply_scene(bsn! {
            Text({add_hint})
            TextFont { font_size: {FontSize::Px(14.0)} }
            TextColor({DIM})
        });
    });
    spawn_text_input(
        commands,
        add_row,
        "",
        420.0,
        Color::srgb(0.08, 0.08, 0.12),
        Color::srgb(0.35, 0.35, 0.5),
        move |ev: On<TextInputCommitted>, mut drafts: ResMut<AddDrafts>| {
            *drafts.of_mut(kind) = ev.value.clone();
        },
    );
    commands.entity(add_row).with_children(|r| {
        r.spawn_empty().apply_scene(button::small(
            &add_label,
            move |_: On<Activate>,
                  mut drafts: ResMut<AddDrafts>,
                  mut sources: ResMut<ContentSources>,
                  mut install: MessageWriter<InstallPack>,
                  loc: Res<Localization>,
                  mut errors: Query<(&AddError, &mut Text)>| {
                let draft = drafts.of_mut(kind).clone();
                let message = match parse_repo_input(&draft, sources.of(kind)) {
                    Ok(spec) => {
                        sources.of_mut(kind).push(spec.clone());
                        install.write(InstallPack { kind, spec });
                        drafts.of_mut(kind).clear();
                        String::new()
                    }
                    Err(RepoInputError::Empty) => String::new(),
                    Err(RepoInputError::NotARepository) => {
                        String::from(loc.msg_args("content-add-invalid", &[("input", draft)]))
                    }
                    Err(RepoInputError::AlreadyListed) => {
                        String::from(loc.msg("content-add-duplicate"))
                    }
                };
                for (error, mut text) in &mut errors {
                    if error.0 == kind {
                        text.0.clone_from(&message);
                    }
                }
            },
        ));
    });
    commands.entity(parent).with_children(|c| {
        c.spawn((
            AddError(kind),
            Text::new(""),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(WARN),
        ));
    });
}

/// Redraws both lists whenever a repository, its download or its update
/// check changes. Also runs on the page's first frame, which is what draws
/// them at all.
fn rebuild_lists(
    mut commands: Commands,
    packs: Res<ContentPacks>,
    sync: Res<PackSync>,
    sources: Res<ContentSources>,
    loc: Res<Localization>,
    lists: Query<(Entity, &PackList, Ref<PackList>)>,
) {
    let fresh_page = lists.iter().any(|(_, _, list)| list.is_added());
    if !fresh_page && !packs.is_changed() && !sync.is_changed() && !sources.is_changed() {
        return;
    }
    for (container, list, _) in &lists {
        commands.entity(container).despawn_related::<Children>();
        let entries: Vec<&PackEntry> = packs.0.iter().filter(|e| e.kind == list.0).collect();
        if entries.is_empty() {
            let empty = String::from(loc.msg("content-empty"));
            commands.entity(container).with_children(|c| {
                c.spawn_empty().apply_scene(bsn! {
                    Text({empty})
                    TextFont { font_size: {FontSize::Px(15.0)} }
                    TextColor({DIM})
                });
            });
        }
        for entry in entries {
            spawn_row(&mut commands, container, entry, &sync, &loc);
        }
    }
}

fn spawn_row(
    commands: &mut Commands,
    parent: Entity,
    entry: &PackEntry,
    sync: &PackSync,
    loc: &Localization,
) {
    let status = row_status(entry, sync);
    let name = display_name(entry);
    let address = address(&entry.spec);
    let status_line = status_text(&status, loc);
    let status_color = match status {
        RowStatus::DownloadFailed(_) | RowStatus::Unusable(_) | RowStatus::CheckFailed { .. } => {
            WARN
        }
        RowStatus::UpdateAvailable { .. } => Color::srgb(0.55, 0.88, 0.62),
        _ => DIM,
    };

    let row = commands
        .spawn_scene(bsn! {
            Node {
                width: {Val::Percent(100.0)},
                flex_direction: {FlexDirection::Row},
                justify_content: {JustifyContent::SpaceBetween},
                align_items: {AlignItems::Center},
                padding: {UiRect::axes(Val::Px(14.0), Val::Px(10.0))},
                column_gap: {Val::Px(12.0)},
            }
            BackgroundColor({Color::srgba(1.0, 1.0, 1.0, 0.04)})
            Children [
                Node {
                    flex_direction: {FlexDirection::Column},
                    row_gap: {Val::Px(2.0)},
                    flex_shrink: {1.0_f32},
                }
                Children [
                    Text({name})
                    TextFont { font_size: {FontSize::Px(19.0)} }
                    TextColor({Color::WHITE})
                    --
                    Text({address})
                    TextFont { font_size: {FontSize::Px(13.0)} }
                    TextColor({DIM})
                    --
                    Text({status_line})
                    TextFont { font_size: {FontSize::Px(14.0)} }
                    TextColor({status_color})
                ]
            ]
        })
        .id();
    commands.entity(parent).add_child(row);

    let buttons = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    commands.entity(row).add_child(buttons);

    let (kind, spec, shown_name) = (entry.kind, entry.spec.clone(), display_name(entry));
    commands.entity(buttons).with_children(|b| {
        match status {
            RowStatus::UpdateAvailable { .. } => {
                b.spawn_empty().apply_scene(button::small(
                    &loc.msg("content-update"),
                    update_action(kind, spec.clone(), shown_name.clone()),
                ));
            }
            RowStatus::NotInstalled | RowStatus::DownloadFailed(_) => {
                let spec = spec.clone();
                b.spawn_empty().apply_scene(button::small(
                    &loc.msg("content-download"),
                    move |_: On<Activate>, mut install: MessageWriter<InstallPack>| {
                        install.write(InstallPack { kind, spec: spec.clone() });
                    },
                ));
            }
            _ => {}
        }
        b.spawn_empty().apply_scene(button::small(
            &loc.msg("content-remove"),
            move |_: On<Activate>,
                  mut pending: ResMut<PendingAction>,
                  loc: Res<Localization>,
                  mut open: MessageWriter<OpenConfirmDialog>| {
                pending.0 = Some((kind, spec.clone()));
                open.write(OpenConfirmDialog {
                    purpose: REMOVE_PURPOSE,
                    message: String::from(
                        loc.msg_args("content-confirm-remove", &[("name", shown_name.clone())]),
                    ),
                });
            },
        ));
    });
}

/// Shared by Options and the song picker, including the confirmation dialog.
fn update_action(
    kind: PackKind,
    spec: RepoSpec,
    shown_name: String,
) -> impl FnMut(On<Activate>, ResMut<PendingAction>, Res<Localization>, MessageWriter<OpenConfirmDialog>)
+ Clone
+ Sync
+ 'static {
    move |_: On<Activate>,
          mut pending: ResMut<PendingAction>,
          loc: Res<Localization>,
          mut open: MessageWriter<OpenConfirmDialog>| {
        pending.0 = Some((kind, spec.clone()));
        open.write(OpenConfirmDialog {
            purpose: UPDATE_PURPOSE,
            message: loc
                .msg_args("content-confirm-update", &[("name", shown_name.clone())])
                .to_string(),
        });
    }
}

pub(super) fn spawn_song_update(
    commands: &mut Commands,
    parent: Entity,
    entry: &PackEntry,
    loc: &Localization,
) {
    let name = display_name(entry);
    let label = loc.msg_args("song-update", &[("name", name.clone())]);
    let button = commands
        .spawn_empty()
        .apply_scene(button::small(&label, update_action(entry.kind, entry.spec.clone(), name)))
        .id();
    commands.entity(parent).add_child(button);
}

/// Carries out the update or removal the player just confirmed.
fn handle_confirm(
    mut chosen: MessageReader<ConfirmChosen>,
    mut pending: ResMut<PendingAction>,
    mut sources: ResMut<ContentSources>,
    mut install: MessageWriter<InstallPack>,
) {
    for choice in chosen.read() {
        if choice.purpose != UPDATE_PURPOSE && choice.purpose != REMOVE_PURPOSE {
            continue;
        }
        let Some((kind, spec)) = pending.0.take() else {
            continue;
        };
        if !choice.confirmed {
            continue;
        }
        if choice.purpose == UPDATE_PURPOSE {
            install.write(InstallPack { kind, spec });
        } else {
            if let Err(e) = uninstall(&spec) {
                warn!("Could not delete the files of {}: {e}", spec.slug());
            }
            let slug = spec.slug();
            sources.of_mut(kind).retain(|s| s.slug() != slug);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_update_confirmation_installs_only_after_accepting() {
        let mut app = App::new();
        app.init_resource::<PendingAction>()
            .init_resource::<ContentSources>()
            .add_message::<ConfirmChosen>()
            .add_message::<InstallPack>()
            .add_systems(Update, handle_confirm);
        for confirmed in [false, true] {
            app.world_mut().resource_mut::<PendingAction>().0 = Some((PackKind::Songs, remote()));
            app.world_mut().write_message(ConfirmChosen { purpose: UPDATE_PURPOSE, confirmed });
            app.update();
            let requests: Vec<_> =
                app.world_mut().resource_mut::<Messages<InstallPack>>().drain().collect();
            assert_eq!(requests.len(), usize::from(confirmed));
            if confirmed {
                assert_eq!(requests[0].kind, PackKind::Songs);
                assert_eq!(requests[0].spec, remote());
            }
            assert!(app.world().resource::<PendingAction>().0.is_none());
        }
    }

    use harmonicon_packs::pack::PackManifest;

    fn entry(spec: RepoSpec, status: PackStatus) -> PackEntry {
        PackEntry { kind: PackKind::Lessons, slug: spec.slug(), root: "/x".into(), spec, status }
    }

    fn ready() -> PackStatus {
        PackStatus::Ready {
            manifest: PackManifest::parse(
                br#"{"schema":1,"kind":"lessons","id":"p","name":"Blues licks","version":"1.2.0"}"#,
            )
            .unwrap()
            .unwrap(),
            commit: Some("abc".into()),
        }
    }

    fn remote() -> RepoSpec {
        RepoSpec::Remote { url: "https://h/me/licks".into(), git_ref: None }
    }

    #[test]
    fn an_installed_pack_reports_what_its_check_found() {
        let e = entry(remote(), ready());
        let mut sync = PackSync::default();
        assert_eq!(row_status(&e, &sync), RowStatus::Installed { version: "1.2.0".into() });
        sync.updates.insert(e.slug.clone(), UpdateState::Available { commit: "def".into() });
        assert_eq!(row_status(&e, &sync), RowStatus::UpdateAvailable { version: "1.2.0".into() });
        assert_eq!(display_name(&e), "Blues licks");
    }

    #[test]
    fn a_failed_download_is_shown_until_retried() {
        let e = entry(remote(), PackStatus::NotInstalled);
        let mut sync = PackSync::default();
        sync.failures.insert(e.slug.clone(), "offline".into());
        assert_eq!(row_status(&e, &sync), RowStatus::DownloadFailed("offline".into()));
        assert_eq!(display_name(&e), "https://h/me/licks");
    }

    #[test]
    fn a_local_folder_is_never_offered_an_update() {
        let e = entry(RepoSpec::Local { path: "/home/me/licks".into() }, ready());
        assert_eq!(
            row_status(&e, &PackSync::default()),
            RowStatus::Local { version: "1.2.0".into() }
        );
    }

    #[test]
    fn every_status_has_text() {
        let loc = Localization::default();
        for status in [
            RowStatus::Downloading,
            RowStatus::DownloadFailed("e".into()),
            RowStatus::NotInstalled,
            RowStatus::Unusable("r".into()),
            RowStatus::UpToDate { version: "1".into() },
        ] {
            assert!(!status_text(&status, &loc).is_empty());
        }
    }
}
