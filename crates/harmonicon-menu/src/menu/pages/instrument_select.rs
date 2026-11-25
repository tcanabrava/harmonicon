// SPDX-License-Identifier: MIT

//! Select Instrument: a grid of every instrument the game teaches, each a
//! card with its picture and name. The choice is `MyInstruments::active`,
//! which Play, Lessons, the Note Trainer and the Song Editor all use, so
//! this page is the one place the instrument changes.
//!
//! The pictures are white silhouettes (`scripts/gen_instrument_icons.py`)
//! tinted with the palette's text colour, so one file serves both themes.

use bevy::input_focus::AutoFocus;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;

use harmonicon_app::app::InstrumentConfirmed;
use harmonicon_app::profile::FirstRun;
use harmonicon_core::instrument::InstrumentKind;
use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::settings::MyInstruments;
use harmonicon_platform::theme::{LoadedTheme, Palette};
use harmonicon_ui::dialogs::button;

use crate::menu::routing::MenuPage;
use crate::menu::scene::{spawn_back_button, spawn_menu_root};

const CARD_WIDTH: f32 = 200.0;
const CARD_HEIGHT: f32 = 220.0;
const ICON_SIZE: f32 = 128.0;
/// Wide enough for five cards per row on a Full HD window.
const GRID_MAX_WIDTH: f32 = 5.0 * CARD_WIDTH + 4.0 * GRID_GAP;
const GRID_GAP: f32 = 20.0;

/// One instrument's card on the grid.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InstrumentCard(pub InstrumentKind);

pub(crate) fn icon_path(kind: InstrumentKind) -> String {
    format!("images/instruments/{}.png", kind.slug())
}

/// Locale key for an instrument's display name.
pub(crate) fn name_key(kind: InstrumentKind) -> String {
    format!("song-instrument-{}", kind.slug())
}

/// Where the menu goes once an instrument is chosen: Welcome on a first
/// launch (consuming `FirstRun`, which routing left for this), else Main.
pub(crate) fn after_choice(first_run: &mut FirstRun) -> MenuPage {
    if std::mem::take(&mut first_run.0) { MenuPage::Welcome } else { MenuPage::Main }
}

fn choose(
    kind: InstrumentKind,
) -> impl Fn(
    On<Activate>,
    ResMut<MyInstruments>,
    ResMut<InstrumentConfirmed>,
    ResMut<FirstRun>,
    ResMut<NextState<MenuPage>>,
) + Clone {
    move |_, mut mine, mut confirmed, mut first_run, mut page| {
        if mine.active != Some(kind) {
            mine.active = Some(kind);
        }
        confirmed.0 = true;
        page.set(after_choice(&mut first_run));
    }
}

pub(crate) fn setup_instrument_select(
    mut commands: Commands,
    theme: Res<LoadedTheme>,
    loc: Res<Localization>,
    mine: Res<MyInstruments>,
    confirmed: Res<InstrumentConfirmed>,
    asset_server: Res<AssetServer>,
) {
    let palette = theme.palette();
    let (root, header, _page_root) = spawn_menu_root(
        &mut commands,
        &loc.msg("instrument-select-title"),
        Some(&loc.msg("instrument-select-help")),
        &theme,
    );
    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(GRID_MAX_WIDTH),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(GRID_GAP),
            row_gap: Val::Px(GRID_GAP),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    commands.entity(root).add_child(grid);
    let current = mine.active();
    for kind in InstrumentKind::ALL {
        let card = spawn_card(
            &mut commands,
            kind,
            kind == current,
            &loc.msg(&name_key(kind)),
            asset_server.load(icon_path(kind)),
            palette,
        );
        commands.entity(grid).add_child(card);
    }
    // Before the first choice there is no menu to go back to.
    if confirmed.0 {
        spawn_back_button(
            &mut commands,
            header,
            &loc.msg("back"),
            |_: On<Activate>, mut page: ResMut<NextState<MenuPage>>| page.set(MenuPage::Main),
        );
    }
}

fn spawn_card(
    commands: &mut Commands,
    kind: InstrumentKind,
    selected: bool,
    name: &str,
    icon: Handle<Image>,
    palette: Palette,
) -> Entity {
    let (base, border) =
        if selected { (palette.selected, palette.accent) } else { (palette.control, palette.border) };
    let mut card = commands.spawn((
        InstrumentCard(kind),
        WidgetButton,
        TabIndex(0),
        Node {
            width: Val::Px(CARD_WIDTH),
            height: Val::Px(CARD_HEIGHT),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(if selected { 3.0 } else { 1.0 })),
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..default()
        },
        BorderColor::all(border),
    ));
    button::make_interactive(&mut card, base).observe(choose(kind));
    if selected {
        // Enter keeps the saved instrument straight away.
        card.insert(AutoFocus);
    }
    card.with_children(|card| {
        card.spawn((
            ImageNode { image: icon, color: palette.text, ..default() },
            Node { width: Val::Px(ICON_SIZE), height: Val::Px(ICON_SIZE), ..default() },
            Pickable::IGNORE,
        ));
        card.spawn((
            Text::new(name.to_string()),
            TextFont { font_size: FontSize::Px(20.0), ..default() },
            TextColor(palette.text),
            Pickable::IGNORE,
        ));
    });
    card.id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_instrument_has_a_picture() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for kind in InstrumentKind::ALL {
            let path = assets.join(icon_path(kind));
            assert!(path.is_file(), "missing {}", path.display());
        }
    }

    #[test]
    fn the_first_choice_leads_to_welcome_once() {
        let mut first_run = FirstRun(true);
        assert_eq!(after_choice(&mut first_run), MenuPage::Welcome);
        assert!(!first_run.0);
        assert_eq!(after_choice(&mut first_run), MenuPage::Main);
    }

    #[test]
    fn choosing_saves_the_instrument_and_confirms_it() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<harmonicon_app::app::AppState>()
            .add_sub_state::<MenuPage>()
            .init_resource::<MyInstruments>()
            .init_resource::<InstrumentConfirmed>()
            .insert_resource(FirstRun(false));
        let card = app.world_mut().spawn_empty().observe(choose(InstrumentKind::Violin)).id();
        app.world_mut().trigger(Activate { entity: card });
        assert_eq!(app.world().resource::<MyInstruments>().active, Some(InstrumentKind::Violin));
        assert!(app.world().resource::<InstrumentConfirmed>().0);
    }
}
