use bevy::prelude::*;

use avian3d::prelude::*;

use bevy_enhanced_input::prelude::InputContextAppExt;

mod demo1;
mod fly_cam;
mod rayon_chunks;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default(),
            bevy_enhanced_input::prelude::EnhancedInputPlugin,
            bevister::plugin::VoxelPlugin,
        ))
        .add_input_context::<fly_cam::FlyCamCtx>()
        .add_systems(Startup, fly_cam::setup)
        .add_systems(Update, (fly_cam::mouse_look, fly_cam::movement))
        .run();
}
