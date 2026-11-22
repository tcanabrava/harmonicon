// SPDX-License-Identifier: MIT

//! Live appearance selection with palette swatches.

use bevy::input_focus::tab_navigation::TabIndex;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button};
use harmonicon_platform::assets_management::SelectedTheme;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::{Appearance, LoadedTheme};
use harmonicon_ui::dialogs::button::make_interactive;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{cleanup_menu, spawn_back_button, spawn_menu_root};

pub struct ThemePickerPlugin;

impl Plugin for ThemePickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuPage::Theme), setup)
            .add_systems(OnExit(MenuPage::Theme), cleanup_menu)
            .add_systems(Update, highlight_selection.run_if(in_state(MenuPage::Theme)));
    }
}

#[derive(Component)]
struct AppearanceCard(Appearance);

fn setup(mut commands: Commands, theme: Res<LoadedTheme>, loc: Res<Localization>) {
    let (root, header, _) = spawn_menu_root(
        &mut commands,
        &loc.msg("theme-title"),
        Some(&loc.msg("theme-subtitle")),
        &theme,
    );
    spawn_back_button(
        &mut commands,
        header,
        &loc.msg("theme-back-to-options"),
        |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Options),
    );
    let cards = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(18.0),
            row_gap: Val::Px(14.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(cards);
    for appearance in Appearance::ALL {
        let p = appearance.palette();
        let mut card = commands.spawn((
            Button,
            TabIndex(0),
            AppearanceCard(appearance),
            Node {
                width: Val::Px(256.0),
                max_width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BorderColor::all(p.border),
        ));
        make_interactive(&mut card, p.panel);
        card.observe(move |_: On<Activate>, mut selected: ResMut<SelectedTheme>| {
            if selected.0 != appearance {
                selected.0 = appearance;
            }
        });
        card.with_children(|children| {
            children.spawn((
                Text::new(loc.msg(appearance.label_key())),
                TextFont { font_size: FontSize::Px(24.0), ..default() },
                TextColor(p.text),
                Pickable::IGNORE,
            ));
            children
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(78.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(8.0),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(p.canvas),
                    Pickable::IGNORE,
                ))
                .with_children(|preview| {
                    for color in [p.control, p.border, p.accent] {
                        preview.spawn((
                            Node {
                                width: Val::Px(42.0),
                                height: Val::Px(34.0),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            BackgroundColor(color),
                            Pickable::IGNORE,
                        ));
                    }
                });
        });
        let id = card.id();
        commands.entity(cards).add_child(id);
    }
}

fn highlight_selection(
    selected: Res<SelectedTheme>,
    theme: Res<LoadedTheme>,
    mut cards: Query<(&AppearanceCard, &mut BorderColor)>,
) {
    for (card, mut border) in &mut cards {
        let color =
            if card.0 == selected.0 { theme.palette().accent } else { card.0.palette().border };
        border.set_if_neq(BorderColor::all(color));
    }
}
