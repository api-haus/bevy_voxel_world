//! Bevy presentation layer for voxel_plugin.
//!
//! This crate bridges the engine-independent voxel_plugin with Bevy,
//! providing mesh rendering and LOD management.

pub mod components;
pub mod entity_queue;
pub mod fly_camera;
pub mod input;
pub mod resources;
pub mod systems;
pub mod triplanar_material;
pub mod world;

#[cfg(feature = "debug_ui")]
pub mod debug_ui;

#[cfg(test)]
mod presentation_test;

#[cfg(test)]
mod consistency_test;

use bevy::prelude::*;
pub use components::*;
pub use entity_queue::{EntityQueue, EntityQueueConfig, QueueStats};
pub use fly_camera::*;
pub use input::{fly_camera_input_bundle, CameraInputContext, CameraInputPlugin};
pub use resources::*;
pub use systems::entities::{mesh_output_to_bevy, spawn_chunk_entity, spawn_triplanar_chunk_entity};
pub use triplanar_material::{
  create_placeholder_material, load_baked_terrain_material, TerrainTextureConfig,
  TriplanarMaterial, TriplanarMaterialPlugin, TriplanarParams, LAYER_COLORS, NUM_LAYERS,
};
pub use world::{VoxelWorldRoot, WorldChunkMap};

// Re-export metrics types for convenience
pub use voxel_plugin::metrics::{WorldMetrics, RollingWindow};

/// Bevy plugin for voxel LOD rendering.
pub struct VoxelBevyPlugin;

impl Plugin for VoxelBevyPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_plugins(CameraInputPlugin)
      .add_plugins(TriplanarMaterialPlugin)
      .add_systems(Startup, systems::startup::setup_octree_scene);
    // Note: fly_camera systems are registered by CameraInputPlugin
  }
}
