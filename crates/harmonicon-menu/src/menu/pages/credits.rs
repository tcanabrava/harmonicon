// SPDX-License-Identifier: MIT

//! Scrolling credits over a slowly moving 3D harmonica.

use bevy::{
    camera::visibility::RenderLayers, input_focus::tab_navigation::TabGroup, prelude::*,
    ui_widgets::Activate,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use harmonicon_platform::assets_management::SelectedHarmonicaModel;
use harmonicon_platform::localization::{Localization, LocalizationExt};

use crate::menu::scene::spawn_back_button;
use harmonicon_app::app::AppState;

// The credits 3D scene lives on its own render layer so it never touches the
// gameplay or options-preview layers.
const CREDITS_LAYER: usize = 20;

const SCROLL_SPEED: f32 = 55.0; // pixels per second upward
// Conservative start: text begins one screen-height below the viewport.
// Works for displays up to ~1200 px tall; on larger screens the text simply
// starts a little early, which is invisible during the fade-in anyway.
const SCROLL_START: f32 = 1000.0;

// ── Components ────────────────────────────────────────────────────────────────

/// Marks each credits root so cleanup removes its subtree.
#[derive(Component, Default, Clone)]
struct CreditsRoot;

/// The rotating harmonica in the 3D background.
#[derive(Component)]
struct CreditsHarmonica;

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
            .add_systems(OnExit(AppState::Credits), (cleanup, restore_camera))
            .add_systems(
                Update,
                (
                    rotate_harmonica,
                    scroll_credits,
                    crate::menu::scene::propagate_scene_layers,
                    handle_input,
                )
                    .run_if(in_state(AppState::Credits)),
            );
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    harmonica_model: Res<SelectedHarmonicaModel>,
    asset_server: Res<AssetServer>,
    loc: Res<Localization>,
    mut cameras: Query<(&mut Camera, &mut Transform), With<Camera2d>>,
) {
    // Same trick as 3D gameplay: push the shared Camera2d behind so the
    // Camera3d renders the harmonica to the screen first.
    for (mut cam, _) in &mut cameras {
        cam.order = 1;
        cam.clear_color = ClearColorConfig::None;
    }

    let layers = RenderLayers::layer(CREDITS_LAYER);
    spawn_3d_scene(&mut commands, &asset_server, &harmonica_model.0, &layers);
    spawn_ui(&mut commands, &loc);
}

fn cleanup(mut commands: Commands, roots: Query<Entity, With<CreditsRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

fn restore_camera(mut cameras: Query<(&mut Camera, &mut Transform), With<Camera2d>>) {
    for (mut cam, _) in &mut cameras {
        cam.order = 0;
        cam.clear_color = ClearColorConfig::Default;
    }
}

// ── 3-D background scene ──────────────────────────────────────────────────────

fn spawn_3d_scene(
    commands: &mut Commands,
    asset_server: &AssetServer,
    model: &str,
    layers: &RenderLayers,
) {
    // Camera: renders layer CREDITS_LAYER only, sits in front of everything.
    commands.spawn((
        Camera3d::default(),
        Camera { order: 0, ..default() },
        Transform::from_xyz(-1.5, 1.8, 5.0).looking_at(Vec3::new(0.0, 0.2, 0.0), Vec3::Y),
        layers.clone(),
        CreditsRoot,
    ));

    // Key light — warm, from upper-right front.
    commands.spawn((
        DirectionalLight { illuminance: 7_000.0, color: Color::srgb(1.0, 0.96, 0.88), ..default() },
        Transform::from_xyz(4.0, 6.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
        CreditsRoot,
    ));

    // Fill light — cool, from behind-left.
    commands.spawn((
        DirectionalLight {
            illuminance: 1_800.0,
            color: Color::srgb(0.55, 0.65, 0.90),
            ..default()
        },
        Transform::from_xyz(-5.0, 2.0, -3.0).looking_at(Vec3::ZERO, Vec3::Y),
        layers.clone(),
        CreditsRoot,
    ));

    // Harmonica model scene, slightly angled so it reads well.
    let scene_path = format!("harmonicas/3d/{model}/harmonica.glb#Scene0");
    commands.spawn((
        WorldAssetRoot(asset_server.load(scene_path)),
        Transform::from_scale(Vec3::splat(0.14)).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -0.4,
            0.2,
            0.0,
        )),
        // Visibility is auto-inserted by WorldAssetRoot's `#[require(Visibility)]`.
        layers.clone(),
        crate::menu::scene::SceneLayer(layers.clone()),
        CreditsHarmonica,
        CreditsRoot,
    ));
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

fn rotate_harmonica(time: Res<Time>, mut q: Query<&mut Transform, With<CreditsHarmonica>>) {
    let t = time.elapsed_secs();
    for mut tf in &mut q {
        // Slow Y-axis spin plus a gentle breathing bob.
        tf.rotation = Quat::from_euler(
            EulerRot::YXZ,
            -0.4 + t * 0.18,
            0.20 + (t * 0.7).sin() * 0.04,
            (t * 0.5).sin() * 0.02,
        );
        // Tiny vertical float so it feels alive.
        tf.translation.y = (t * 0.6).sin() * 0.008;
    }
}

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
