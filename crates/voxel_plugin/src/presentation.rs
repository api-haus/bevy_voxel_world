//! PresentationLayer - callback interface for engine bridges.
//!
//! This trait allows the core voxel pipeline to notify engine-specific code
//! (Bevy, Unity, Godot) about chunk lifecycle events without depending on
//! any specific engine.

use crate::octree::OctreeNode;
use crate::pipeline::{MeshData, PresentationHint};
use crate::world::WorldId;

/// Callback interface for engine bridges.
///
/// Implementations handle chunk lifecycle events from the voxel pipeline.
/// Must be thread-safe as callbacks may be invoked from worker threads.
///
/// # Example (Bevy)
///
/// ```ignore
/// struct BevyPresentation {
///     chunk_spawner: ChunkSpawner,
/// }
///
/// impl PresentationLayer for BevyPresentation {
///     fn on_chunk_ready(&self, world_id: WorldId, node: OctreeNode,
///                       mesh_data: MeshData, hint: PresentationHint) {
///         self.chunk_spawner.spawn(world_id, node, mesh_data, hint);
///     }
///     // ...
/// }
/// ```
pub trait PresentationLayer: Send + Sync {
  /// Called when a chunk mesh is ready to be displayed.
  ///
  /// # Arguments
  /// - `world_id`: The world this chunk belongs to
  /// - `node`: The octree node identifier
  /// - `mesh_data`: Serialized mesh data (vertices, indices, bounds)
  /// - `hint`: How to present the chunk (Immediate, FadeIn, FadeOut)
  fn on_chunk_ready(
    &self,
    world_id: WorldId,
    node: OctreeNode,
    mesh_data: MeshData,
    hint: PresentationHint,
  );

  /// Called when a chunk should be removed from display.
  ///
  /// # Arguments
  /// - `world_id`: The world this chunk belongs to
  /// - `node`: The octree node to remove
  fn on_chunk_remove(&self, world_id: WorldId, node: OctreeNode);

  /// Called when a world is being destroyed.
  ///
  /// Implementations should clean up all chunks belonging to this world.
  ///
  /// # Arguments
  /// - `world_id`: The world being destroyed
  fn on_world_destroy(&self, world_id: WorldId);
}

/// No-op implementation for testing and headless operation.
pub struct NullPresentation;

impl PresentationLayer for NullPresentation {
  fn on_chunk_ready(
    &self,
    _world_id: WorldId,
    _node: OctreeNode,
    _mesh_data: MeshData,
    _hint: PresentationHint,
  ) {
    // No-op
  }

  fn on_chunk_remove(&self, _world_id: WorldId, _node: OctreeNode) {
    // No-op
  }

  fn on_world_destroy(&self, _world_id: WorldId) {
    // No-op
  }
}
