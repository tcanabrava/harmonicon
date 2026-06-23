// SPDX-License-Identifier: MIT

//! Credits screen: a slow-rotating 3D harmonica in the background, dark
//! translucent overlay in front, vertical scrolling credit text on top.
//!
//! Entry: Main Menu → Credits button.
//! Exit:  ESC key or the "Back to Menu" button.

use bevy::{
    camera::visibility::RenderLayers,
    picking::Pickable,
    picking::events::{Click, Out, Over, Pointer},
    prelude::*,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::assets_management::{GlobalFonts, SelectedHarmonicaModel};

use super::AppState;

const BACK_IDLE: Color = Color::srgba(0.10, 0.10, 0.18, 0.90);
const BACK_HOVER: Color = Color::srgba(0.18, 0.18, 0.30, 0.95);

// The credits 3D scene lives on its own render layer so it never touches the
// gameplay or options-preview layers.
const CREDITS_LAYER: usize = 20;

const SCROLL_SPEED: f32 = 55.0; // pixels per second upward
// Conservative start: text begins one screen-height below the viewport.
// Works for displays up to ~1200 px tall; on larger screens the text simply
// starts a little early, which is invisible during the fade-in anyway.
const SCROLL_START: f32 = 1000.0;

// ── Components ────────────────────────────────────────────────────────────────

/// Marks every entity that belongs to the credits screen.
#[derive(Component, Default, Clone)]
struct CreditsRoot;

/// The rotating harmonica in the 3D background.
#[derive(Component)]
struct CreditsHarmonica;

/// Carries the desired `RenderLayers` so the layer-propagation system can push
/// it onto children that glTF spawns a frame late.
#[derive(Component)]
struct CreditsSceneLayer(RenderLayers);

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
                    propagate_scene_layers,
                    handle_input,
                )
                    .run_if(in_state(AppState::Credits)),
            );
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

fn setup(
    mut commands: Commands,
    fonts: Res<GlobalFonts>,
    harmonica_model: Res<SelectedHarmonicaModel>,
    asset_server: Res<AssetServer>,
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
    spawn_ui(&mut commands, &fonts);
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
        DirectionalLight {
            illuminance: 7_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            ..default()
        },
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
        Transform::from_scale(Vec3::splat(0.14))
            .with_rotation(Quat::from_euler(EulerRot::YXZ, -0.4, 0.2, 0.0)),
        // Visibility is auto-inserted by WorldAssetRoot's `#[require(Visibility)]`.
        layers.clone(),
        CreditsSceneLayer(layers.clone()),
        CreditsHarmonica,
        CreditsRoot,
    ));
}

// ── UI overlay ────────────────────────────────────────────────────────────────

fn spawn_ui(commands: &mut Commands, fonts: &GlobalFonts) {
    let font = fonts.gameplay.clone();

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
            spawn_credit_line(col, &font, item);
        }
    });

    // "Back to Menu" button — fixed at the bottom-right of the overlay. Its
    // click/hover behaviour rides along as inline on(...) observers. (Default
    // font: bsn! can't set TextFont.font in 0.19.)
    commands.spawn_scene(bsn! {
        Button
        Node {
            position_type: {PositionType::Absolute},
            bottom: {Val::Px(20.0)},
            right: {Val::Px(20.0)},
            padding: {UiRect::axes(Val::Px(22.0), Val::Px(10.0))},
            justify_content: {JustifyContent::Center},
        }
        BackgroundColor({BACK_IDLE})
        GlobalZIndex(20)
        CreditsRoot
        on(|_: On<Pointer<Click>>, mut next_state: ResMut<NextState<AppState>>| {
            next_state.set(AppState::Menu);
        })
        on(back_over)
        on(back_out)
        Children [
            (
                Text({"\u{2190} Back to Menu".to_string()})
                TextFont { font_size: {FontSize::Px(18.0)} }
                TextColor({Color::srgb(0.75, 0.78, 0.90)})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    });
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

fn spawn_credit_line(parent: &mut ChildSpawnerCommands, font: &FontSource, item: CreditLine) {
    match item {
        CreditLine::BigTitle(text) => {
            parent.spawn((
                Text::new(text),
                TextFont { font_size: FontSize::Px(38.0), font: font.clone(), ..default() },
                TextColor(Color::WHITE),
                Node { margin: UiRect::bottom(Val::Px(6.0)), ..default() },
            ));
        }
        CreditLine::Subtitle(text) => {
            parent.spawn((
                Text::new(text),
                TextFont { font_size: FontSize::Px(18.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.62, 0.65, 0.80)),
            ));
        }
        CreditLine::Heading(text) => {
            parent.spawn((
                Text::new(text),
                TextFont { font_size: FontSize::Px(20.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.85, 0.72, 0.35)),
                Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() },
            ));
        }
        CreditLine::Body(text) => {
            parent.spawn((
                Text::new(text),
                TextFont { font_size: FontSize::Px(17.0), font: font.clone(), ..default() },
                TextColor(Color::srgb(0.78, 0.80, 0.88)),
                Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() },
            ));
        }
        CreditLine::Divider => {
            parent.spawn((
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(1.0),
                    margin: UiRect::axes(Val::ZERO, Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.55, 0.58, 0.72, 0.40)),
            ));
        }
        CreditLine::Gap(px) => {
            parent.spawn(Node {
                height: Val::Px(px),
                ..default()
            });
        }
    }
}

// ── Update systems ────────────────────────────────────────────────────────────

fn rotate_harmonica(time: Res<Time>, mut q: Query<&mut Transform, With<CreditsHarmonica>>) {
    let dt = time.delta_secs();
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
        tf.translation.y = (t * 0.6).sin() * 0.008 * dt.recip().min(200.0) * dt;
    }
}

fn scroll_credits(
    time: Res<Time>,
    mut scrollers: Query<(&mut Node, &mut CreditsScroll)>,
) {
    let dt = time.delta_secs();
    for (mut node, mut scroll) in &mut scrollers {
        scroll.offset -= SCROLL_SPEED * dt;
        node.top = Val::Px(scroll.offset);
    }
}

/// Pushes the credits render layer onto scene children that glTF spawns a
/// frame late (they don't inherit `RenderLayers` from the parent on spawn).
fn propagate_scene_layers(
    mut commands: Commands,
    roots: Query<(Entity, &CreditsSceneLayer)>,
    children: Query<&Children>,
    already_layered: Query<(), With<RenderLayers>>,
) {
    for (root, layer) in &roots {
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if let Ok(kids) = children.get(entity) {
                for child in kids {
                    if already_layered.get(*child).is_err() {
                        commands.entity(*child).insert(layer.0.clone());
                    }
                    stack.push(*child);
                }
            }
        }
    }
}

/// Esc leaves the credits screen (the Back button does it via its own on()).
fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Menu);
    }
}

fn back_over(ev: On<Pointer<Over>>, mut colors: Query<&mut BackgroundColor>) {
    if let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(BACK_HOVER);
    }
}

fn back_out(ev: On<Pointer<Out>>, mut colors: Query<&mut BackgroundColor>) {
    if let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(BACK_IDLE);
    }
}
