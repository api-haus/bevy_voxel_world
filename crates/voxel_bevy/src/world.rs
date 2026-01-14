//! World isolation for Bevy - multi-world voxel support.
//!
//! Each voxel world is an entity with a `VoxelWorldRoot` component.
//! Chunks track which world they belong to via `WorldId`.

use std::collections::HashMap;

use bevy::prelude::*;
use voxel_plugin::octree::{OctreeConfig, OctreeNode};
use voxel_plugin::pipeline::VolumeSampler;
use voxel_plugin::world::WorldId;
use voxel_plugin::VoxelWorld;

// =============================================================================
// VoxelWorldRoot - Component wrapping VoxelWorld for ECS
// =============================================================================

/// Component wrapping a VoxelWorld for Bevy ECS.
///
/// Uses type-erased sampler (`Box<dyn VolumeSampler>`) for ECS uniform
/// component types. Each entity with this component represents an independent
/// voxel world.
///
/// # Example
///
/// ```ignore
/// commands.spawn((
///     VoxelWorldRoot::new(config, Box::new(my_sampler)),
///     Transform::default(),
/// ));
/// ```
#[derive(Component)]
pub struct VoxelWorldRoot {
  /// The underlying voxel world state.
  pub world: VoxelWorld<Box<dyn VolumeSampler>>,
}

impl VoxelWorldRoot {
  /// Create a new voxel world root with the given config and sampler.
  pub fn new(config: OctreeConfig, sampler: Box<dyn VolumeSampler>) -> Self {
    Self {
      world: VoxelWorld::new(config, sampler),
    }
  }

  /// Create a new voxel world root with initial leaves at the given LOD.
  pub fn new_with_initial_lod(
    config: OctreeConfig,
    sampler: Box<dyn VolumeSampler>,
    initial_lod: i32,
  ) -> Self {
    Self {
      world: VoxelWorld::new_with_initial_lod(config, sampler, initial_lod),
    }
  }

  /// Get the world's unique identifier.
  #[inline]
  pub fn id(&self) -> WorldId {
    self.world.id
  }

  /// Get the world's octree configuration.
  #[inline]
  pub fn config(&self) -> &OctreeConfig {
    &self.world.config
  }
}

// =============================================================================
// WorldChunkMap - Resource for chunk entity lookup
// =============================================================================

/// Resource mapping (WorldId, OctreeNode) to chunk Entity.
///
/// Enables O(1) lookup of chunk entities by world and node.
/// Used for chunk updates, removal, and world cleanup.
#[derive(Resource, Default)]
pub struct WorldChunkMap {
  /// Outer map: WorldId -> inner map
  /// Inner map: OctreeNode -> Entity
  worlds: HashMap<WorldId, HashMap<OctreeNode, Entity>>,
}

impl WorldChunkMap {
  /// Insert a chunk entity for a world/node pair.
  pub fn insert(&mut self, world_id: WorldId, node: OctreeNode, entity: Entity) {
    self
      .worlds
      .entry(world_id)
      .or_default()
      .insert(node, entity);
  }

  /// Remove a chunk entity for a world/node pair.
  pub fn remove(&mut self, world_id: WorldId, node: &OctreeNode) -> Option<Entity> {
    self.worlds.get_mut(&world_id)?.remove(node)
  }

  /// Get a chunk entity for a world/node pair.
  pub fn get(&self, world_id: WorldId, node: &OctreeNode) -> Option<Entity> {
    self.worlds.get(&world_id)?.get(node).copied()
  }

  /// Check if a chunk exists for a world/node pair.
  pub fn contains(&self, world_id: WorldId, node: &OctreeNode) -> bool {
    self
      .worlds
      .get(&world_id)
      .is_some_and(|nodes| nodes.contains_key(node))
  }

  /// Get all chunk entities for a world.
  pub fn get_world_chunks(&self, world_id: WorldId) -> Option<&HashMap<OctreeNode, Entity>> {
    self.worlds.get(&world_id)
  }

  /// Remove all chunks for a world, returning all entities.
  pub fn remove_world(&mut self, world_id: WorldId) -> Vec<Entity> {
    self
      .worlds
      .remove(&world_id)
      .map(|nodes| nodes.into_values().collect())
      .unwrap_or_default()
  }

  /// Get the number of chunks across all worlds.
  pub fn total_chunks(&self) -> usize {
    self.worlds.values().map(|m| m.len()).sum()
  }

  /// Get the number of worlds being tracked.
  pub fn world_count(&self) -> usize {
    self.worlds.len()
  }
}

// =============================================================================
// Systems
// =============================================================================

/// System to sync Bevy Transform to VoxelWorld transform.
///
/// Runs when a VoxelWorldRoot entity's GlobalTransform changes.
pub fn sync_world_transforms(
  mut worlds: Query<(&mut VoxelWorldRoot, &GlobalTransform), Changed<GlobalTransform>>,
) {
  use bevy::math::{DAffine3, DMat3, DVec3};

  for (mut root, gt) in &mut worlds {
    // Convert Bevy's Affine3A (f32) to DAffine3 (f64)
    let affine = gt.affine();
    let transform = DAffine3::from_mat3_translation(
      DMat3::from_cols(
        DVec3::new(
          affine.matrix3.x_axis.x as f64,
          affine.matrix3.x_axis.y as f64,
          affine.matrix3.x_axis.z as f64,
        ),
        DVec3::new(
          affine.matrix3.y_axis.x as f64,
          affine.matrix3.y_axis.y as f64,
          affine.matrix3.y_axis.z as f64,
        ),
        DVec3::new(
          affine.matrix3.z_axis.x as f64,
          affine.matrix3.z_axis.y as f64,
          affine.matrix3.z_axis.z as f64,
        ),
      ),
      DVec3::new(
        affine.translation.x as f64,
        affine.translation.y as f64,
        affine.translation.z as f64,
      ),
    );
    root.world.set_transform(transform);
  }
}

/// System to cleanup chunk entities when a VoxelWorldRoot is despawned.
pub fn cleanup_despawned_worlds(
  mut removed: RemovedComponents<VoxelWorldRoot>,
  commands: Commands,
  chunk_map: ResMut<WorldChunkMap>,
  chunks: Query<(Entity, &crate::components::VoxelChunk)>,
) {
  for _entity in removed.read() {
    // Find all chunks that belong to despawned worlds
    // Note: We need the WorldId from the component before it was removed
    // This is a limitation - we'll clean up based on orphaned chunks instead
  }

  // Alternative: periodically clean up chunks whose world no longer exists
  // This would require storing WorldId in VoxelChunk (which we do in the updated
  // component)
  let _ = (commands, chunk_map, chunks); // Silence unused warnings for now
}

#[cfg(test)]
#[path = "world_test.rs"]
mod world_test;
