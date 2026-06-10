use bevy::prelude::*;

#[derive(Resource)]
pub struct GlobalFonts {
    pub gameplay: Handle<Font>,
}

pub struct AssetsManagementPlugin;

impl Plugin for AssetsManagementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_global_fonts);
    }
}

fn load_global_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("Loading global fonts...");
    commands.insert_resource(GlobalFonts {
        gameplay: asset_server.load("fonts/UbuntuSansMono-Regular.otf"),
    });
}
