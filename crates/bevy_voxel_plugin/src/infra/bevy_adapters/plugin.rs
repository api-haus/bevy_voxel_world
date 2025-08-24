//! Bevy plugin implementation

use super::materials;
use super::systems;
use bevy::prelude::*;

/// The main Bevy plugin for the voxel system
pub struct BevyVoxelPlugin;

impl Plugin for BevyVoxelPlugin {
	fn build(&self, app: &mut App) {
		// Register resources
		app
			.init_resource::<super::VoxelVolumeConfig>()
			.init_resource::<super::MeshingBudget>();

		// Register events
		app
			.add_event::<super::VoxelEditEvent>()
			.add_event::<super::MeshReady>();

		// Systems
		app.add_systems(
			Startup,
			(
				systems::spawn_volume_system,
				materials::init_voxel_materials_system,
			),
		);

		app.add_systems(
			Update,
			(
				systems::handle_edit_events_system,
				systems::process_meshing_queue_system,
				systems::apply_generated_meshes_system.after(systems::process_meshing_queue_system),
			),
		);
	}
}
