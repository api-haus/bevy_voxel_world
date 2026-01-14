//! Bevy presentation layer for voxel_plugin.
//!
//! This crate bridges the engine-independent voxel_plugin with Bevy,
//! providing mesh rendering, LOD management, and procedural noise generation.

pub mod components;
pub mod fly_camera;
pub mod noise;
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
pub use noise::*;
pub use resources::*;
pub use world::{VoxelWorldRoot, WorldChunkMap};

/// Bevy plugin for voxel LOD rendering.
pub struct VoxelBevyPlugin;

impl Plugin for VoxelBevyPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_systems(Startup, systems::startup::setup_octree_scene)
      .add_systems(Update, fly_camera::update_fly_camera);
  }
}
