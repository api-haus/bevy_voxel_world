//! Bevy presentation layer for voxel_plugin.
//!
//! This crate bridges the engine-independent voxel_plugin with Bevy,
//! providing mesh rendering and LOD management.

pub mod components;
pub mod fly_camera;
pub mod input;
pub mod resources;
pub mod systems;
pub mod world;

#[cfg(test)]
mod presentation_test;

#[cfg(test)]
mod consistency_test;

use bevy::prelude::*;
pub use components::*;
pub use fly_camera::*;
pub use input::{fly_camera_input_bundle, CameraInputContext, CameraInputPlugin};
pub use resources::*;
pub use world::{VoxelWorldRoot, WorldChunkMap};

/// Bevy plugin for voxel LOD rendering.
pub struct VoxelBevyPlugin;

impl Plugin for VoxelBevyPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(CameraInputPlugin)
			.add_systems(Startup, systems::startup::setup_octree_scene);
		// Note: fly_camera systems are registered by CameraInputPlugin
	}
}
