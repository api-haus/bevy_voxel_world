//! Entity management for voxel chunks.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::Material;
use bevy::prelude::*;
use voxel_plugin::octree::{OctreeConfig, OctreeNode};
use voxel_plugin::types::MeshOutput;
use voxel_plugin::world::WorldId;

use crate::components::VoxelChunk;
use crate::resources::ChunkEntityMap;
use crate::world::WorldChunkMap;

/// Material blend weights are stored in vertex color (RGBA = 4 layer weights).
/// This uses Bevy's standard ATTRIBUTE_COLOR for compatibility with
/// ExtendedMaterial and GPU batching.

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
  let cell_size = config.get_cell_size(node.lod);

  // Debug: log chunk positioning for chunks near origin at any LOD
  if node.x.abs() <= 1 && node.y.abs() <= 1 && node.z.abs() <= 1 {
    info!(
      "Chunk node=({},{},{}) LOD{}: world_min=({:.1}, {:.1}, {:.1}), voxel_size={:.1}, cell_size={:.1}, mesh_bounds_x=[{:.1}, {:.1}]",
      node.x, node.y, node.z, node.lod,
      world_min.x, world_min.y, world_min.z,
      voxel_size,
      cell_size,
      output.bounds.min[0],
      output.bounds.max[0]
    );
  }

  // Transform position = node_min (matches C# OctreeTransform.GetWorldPosition)
  // Mesh vertices are in local [0, ~31] coords, scaled by voxel_size via transform.
  // No offset needed - sample 0 is at node_min, mesh vertex 0 should appear at node_min.
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

/// Spawn a mesh entity with a custom material for an octree node.
///
/// Generic version that works with any Material type (e.g., triplanar terrain materials).
/// Material blend weights are passed via vertex colors.
pub fn spawn_custom_material_chunk_entity<M: Material>(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  material: Handle<M>,
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
pub fn mesh_output_to_bevy(output: &MeshOutput) -> Mesh {
  let mut mesh = Mesh::new(
    PrimitiveTopology::TriangleList,
    RenderAssetUsages::default(),
  );

  if output.is_empty() {
    return mesh;
  }

  let positions: Vec<[f32; 3]> = output.vertices.iter().map(|v| v.position).collect();
  let normals: Vec<[f32; 3]> = output.vertices.iter().map(|v| v.normal).collect();
  let material_weights: Vec<[f32; 4]> =
    output.vertices.iter().map(|v| v.material_weights).collect();

  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
  // Material blend weights stored as vertex color (RGBA = 4 layer weights)
  mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, material_weights);
  mesh.insert_indices(Indices::U16(output.indices.clone()));

  mesh
}
