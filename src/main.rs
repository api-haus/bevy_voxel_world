use bevy::prelude::*;

use avian3d::prelude::*;

use bevy::pbr::{ExtendedMaterial, MaterialPlugin, StandardMaterial};
use bevy_enhanced_input::prelude::InputContextAppExt;
use iyes_perf_ui::PerfUiPlugin;

mod demo1;
mod fly_cam;
mod rayon_chunks;

fn main() {
    App::new()
        // we want Bevy to measure these values for us:
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin)
        .add_plugins((
            DefaultPlugins,
            PerfUiPlugin,
            PhysicsPlugins::default(),
            bevy_enhanced_input::prelude::EnhancedInputPlugin,
            bevister::plugin::VoxelPlugin,
        ))
        .add_plugins(MaterialPlugin::<
            ExtendedMaterial<StandardMaterial, bevister::plugin::TriplanarExtension>,
        >::default())
        .add_input_context::<fly_cam::FlyCamCtx>()
        .add_systems(Startup, fly_cam::setup)
        .add_systems(
            Update,
            (fly_cam::mouse_look, fly_cam::movement, fly_cam::interact),
        )
        .run();
}
