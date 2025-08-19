use bevy::prelude::*;

use avian3d::prelude::*;

use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::*;

mod demo1;
mod rayon_chunks;
mod chunks_bevy;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default(),
            // We need both Tnua's main controller plugin, and the plugin to connect to the physics
            // backend (in this case Avian 3D)
            TnuaControllerPlugin::new(FixedUpdate),
            TnuaAvian3dPlugin::new(FixedUpdate),
        ))
        .add_systems(
            Startup,
            (
                demo1::setup_camera_and_lights,
                demo1::setup_level,
                demo1::setup_player,
                chunks_bevy::setup_chunks,
            ),
        )
        .add_systems(
            FixedUpdate,
            demo1::apply_controls.in_set(TnuaUserControlsSystemSet),
        )
        .run();
}

