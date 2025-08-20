use bevy::prelude::*;

use avian3d::prelude::*;
use bevister::plugin;
use bevy::diagnostic;
use bevy::log::{Level, LogPlugin};
use bevy::pbr::{ExtendedMaterial, MaterialPlugin, StandardMaterial};
use bevy::window::WindowMode;
use bevy_enhanced_input::prelude;
use bevy_enhanced_input::prelude::InputContextAppExt;
use bevy_prng::WyRand;
use bevy_rand::plugin::EntropyPlugin;
use bevy_screen_diagnostics::{ScreenDiagnosticsPlugin, ScreenFrameDiagnosticsPlugin};
use diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use plugin::{TriplanarExtension, VoxelPlugin};
use prelude::EnhancedInputPlugin;

mod demo1;
mod fly_cam;

fn main() {
    App::new()
        // we want Bevy to measure these values for us:
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(EntityCountDiagnosticsPlugin)
        .add_plugins(SystemInformationDiagnosticsPlugin)
        .add_plugins((
            DefaultPlugins
                .set(LogPlugin {
                    // This will show some log events from Bevy to the native logger.
                    level: Level::DEBUG,
                    filter: "wgpu=error,bevy_render=info,bevy_ecs=trace".to_string(),
                    ..Default::default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resizable: false,
                        mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                        // on iOS, gestures must be enabled.
                        // This doesn't work on Android
                        recognize_rotation_gesture: true,
                        // Only has an effect on iOS
                        prefers_home_indicator_hidden: true,
                        // Only has an effect on iOS
                        prefers_status_bar_hidden: true,
                        ..default()
                    }),
                    ..default()
                }),
            ScreenDiagnosticsPlugin::default(),
            ScreenFrameDiagnosticsPlugin,
            PhysicsPlugins::default(),
            EnhancedInputPlugin,
            VoxelPlugin,
            EntropyPlugin::<WyRand>::default(),
        ))
        .add_plugins(MaterialPlugin::<
            ExtendedMaterial<StandardMaterial, TriplanarExtension>,
        >::default())
        .add_input_context::<fly_cam::FlyCamCtx>()
        .add_systems(Startup, fly_cam::setup)
        .add_systems(
            Update,
            (fly_cam::mouse_look, fly_cam::movement, fly_cam::interact),
        )
        .run();
}
