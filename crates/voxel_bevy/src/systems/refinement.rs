//! LOD refinement system driven by VoxelViewer.

use bevy::prelude::*;
use voxel_plugin::octree::{refine, OctreeNode, RefinementBudget, RefinementInput};

use crate::components::{VoxelChunk, VoxelViewer};
use crate::world::{VoxelWorldRoot, WorldChunkMap};

/// System that refines octree LOD based on viewer position.
///
/// Runs each frame, checking viewer distance to chunks and
/// subdividing or coarsening as needed. Returns transition info
/// for the presentation sync system.
pub fn refine_octree_lod(
  viewers: Query<&GlobalTransform, With<VoxelViewer>>,
  mut worlds: Query<&mut VoxelWorldRoot>,
) {
  // Get first viewer position (support for multiple viewers could average them)
  let Some(viewer_transform) = viewers.iter().next() else {
    return;
  };

  let viewer_pos = viewer_transform.translation();
  let viewer_pos_d = bevy::math::DVec3::new(
    viewer_pos.x as f64,
    viewer_pos.y as f64,
    viewer_pos.z as f64,
  );

  // Refine each world
  for mut world_root in &mut worlds {
    let input = RefinementInput {
      viewer_pos: viewer_pos_d,
      config: world_root.config().clone(),
      prev_leaves: world_root.world.leaves.as_set().clone(),
      budget: RefinementBudget {
        max_subdivisions: 8,
        max_collapses: 8,
        ..RefinementBudget::DEFAULT
      },
    };

    let output = refine(input);

    // Apply transitions
    if !output.transition_groups.is_empty() {
      for group in &output.transition_groups {
        for node in &group.nodes_to_remove {
          world_root.world.leaves.remove(node);
        }
        for node in &group.nodes_to_add {
          world_root.world.leaves.insert(*node);
        }
      }
    }
  }
}

/// Pending chunk operations for presentation sync.
#[derive(Resource, Default)]
pub struct PendingChunkOps {
  /// Nodes to spawn meshes for.
  pub to_spawn: Vec<(voxel_plugin::world::WorldId, OctreeNode)>,
  /// Entities to despawn.
  pub to_despawn: Vec<Entity>,
}

/// System that syncs chunk entities to octree leaves.
///
/// Compares WorldChunkMap to VoxelWorldRoot.leaves and queues
/// spawn/despawn operations.
pub fn sync_chunks_to_leaves(
  mut commands: Commands,
  worlds: Query<&VoxelWorldRoot>,
  chunks: Query<(Entity, &VoxelChunk)>,
  mut world_chunk_map: ResMut<WorldChunkMap>,
  mut pending: ResMut<PendingChunkOps>,
) {
  pending.to_spawn.clear();
  pending.to_despawn.clear();

  for world_root in &worlds {
    let world_id = world_root.id();
    let leaves = world_root.world.leaves.as_set();

    // Find chunks to despawn (in map but not in leaves)
    if let Some(chunk_nodes) = world_chunk_map.get_world_chunks(world_id) {
      for (node, &entity) in chunk_nodes.iter() {
        if !leaves.contains(node) {
          pending.to_despawn.push(entity);
        }
      }
    }

    // Find nodes to spawn (in leaves but not in map)
    for node in leaves.iter() {
      if !world_chunk_map.contains(world_id, node) {
        pending.to_spawn.push((world_id, *node));
      }
    }
  }

  // Despawn removed chunks
  for entity in &pending.to_despawn {
    // Get chunk info before despawning
    if let Ok((_, chunk)) = chunks.get(*entity) {
      world_chunk_map.remove(chunk.world_id, &chunk.node);
    }
    commands.entity(*entity).despawn();
  }

  // Note: Spawning new chunks requires noise sampling and meshing,
  // which should be done in a separate system with parallel processing.
  // For now, just track what needs spawning.
}
