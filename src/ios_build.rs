use bevy::prelude::Plugin;
use bevy::winit::WinitSettings;

pub struct IosBuildPlugin;

impl Plugin for IosBuildPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        // Make the winit loop wait more aggressively when no user input is received
        // This can help reduce cpu usage on mobile devices
        app.insert_resource(WinitSettings::mobile());
    }
}
