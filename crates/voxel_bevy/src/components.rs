//! Bevy components for voxel rendering.

use bevy::prelude::*;
use voxel_plugin::octree::OctreeNode;
use voxel_plugin::world::WorldId;

/// Component for mesh entities representing octree chunks.
///
/// Each chunk belongs to a specific voxel world and represents
/// one octree node's mesh.
#[derive(Component)]
pub struct VoxelChunk {
  /// The world this chunk belongs to.
  pub world_id: WorldId,
  /// The octree node this mesh represents.
  pub node: OctreeNode,
}

/// Marker component for entities that drive LOD refinement.
///
/// Attach to any entity with a `GlobalTransform` (typically a camera)
/// to make it the viewer for voxel LOD calculations.
///
/// # Example
/// ```ignore
/// commands.spawn((
///     Camera3d::default(),
///     Transform::from_xyz(0.0, 50.0, 0.0),
///     VoxelViewer,
/// ));
/// ```
#[derive(Component, Default)]
pub struct VoxelViewer;
