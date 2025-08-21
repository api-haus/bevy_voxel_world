use avian3d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::log::{Level, LogPlugin};
use bevy::pbr::{ExtendedMaterial, MaterialPlugin, StandardMaterial};
use bevy::prelude::*;
use bevy::window::WindowMode;
use bevy_enhanced_input::prelude;
use bevy_enhanced_input::prelude::InputContextAppExt;
use bevy_prng::WyRand;
use bevy_rand::plugin::EntropyPlugin;
use prelude::EnhancedInputPlugin;

use bevy_voxel_plugin::plugin::{TriplanarExtension, VoxelPlugin};
mod diag;

mod demo1;
mod fly_cam;

fn main() {
	App::new()
		.add_plugins((
			DefaultPlugins
				.set(LogPlugin {
					level: Level::DEBUG,
					filter: "wgpu=error,bevy_render=info,bevy_ecs=trace,vox=trace".to_string(),
					..Default::default()
				})
				.set(WindowPlugin {
					primary_window: Some(Window {
						resizable: false,
						mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
						recognize_rotation_gesture: true,
						prefers_home_indicator_hidden: true,
						prefers_status_bar_hidden: true,
						..default()
					}),
					..default()
				})
				.set(AssetPlugin {
					file_path: "../../assets".into(),
					..Default::default()
				}),
			PhysicsPlugins::default(),
			EnhancedInputPlugin,
			VoxelPlugin,
			EntropyPlugin::<WyRand>::default(),
			diag::DiagPlugin,
			diag::onscreen::OnScreenDiagPlugin,
			MaterialPlugin::<ExtendedMaterial<StandardMaterial, TriplanarExtension>>::default(),
		))
		.add_input_context::<fly_cam::FlyCamCtx>()
		.add_systems(Startup, fly_cam::setup)
		.add_systems(
			Update,
			(fly_cam::mouse_look, fly_cam::movement, fly_cam::interact),
		)
		.run();
}
