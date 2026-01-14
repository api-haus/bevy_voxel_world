//! Tests for world isolation types.

use bevy::prelude::*;
use voxel_plugin::constants::SAMPLE_SIZE_CB;
use voxel_plugin::octree::OctreeConfig;
use voxel_plugin::octree::OctreeNode;
use voxel_plugin::pipeline::VolumeSampler;
use voxel_plugin::types::{MaterialId, SdfSample};
use voxel_plugin::world::WorldId;

use super::{VoxelWorldRoot, WorldChunkMap};

struct MockSampler;

impl VolumeSampler for MockSampler {
    fn sample_volume(
        &self,
        _sample_start: [f64; 3],
        _voxel_size: f64,
        volume: &mut [SdfSample; SAMPLE_SIZE_CB],
        materials: &mut [MaterialId; SAMPLE_SIZE_CB],
    ) {
        // Fill with all-air (positive values = outside)
        volume.fill(127);
        materials.fill(0);
    }
}

#[test]
fn test_world_chunk_map_insert_get() {
    let mut map = WorldChunkMap::default();
    let world_id = WorldId::new();
    let node = OctreeNode::new(0, 0, 0, 3);
    let entity = Entity::from_bits(42u64);

    map.insert(world_id, node, entity);

    assert_eq!(map.get(world_id, &node), Some(entity));
    assert!(map.contains(world_id, &node));
}

#[test]
fn test_world_chunk_map_remove() {
    let mut map = WorldChunkMap::default();
    let world_id = WorldId::new();
    let node = OctreeNode::new(0, 0, 0, 3);
    let entity = Entity::from_bits(42u64);

    map.insert(world_id, node, entity);
    let removed = map.remove(world_id, &node);

    assert_eq!(removed, Some(entity));
    assert!(!map.contains(world_id, &node));
}

#[test]
fn test_world_chunk_map_remove_world() {
    let mut map = WorldChunkMap::default();
    let world_id = WorldId::new();

    // Add multiple chunks to the world (start from 1, 0 is reserved)
    for i in 1..=5i32 {
        let node = OctreeNode::new(i, 0, 0, 3);
        let entity = Entity::from_bits((i as u64) << 32 | 1); // Valid entity bits
        map.insert(world_id, node, entity);
    }

    assert_eq!(map.total_chunks(), 5);

    let removed = map.remove_world(world_id);
    assert_eq!(removed.len(), 5);
    assert_eq!(map.total_chunks(), 0);
}

#[test]
fn test_voxel_world_root_creation() {
    let config = OctreeConfig::default();
    let root = VoxelWorldRoot::new(config, Box::new(MockSampler));

    assert!(root.world.leaves.is_empty());
}

#[test]
fn test_voxel_world_root_with_initial_lod() {
    let config = OctreeConfig::default();
    let root = VoxelWorldRoot::new_with_initial_lod(config, Box::new(MockSampler), 5);

    assert_eq!(root.world.leaves.len(), 1);
}
