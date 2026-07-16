// SPDX-License-Identifier: MIT

//! Reusable "are you sure?" Yes/No modal, decoupled from its callers via
//! messages the same way `dialogs::file_dialog` is. Open one by writing
//! [`OpenConfirmDialog`] with a [`DialogId`] you choose and a message; read
//! the result by reading [`ConfirmChosen`] messages whose `purpose` matches
//! your id and checking `.confirmed`. Only one confirm dialog can be open at
//! a time (a second [`OpenConfirmDialog`] while one is already open replaces
//! it, same as `FileDialog`'s own single-dialog assumption).

use bevy::prelude::*;

use super::button;
pub use super::file_dialog::DialogId;

const PANEL_BG: Color = Color::srgba(0.08, 0.08, 0.11, 0.98);

/// Request to open the confirm dialog.
#[derive(Message)]
pub struct OpenConfirmDialog {
    pub purpose: DialogId,
    pub message: String,
}

/// Emitted once the user picks Yes or No (or cancels via Escape/backdrop,
/// which reads the same as No).
#[derive(Message)]
pub struct ConfirmChosen {
    pub purpose: DialogId,
    pub confirmed: bool,
}

#[derive(Resource, Default)]
struct ConfirmDialogOpen(Option<DialogId>);

#[derive(Component)]
struct ConfirmDialogRoot;

fn handle_open(
    mut requests: MessageReader<OpenConfirmDialog>,
    mut open: ResMut<ConfirmDialogOpen>,
    roots: Query<Entity, With<ConfirmDialogRoot>>,
    mut commands: Commands,
) {
    let Some(req) = requests.read().last() else {
        return;
    };
    for e in &roots {
        commands.entity(e).despawn();
    }
    open.0 = Some(req.purpose);

    let purpose = req.purpose;
    commands
        .spawn((
            ConfirmDialogRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.78)),
            GlobalZIndex(300),
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(18.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        max_width: Val::Px(480.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(Color::srgb(0.30, 0.30, 0.40)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(req.message.clone()),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextLayout {
                            justify: Justify::Center,
                            ..default()
                        },
                    ));
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(12.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn_empty().apply_scene(button::small(
                                "Yes",
                                move |_: On<Pointer<Click>>,
                                      mut open: ResMut<ConfirmDialogOpen>,
                                      roots: Query<Entity, With<ConfirmDialogRoot>>,
                                      mut chosen: MessageWriter<ConfirmChosen>,
                                      mut commands: Commands| {
                                    respond(purpose, true, &mut open, &roots, &mut chosen, &mut commands);
                                },
                            ));
                            row.spawn_empty().apply_scene(button::small(
                                "No",
                                move |_: On<Pointer<Click>>,
                                      mut open: ResMut<ConfirmDialogOpen>,
                                      roots: Query<Entity, With<ConfirmDialogRoot>>,
                                      mut chosen: MessageWriter<ConfirmChosen>,
                                      mut commands: Commands| {
                                    respond(purpose, false, &mut open, &roots, &mut chosen, &mut commands);
                                },
                            ));
                        });
                });
        });
}

fn respond(
    purpose: DialogId,
    confirmed: bool,
    open: &mut ConfirmDialogOpen,
    roots: &Query<Entity, With<ConfirmDialogRoot>>,
    chosen: &mut MessageWriter<ConfirmChosen>,
    commands: &mut Commands,
) {
    open.0 = None;
    for e in roots {
        commands.entity(e).despawn();
    }
    chosen.write(ConfirmChosen { purpose, confirmed });
}

/// Escape cancels the open dialog (reads as No), consuming the keypress so
/// an unrelated "go back" Escape handler ordered `.after` this one doesn't
/// also fire on the same press — same reasoning as `dialogs::combobox`'s
/// `close_open_comboboxes_on_escape`.
fn close_on_escape(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut open: ResMut<ConfirmDialogOpen>,
    roots: Query<Entity, With<ConfirmDialogRoot>>,
    mut chosen: MessageWriter<ConfirmChosen>,
    mut commands: Commands,
) {
    let Some(purpose) = open.0 else {
        return;
    };
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    keyboard.clear_just_pressed(KeyCode::Escape);
    respond(purpose, false, &mut open, &roots, &mut chosen, &mut commands);
}

pub struct ConfirmDialogPlugin;

impl Plugin for ConfirmDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OpenConfirmDialog>()
            .add_message::<ConfirmChosen>()
            .init_resource::<ConfirmDialogOpen>()
            .add_systems(Update, (handle_open, close_on_escape).chain());
    }
}
