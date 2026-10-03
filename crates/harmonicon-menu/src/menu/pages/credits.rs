// SPDX-License-Identifier: MIT

//! Scrolling credits over the theme's Credits background.

use bevy::{input_focus::tab_navigation::TabGroup, prelude::*, ui_widgets::Activate};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use harmonicon_platform::localization::{Localization, LocalizationExt};
use harmonicon_platform::theme::LoadedTheme;

use crate::menu::scene::spawn_back_button;
use harmonicon_app::app::AppState;

const SCROLL_SPEED: f32 = 55.0; // pixels per second upward
// Conservative start: text begins one screen-height below the viewport.
// Works for displays up to ~1200 px tall; on larger screens the text simply
// starts a little early, which is invisible during the fade-in anyway.
const SCROLL_START: f32 = 1000.0;

// ── Components ────────────────────────────────────────────────────────────────

/// Marks each credits root so cleanup removes its subtree.
#[derive(Component, Default, Clone)]
struct CreditsRoot;

/// The scrolling container node. `offset` is the current `top` value in pixels;
/// it starts positive (below the viewport) and decreases each frame.
#[derive(Component)]
struct CreditsScroll {
    offset: f32,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct CreditsPlugin;

impl Plugin for CreditsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Credits), setup)
            .add_systems(OnExit(AppState::Credits), cleanup)
            .add_systems(
                Update,
                (scroll_credits, handle_input).run_if(in_state(AppState::Credits)),
            );
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

fn setup(mut commands: Commands, loc: Res<Localization>, theme: Res<LoadedTheme>) {
    if let Some(background) = theme.background_for("Credits") {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::new(background.clone()),
            CreditsRoot,
        ));
    }
    spawn_ui(&mut commands, &loc);
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<CreditsRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

// ── UI overlay ────────────────────────────────────────────────────────────────

fn spawn_ui(commands: &mut Commands, loc: &Localization) {
    // Full-screen dark overlay that clips the scrolling text.
    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(15.0),
                top: Val::ZERO,
                width: Val::Percent(70.0),
                height: Val::Percent(100.0),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.82)),
            GlobalZIndex(10),
            CreditsRoot,
        ))
        .id();

    // Scrolling column — absolutely positioned inside the overlay.
    let scroller = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(SCROLL_START),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(32.0), Val::Px(24.0)),
                row_gap: Val::Px(0.0),
                ..default()
            },
            CreditsScroll { offset: SCROLL_START },
        ))
        .id();

    commands.entity(overlay).add_child(scroller);

    commands.entity(scroller).with_children(|col| {
        for item in load_credits() {
            spawn_credit_line(col, item);
        }
    });

    // Back button — the same icon-only header control every menu page uses
    // (`menu::scene::spawn_back_button`), anchored to the true screen corner
    // (not nested inside `overlay`, which only spans the middle 70% of the
    // width) with its own `GlobalZIndex` above `overlay`'s so it stays
    // clickable. Tagged `CreditsRoot` itself — the previous version spawned
    // a bare, untagged button here, which `cleanup` (only sweeping
    // `CreditsRoot`) never despawned, leaking it across screens.
    let back_anchor = commands
        .spawn_scene(bsn! {
            Node {
                position_type: {PositionType::Absolute},
                top: {Val::Px(20.0)},
                right: {Val::Px(20.0)},
            }
            GlobalZIndex(20)
            // The screen's only focusable element hangs off this anchor, so
            // this is what Tab has to scope to.
            TabGroup
            CreditsRoot
        })
        .id();
    spawn_back_button(
        commands,
        back_anchor,
        &loc.msg("credits-back-to-menu"),
        |_: On<Activate>,
         mut next_state: ResMut<NextState<AppState>>,
         mut ret_help: ResMut<harmonicon_app::app::ReturnToHelpAbout>| {
            ret_help.0 = true;
            next_state.set(AppState::Menu);
        },
    );
}

// ── Credit line definitions ───────────────────────────────────────────────────

enum CreditLine {
    BigTitle(String),
    Subtitle(String),
    Heading(String),
    Body(String),
    Divider,
    Gap(f32),
}

/// Reads `assets/credits.md` and converts it to [`CreditLine`] items.
///
/// Markdown conventions used in that file:
///   `#`   → BigTitle      `##` → Subtitle     `###` → Heading
///   plain paragraphs → Body (each soft-break becomes a separate line)
///   `---` → Divider
///
/// Gaps are injected automatically around each block type so the file only
/// needs to contain human-readable text without any layout annotations.
fn load_credits() -> Vec<CreditLine> {
    let markdown = std::fs::read_to_string("assets/credits.md").unwrap_or_else(|err| {
        warn!("Could not read assets/credits.md: {err}");
        String::new()
    });
    parse_credits(&markdown)
}

fn parse_credits(markdown: &str) -> Vec<CreditLine> {
    let mut out = vec![CreditLine::Gap(60.0)];
    let mut current_text = String::new();
    let mut heading_level: Option<HeadingLevel> = None;

    let parser = Parser::new_ext(markdown, Options::empty());

    for event in parser {
        match event {
            // ── Headings ──────────────────────────────────────────────────
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(level);
                current_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                let text = std::mem::take(&mut current_text);
                match heading_level.take() {
                    Some(HeadingLevel::H1) => {
                        out.push(CreditLine::Gap(32.0));
                        out.push(CreditLine::BigTitle(text));
                    }
                    Some(HeadingLevel::H2) => {
                        out.push(CreditLine::Subtitle(text));
                        out.push(CreditLine::Gap(32.0));
                    }
                    _ => {
                        out.push(CreditLine::Heading(text));
                    }
                }
            }
            // ── Paragraphs ────────────────────────────────────────────────
            Event::Start(Tag::Paragraph) => {
                current_text.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                let text = std::mem::take(&mut current_text);
                for line in text.split('\n') {
                    let line = line.trim();
                    if !line.is_empty() {
                        out.push(CreditLine::Body(line.to_string()));
                    }
                }
                out.push(CreditLine::Gap(12.0));
            }
            // ── Inline content ────────────────────────────────────────────
            Event::Text(t) => current_text.push_str(&t),
            Event::SoftBreak => current_text.push('\n'),
            // ── Horizontal rule → divider ─────────────────────────────────
            Event::Rule => {
                out.push(CreditLine::Divider);
                out.push(CreditLine::Gap(24.0));
            }
            _ => {}
        }
    }

    out.push(CreditLine::Gap(400.0));
    out
}

fn spawn_credit_line(parent: &mut ChildSpawnerCommands, item: CreditLine) {
    let (text, font_size, color, bottom_margin) = match item {
        CreditLine::BigTitle(text) => (text, 38.0, Color::WHITE, 6.0),
        CreditLine::Subtitle(text) => (text, 18.0, Color::srgb(0.62, 0.65, 0.80), 0.0),
        CreditLine::Heading(text) => (text, 20.0, Color::srgb(0.85, 0.72, 0.35), 8.0),
        CreditLine::Body(text) => (text, 17.0, Color::srgb(0.78, 0.80, 0.88), 4.0),
        CreditLine::Divider => {
            parent.spawn_empty().apply_scene(bsn! {
                Node {
                    width: {Val::Px(320.0)},
                    height: {Val::Px(1.0)},
                    margin: {UiRect::axes(Val::ZERO, Val::Px(2.0))},
                }
                BackgroundColor({Color::srgba(0.55, 0.58, 0.72, 0.40)})
            });
            return;
        }
        CreditLine::Gap(px) => {
            parent.spawn(Node { height: Val::Px(px), ..default() });
            return;
        }
    };
    parent.spawn_empty().apply_scene(bsn! {
        Text({text})
        TextFont { font_size: {FontSize::Px(font_size)} }
        TextColor({color})
        Node { margin: {UiRect::bottom(Val::Px(bottom_margin))} }
    });
}

// ── Update systems ────────────────────────────────────────────────────────────

fn scroll_credits(time: Res<Time>, mut scrollers: Query<(&mut Node, &mut CreditsScroll)>) {
    let dt = time.delta_secs();
    for (mut node, mut scroll) in &mut scrollers {
        scroll.offset -= SCROLL_SPEED * dt;
        node.top = Val::Px(scroll.offset);
    }
}

/// Esc leaves the credits screen (the Back button does it via its own on()).
fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut ret_help: ResMut<harmonicon_app::app::ReturnToHelpAbout>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        ret_help.0 = true;
        next_state.set(AppState::Menu);
    }
}
