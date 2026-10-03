use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::Pickable;
use bevy::picking::events::{
    PointerCancel, PointerDragEnd, PointerOut, PointerOver, PointerPress, PointerRelease,
};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::Button as WidgetButton;

pub fn color_default() -> Color {
    Color::srgb(0.14, 0.14, 0.22)
}

/// Background for a "this option is currently selected" choice button —
/// shared by any button-group picker (pitch algorithm, harmonica model, ...).
pub const CHOICE_SELECTED: Color = Color::srgb(0.25, 0.45, 0.30);
/// Hover background for an unselected choice button in the same group.
pub const CHOICE_HOVER: Color = Color::srgb(0.20, 0.20, 0.32);

/// A button's own resting (unhovered, unpressed) background. Several
/// callers — the Song Editor's `update_mod_panel`/`update_mode_buttons`/
/// etc. — already recompute a button's "logical" color every frame from
/// selection/active/enabled state; those now write *this* instead of
/// `BackgroundColor` directly, and [`apply_button_visuals`] is the only
/// system that still touches `BackgroundColor` — layering the hover/press
/// tint on top of whatever `BaseButtonColor` currently holds, whenever
/// either changes.
/// Without this split, a per-frame active-state rewrite and a transient
/// hover/press tint would each blindly overwrite `BackgroundColor` and
/// erase the other. See [`make_interactive`] for attaching the whole thing
/// to a button built outside a `bsn!` scene.
#[derive(Component, Clone, Copy, Default)]
pub struct BaseButtonColor(pub Color);

/// Live hover/press state for [`apply_button_visuals`] to read — updated by
/// this module's own observers, which touch only this, never
/// `BackgroundColor` directly (see [`BaseButtonColor`]'s doc comment for
/// why).
#[derive(Component, Clone, Copy, Default)]
struct ButtonInteractionState {
    hovered: bool,
    pressed: bool,
}

/// Blends `c` toward `target` by `t` (0..1) — used for both
/// [`brighten`]/[`darken`] rather than a flat per-channel add/subtract, so
/// the hover/press tint stays proportionally consistent regardless of how
/// bright or dark a given button's own base color is.
fn mix_toward(c: Color, target: Color, t: f32) -> Color {
    let a = c.to_srgba();
    let b = target.to_srgba();
    Color::srgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}

/// Hover tint: a base color mixed toward white.
fn brighten(base: Color) -> Color {
    mix_toward(base, Color::WHITE, 0.18)
}

/// Pressed ("sunken") tint: a base color mixed toward black — noticeably
/// darker than both the resting and hover colors, so a click reads as the
/// button physically depressing rather than just re-highlighting.
fn darken(base: Color) -> Color {
    mix_toward(base, Color::BLACK, 0.35)
}

fn mouse_over(ev: On<PointerOver>, mut states: Query<&mut ButtonInteractionState>) {
    if let Ok(mut s) = states.get_mut(ev.entity) {
        s.hovered = true;
    }
}

fn mouse_out(ev: On<PointerOut>, mut states: Query<&mut ButtonInteractionState>) {
    if let Ok(mut s) = states.get_mut(ev.entity) {
        s.hovered = false;
    }
}

/// Sunken while actually pressed — `bevy_ui_widgets::Button` already tracks
/// this precisely via its own `Pressed` marker (inserted on [`Press`],
/// removed on [`Release`]/[`DragEnd`]/[`Cancel`] — see its own observers),
/// so this mirrors that exact same event set purely for the visual, rather
/// than polling `Pressed` in a separate system.
fn mouse_press(ev: On<PointerPress>, mut states: Query<&mut ButtonInteractionState>) {
    if let Ok(mut s) = states.get_mut(ev.entity) {
        s.pressed = true;
    }
}

fn mouse_release(ev: On<PointerRelease>, mut states: Query<&mut ButtonInteractionState>) {
    if let Ok(mut s) = states.get_mut(ev.entity) {
        s.pressed = false;
    }
}

/// A press that ends without releasing over the button (dragged off, or
/// picking cancelled the gesture).
fn mouse_press_interrupted(ev: On<PointerCancel>, mut states: Query<&mut ButtonInteractionState>) {
    if let Ok(mut s) = states.get_mut(ev.entity) {
        s.pressed = false;
        s.hovered = false;
    }
}

/// Same as [`mouse_press_interrupted`], for the other event
/// `bevy_ui_widgets::Button` treats as ending a press without a click.
fn mouse_drag_end(ev: On<PointerDragEnd>, mut states: Query<&mut ButtonInteractionState>) {
    if let Ok(mut s) = states.get_mut(ev.entity) {
        s.pressed = false;
        s.hovered = false;
    }
}

/// Recomputes every interactive button's actual `BackgroundColor` from its
/// `BaseButtonColor` + `ButtonInteractionState` — see `BaseButtonColor`'s
/// doc comment for why this is the *only* system allowed to write
/// `BackgroundColor` on an entity carrying both. Visits only buttons whose
/// `BaseButtonColor` or `ButtonInteractionState` changed (spawning counts),
/// which covers a caller's own active-state system and this module's
/// hover/press observers alike, since both write one of those two.
/// Registered once app-wide by [`ButtonVisualsPlugin`].
fn apply_button_visuals(
    mut buttons: Query<
        (&BaseButtonColor, &ButtonInteractionState, &mut BackgroundColor),
        Or<(Changed<BaseButtonColor>, Changed<ButtonInteractionState>)>,
    >,
) {
    for (base, interaction, mut bg) in &mut buttons {
        let color = if interaction.pressed {
            darken(base.0)
        } else if interaction.hovered {
            brighten(base.0)
        } else {
            base.0
        };
        if bg.0 != color {
            bg.0 = color;
        }
    }
}

pub struct ButtonVisualsPlugin;

impl Plugin for ButtonVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_button_visuals);
    }
}

/// Attaches hover/press/release/cancel visual feedback to a button spawned
/// imperatively (outside a `bsn!` scene, e.g. the Song Editor's own
/// hand-rolled `WidgetButton`s) — inserts [`BaseButtonColor`] + a starting
/// [`BackgroundColor`] + [`ButtonInteractionState`], then the same six
/// observers [`small`]/[`sized`]/[`icon`]/[`default`] wire inline via
/// `on(...)`. A caller that also drives its own active/selected tint (the
/// Song Editor's `update_mod_panel` and siblings) should target
/// `BaseButtonColor`, not `BackgroundColor`, from here on — see
/// `BaseButtonColor`'s doc comment. Returns `ec` so it chains with the
/// caller's own `.observe(on_click)`/`.insert(...)`/`.with_children(...)`
/// calls.
pub fn make_interactive<'a, 'b>(
    ec: &'a mut EntityCommands<'b>,
    base: Color,
) -> &'a mut EntityCommands<'b> {
    ec.insert((BaseButtonColor(base), BackgroundColor(base), ButtonInteractionState::default()))
        .observe(mouse_over)
        .observe(mouse_out)
        .observe(mouse_press)
        .observe(mouse_release)
        .observe(mouse_press_interrupted)
        .observe(mouse_drag_end)
}

/// A compact button (no 220px min-width, smaller padding/font) for HUD-style
/// controls. Same colours/hover as [`default`].
pub fn small<M: 'static>(
    label: &str,
    on_click: impl IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) -> impl Scene {
    bsn! {
        WidgetButton
        TabIndex(0)
        BackgroundColor({color_default()})
        BaseButtonColor({color_default()})
        ButtonInteractionState
        on(on_click)
        on(mouse_over)
        on(mouse_out)
        on(mouse_press)
        on(mouse_release)
        on(mouse_press_interrupted)
        on(mouse_drag_end)
        Node {
            padding: {UiRect::axes(Val::Px(12.0), Val::Px(6.0))},
            justify_content: {JustifyContent::Center},
            flex_shrink: {0.0_f32},
        }
        Children [
            Text({label.to_string()})
            TextFont { font_size: {FontSize::Px(15.0)} }
            TextColor({Color::WHITE})
            Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

/// A button with an exact (not minimum) width and caller-chosen font size —
/// for lists where every row must come out the same size regardless of
/// label length (e.g. the Lessons list, where `default`'s `min_width`-only
/// sizing would let long titles grow wider than short ones). Text wraps
/// within `width` by bevy_ui's default text layout if `font_size` doesn't
/// keep the label to one line — pick `font_size` (see `menu::lessons::
/// lesson_button_font_size`) to avoid that.
pub fn sized<M: 'static>(
    label: &str,
    width: f32,
    font_size: f32,
    on_click: impl IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) -> impl Scene {
    bsn! {
        WidgetButton
        TabIndex(0)
        BackgroundColor({color_default()})
        BaseButtonColor({color_default()})
        ButtonInteractionState
        on(on_click)
        on(mouse_over)
        on(mouse_out)
        on(mouse_press)
        on(mouse_release)
        on(mouse_press_interrupted)
        on(mouse_drag_end)
        Node {
            width: {Val::Px(width)},
            padding: {UiRect::axes(Val::Px(16.0), Val::Px(12.0))},
            justify_content: {JustifyContent::Center},
            align_items: {AlignItems::Center},
            flex_shrink: {0.0_f32},
        }
        Children [
                Text({label.to_string()})
                TextFont { font_size: {FontSize::Px(font_size)} }
                TextColor({Color::WHITE})
                TextLayout { justify: {Justify::Center} }
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

/// A compact square icon-only button — a fixed glyph (e.g. a header's "←"
/// Back control), no caller-provided label. Same colours/hover as
/// [`default`]; pair with a `Tooltip` at the call site since there's no
/// visible text to explain the glyph.
pub fn icon<M: 'static>(
    glyph: &str,
    on_click: impl IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) -> impl Scene {
    bsn! {
        WidgetButton
        TabIndex(0)
        BackgroundColor({color_default()})
        BaseButtonColor({color_default()})
        ButtonInteractionState
        on(on_click)
        on(mouse_over)
        on(mouse_out)
        on(mouse_press)
        on(mouse_release)
        on(mouse_press_interrupted)
        on(mouse_drag_end)
        Node {
            width: {Val::Px(40.0)},
            height: {Val::Px(40.0)},
            justify_content: {JustifyContent::Center},
            align_items: {AlignItems::Center},
            flex_shrink: {0.0_f32},
        }
        Children [
                Text({glyph.to_string()})
                TextFont { font_size: {FontSize::Px(20.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

pub fn default<M: 'static>(
    label: &str,
    on_click: impl IntoObserverSystem<Activate, M> + Clone + Sync + 'static,
) -> impl Scene {
    bsn! {
        WidgetButton
        TabIndex(0)
        BackgroundColor({color_default()})
        BaseButtonColor({color_default()})
        ButtonInteractionState
        on(on_click)
        on(mouse_over)
        on(mouse_out)
        on(mouse_press)
        on(mouse_release)
        on(mouse_press_interrupted)
        on(mouse_drag_end)
        Node {
            min_width: {Val::Px(220.0)},
            padding: {UiRect::axes(Val::Px(28.0), Val::Px(12.0))},
            justify_content: {JustifyContent::Center},
            // Keep natural size inside height-constrained scroll lists (the file
            // dialog) instead of being compressed to fit.
            flex_shrink: {0.0_f32},
        }
        Children [
                Text({label.to_string()})
                TextFont { font_size: {FontSize::Px(20.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn background(world: &World, entity: Entity) -> Color {
        world.get::<BackgroundColor>(entity).unwrap().0
    }

    #[test]
    fn visuals_follow_hover_and_leave_an_idle_button_alone() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_button_visuals);
        let base = color_default();
        let button = world
            .spawn((
                BaseButtonColor(base),
                ButtonInteractionState::default(),
                BackgroundColor(Color::BLACK),
            ))
            .id();

        // A freshly spawned button is painted from its base colour.
        schedule.run(&mut world);
        assert_eq!(background(&world, button), base);

        world.get_mut::<ButtonInteractionState>(button).unwrap().hovered = true;
        schedule.run(&mut world);
        assert_eq!(background(&world, button), brighten(base));

        // Nothing changed this frame, so the colour is not rewritten.
        world.get_mut::<BackgroundColor>(button).unwrap().0 = Color::WHITE;
        schedule.run(&mut world);
        assert_eq!(background(&world, button), Color::WHITE);

        // A caller's new resting colour is picked up, hover still layered on.
        world.get_mut::<BaseButtonColor>(button).unwrap().0 = CHOICE_SELECTED;
        schedule.run(&mut world);
        assert_eq!(background(&world, button), brighten(CHOICE_SELECTED));
    }
}
