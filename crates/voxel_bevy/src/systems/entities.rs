//! Entity management for voxel chunks.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use voxel_plugin::octree::{OctreeConfig, OctreeNode};
use voxel_plugin::types::MeshOutput;
use voxel_plugin::world::WorldId;

use crate::components::VoxelChunk;
use crate::resources::ChunkEntityMap;
use crate::world::WorldChunkMap;

/// Spawn a mesh entity for an octree node.
///
/// If `world_chunk_map` is provided, the chunk is also registered in the
/// world-aware chunk map for multi-world support.
pub fn spawn_chunk_entity(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  material: Handle<StandardMaterial>,
  chunk_map: &mut ChunkEntityMap,
  world_chunk_map: Option<&mut WorldChunkMap>,
  world_id: WorldId,
  node: OctreeNode,
  output: &MeshOutput,
  config: &OctreeConfig,
) -> Entity {
  let mesh = mesh_output_to_bevy(output);
  let mesh_handle = meshes.add(mesh);

  let world_min = config.get_node_min(&node);
  let voxel_size = config.get_voxel_size(node.lod) as f32;

  let entity = commands
    .spawn((
      Mesh3d(mesh_handle),
      MeshMaterial3d(material),
      Transform::from_translation(Vec3::new(
        world_min.x as f32,
        world_min.y as f32,
        world_min.z as f32,
      ))
      .with_scale(Vec3::splat(voxel_size)),
      VoxelChunk { world_id, node },
    ))
    .id();

  chunk_map.insert(node, entity);
  if let Some(wcm) = world_chunk_map {
    wcm.insert(world_id, node, entity);
  }
  entity
}

/// Despawn a chunk entity.
#[allow(dead_code)]
pub fn despawn_chunk_entity(
  commands: &mut Commands,
  chunk_map: &mut ChunkEntityMap,
  node: &OctreeNode,
) {
  if let Some(entity) = chunk_map.remove(node) {
    commands.entity(entity).despawn();
  }
}

/// Convert voxel_plugin MeshOutput to Bevy Mesh.
fn mesh_output_to_bevy(output: &MeshOutput) -> Mesh {
  let mut mesh = Mesh::new(
    PrimitiveTopology::TriangleList,
    RenderAssetUsages::default(),
  );

  if output.is_empty() {
    return mesh;
  }

  let positions: Vec<[f32; 3]> = output.vertices.iter().map(|v| v.position).collect();
  let normals: Vec<[f32; 3]> = output.vertices.iter().map(|v| v.normal).collect();

  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
  mesh.insert_indices(Indices::U32(output.indices.clone()));

  mesh
}
