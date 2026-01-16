//! Tests for FastNoise2Terrain sampler edge coherency.
//!
//! These tests verify that adjacent chunks produce identical SDF values
//! at their shared edges when sampled through the full pipeline.

use super::FastNoise2Terrain;
use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::octree::{OctreeConfig, OctreeNode};
use crate::pipeline::{sample_volume_for_node, VolumeSampler};

/// Test edge coherency at voxel_size=1.0 through the full pipeline.
#[test]
fn test_terrain_edge_coherency_full_pipeline() {
  let sampler = FastNoise2Terrain::new(1337);
  let config = OctreeConfig {
    voxel_size: 1.0,
    world_origin: glam::DVec3::ZERO,
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
  };

  // Sample two adjacent chunks in X
  let node_a = OctreeNode::new(0, 0, 0, 0);
  let node_b = OctreeNode::new(1, 0, 0, 0);

  let sampled_a = sample_volume_for_node(&node_a, &sampler, &config);
  let sampled_b = sample_volume_for_node(&node_b, &sampler, &config);

  // Compare overlapping edge samples
  // Node A's samples at x=28..31 should match Node B's samples at x=0..3
  // Volume layout: X-slowest (vol_idx = x * SIZE² + y * SIZE + z)
  let mut mismatches = 0;
  let mut max_diff: i16 = 0;

  for y in 0..SAMPLE_SIZE {
    for z in 0..SAMPLE_SIZE {
      for overlap_idx in 0..4 {
        let a_x = 28 + overlap_idx;
        let b_x = overlap_idx;

        // Volume X-slowest index
        let a_idx = a_x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;
        let b_idx = b_x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;

        let a_val = sampled_a.volume[a_idx] as i16;
        let b_val = sampled_b.volume[b_idx] as i16;
        let diff = (a_val - b_val).abs();

        if diff > 0 {
          mismatches += 1;
          max_diff = max_diff.max(diff);
          if mismatches <= 5 {
            eprintln!(
              "Mismatch at overlap_idx={}, y={}, z={}: a={}, b={}, diff={}",
              overlap_idx, y, z, a_val, b_val, diff
            );
          }
        }
      }
    }
  }

  assert_eq!(
    mismatches, 0,
    "Found {} edge sample mismatches at voxel_size=1.0 (max diff: {})",
    mismatches, max_diff
  );
}

/// Test edge coherency at voxel_size=0.25 through the full pipeline.
#[test]
fn test_terrain_edge_coherency_small_voxel() {
  let sampler = FastNoise2Terrain::new(1337);
  let config = OctreeConfig {
    voxel_size: 0.25,
    world_origin: glam::DVec3::ZERO,
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
  };

  // Sample two adjacent chunks in X
  let node_a = OctreeNode::new(0, 0, 0, 0);
  let node_b = OctreeNode::new(1, 0, 0, 0);

  let sampled_a = sample_volume_for_node(&node_a, &sampler, &config);
  let sampled_b = sample_volume_for_node(&node_b, &sampler, &config);

  // Compare overlapping edge samples
  let mut mismatches = 0;
  let mut max_diff: i16 = 0;

  for y in 0..SAMPLE_SIZE {
    for z in 0..SAMPLE_SIZE {
      for overlap_idx in 0..4 {
        let a_x = 28 + overlap_idx;
        let b_x = overlap_idx;

        let a_idx = a_x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;
        let b_idx = b_x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;

        let a_val = sampled_a.volume[a_idx] as i16;
        let b_val = sampled_b.volume[b_idx] as i16;
        let diff = (a_val - b_val).abs();

        if diff > 0 {
          mismatches += 1;
          max_diff = max_diff.max(diff);
          if mismatches <= 5 {
            eprintln!(
              "Mismatch at overlap_idx={}, y={}, z={}: a={}, b={}, diff={}",
              overlap_idx, y, z, a_val, b_val, diff
            );
          }
        }
      }
    }
  }

  assert_eq!(
    mismatches, 0,
    "Found {} edge sample mismatches at voxel_size=0.25 (max diff: {})",
    mismatches, max_diff
  );
}

/// Debug test: Print the world positions being sampled for adjacent chunks.
#[test]
fn test_debug_world_positions() {
  let config = OctreeConfig {
    voxel_size: 0.25,
    world_origin: glam::DVec3::ZERO,
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
  };

  let node_a = OctreeNode::new(0, 0, 0, 0);
  let node_b = OctreeNode::new(1, 0, 0, 0);

  let min_a = config.get_node_min(&node_a);
  let min_b = config.get_node_min(&node_b);

  let voxel_size = config.get_voxel_size(0);
  let cell_size = config.get_cell_size(0);

  eprintln!("voxel_size: {}", voxel_size);
  eprintln!("cell_size: {}", cell_size);
  eprintln!("node_a min: {:?}", min_a);
  eprintln!("node_b min: {:?}", min_b);

  // Grid offsets (used by sampler)
  let grid_a = [
    (min_a.x / voxel_size).round() as i64,
    (min_a.y / voxel_size).round() as i64,
    (min_a.z / voxel_size).round() as i64,
  ];
  let grid_b = [
    (min_b.x / voxel_size).round() as i64,
    (min_b.y / voxel_size).round() as i64,
    (min_b.z / voxel_size).round() as i64,
  ];

  eprintln!("grid_offset_a: {:?}", grid_a);
  eprintln!("grid_offset_b: {:?}", grid_b);

  // World positions for overlapping samples
  // Node A sample 28 should be at same world pos as Node B sample 0
  let a_sample_28_world = (grid_a[0] + 28) as f64 * voxel_size;
  let b_sample_0_world = (grid_b[0] + 0) as f64 * voxel_size;

  eprintln!("Node A sample 28 world X: {}", a_sample_28_world);
  eprintln!("Node B sample 0 world X: {}", b_sample_0_world);
  eprintln!("Difference: {}", (a_sample_28_world - b_sample_0_world).abs());

  // They should be equal for coherency
  assert!(
    (a_sample_28_world - b_sample_0_world).abs() < 1e-10,
    "World positions don't match: {} vs {}",
    a_sample_28_world,
    b_sample_0_world
  );
}
