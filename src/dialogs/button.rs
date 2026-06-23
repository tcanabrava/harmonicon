use bevy::picking::Pickable;
use bevy::picking::events::{Click, Out, Over, Pointer};
use bevy::prelude::*;
use bevy::ecs::system::IntoObserverSystem;

pub fn color_default() -> Color {
    Color::srgb(0.14, 0.14, 0.22)
}

fn mouse_over(ev: On<Pointer<Over>>, mut colors: Query<&mut BackgroundColor>) {
    if let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(Color::srgb(0.20, 0.20, 0.32));
    }
}

fn mouse_out(ev: On<Pointer<Out>>, mut colors: Query<&mut BackgroundColor>) {
    if let Ok(mut bg) = colors.get_mut(ev.entity) {
        *bg = BackgroundColor(color_default());
    }
}

pub fn default<M: 'static>(label: &str, on_click: impl IntoObserverSystem<Pointer<Click>, (), M> + Clone + Sync + 'static) -> impl Scene {
    bsn! {
        Button
        BackgroundColor({color_default()})
        on(on_click)
        on(mouse_over)
        on(mouse_out)
        Node {
            min_width: {Val::Px(220.0)},
            padding: {UiRect::axes(Val::Px(28.0), Val::Px(12.0))},
            justify_content: {JustifyContent::Center},
            // Keep natural size inside height-constrained scroll lists (the file
            // dialog) instead of being compressed to fit.
            flex_shrink: {0.0},
        }
        Children [
            (
                Text({label.to_string()})
                TextFont { font_size: {FontSize::Px(20.0)} }
                TextColor({Color::WHITE})
                Pickable { should_block_lower: {false}, is_hoverable: {false} }
            )
        ]
    }
}
