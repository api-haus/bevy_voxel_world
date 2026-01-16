//! Tests for Stage 2: Presample
//!
//! Sample full 32³ volume, check homogeneity.

use super::{presample_batch, presample_node};
use crate::constants::SAMPLE_SIZE_CB;
use crate::octree::OctreeNode;
use crate::pipeline::test_utils::*;
use crate::pipeline::types::{VolumeSampler, WorkSource};
use crate::types::{MaterialId, SdfSample};

// =============================================================================
// Batch 1: Homogeneity Detection
// =============================================================================

#[test]
fn test_all_solid_returns_none() {
  let sampler = ConstantSampler::all_solid();
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 2);

  let output = presample_node(node, WorkSource::Refinement, &sampler, &config);

  assert_eq!(output.node, node);
  assert!(
    output.volume.is_none(),
    "Homogeneous solid should return None"
  );
}

#[test]
fn test_all_air_returns_none() {
  let sampler = ConstantSampler::all_air();
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 2);

  let output = presample_node(node, WorkSource::Refinement, &sampler, &config);

  assert_eq!(output.node, node);
  assert!(
    output.volume.is_none(),
    "Homogeneous air should return None"
  );
}

#[test]
fn test_mixed_returns_volume() {
  let sampler = CornerSampler::mixed();
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 0);

  let output = presample_node(
    node,
    WorkSource::Refinement,
    &sampler.for_node(&node, &config),
    &config,
  );

  assert_eq!(output.node, node);
  let vol = output.volume.expect("Mixed volume should return Some");
  assert_eq!(vol.volume.len(), SAMPLE_SIZE_CB);
  assert_eq!(vol.materials.len(), SAMPLE_SIZE_CB);
}

#[test]
fn test_single_difference_returns_volume() {
  // 7 negative + 1 positive → surface might exist
  let sampler = CornerSampler::mostly_solid();
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 0);

  let output = presample_node(
    node,
    WorkSource::Refinement,
    &sampler.for_node(&node, &config),
    &config,
  );

  assert!(output.volume.is_some(), "7-1 split should return volume");
}

// =============================================================================
// Batch 2: Edge Cases
// =============================================================================

#[test]
fn test_zero_sdf_counts_as_surface() {
  // SDF exactly 0 (on surface) at one corner → should NOT skip
  let sampler = CornerSampler::with_zero_corner();
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 0);

  let output = presample_node(
    node,
    WorkSource::Refinement,
    &sampler.for_node(&node, &config),
    &config,
  );

  assert!(
    output.volume.is_some(),
    "Zero SDF sample should produce volume"
  );
}

#[test]
fn test_small_negative_is_homogeneous() {
  let sampler = ConstantSampler::with_value(-1);
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 2);

  let output = presample_node(node, WorkSource::Refinement, &sampler, &config);

  assert!(
    output.volume.is_none(),
    "Small negative should be homogeneous"
  );
}

#[test]
fn test_small_positive_is_homogeneous() {
  let sampler = ConstantSampler::with_value(1);
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 2);

  let output = presample_node(node, WorkSource::Refinement, &sampler, &config);

  assert!(
    output.volume.is_none(),
    "Small positive should be homogeneous"
  );
}

// =============================================================================
// Batch 3: Volume Sampling Correctness
// =============================================================================

#[test]
fn test_volume_samples_32_cubed_points() {
  let inner = SphereSampler::at_origin(15.0);
  let sampler = CountingSampler::new(inner);
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 0);

  let output = presample_node(node, WorkSource::Refinement, &sampler, &config);

  let vol = output.volume.expect("Sphere should produce volume");
  assert_eq!(vol.volume.len(), SAMPLE_SIZE_CB);

  // Presample calls sample_volume once per node
  let count = sampler.count();
  assert_eq!(count, 1, "Should call sample_volume exactly once per node");
}

#[test]
fn test_homogeneous_still_samples_full_volume() {
  // Presample: we always sample full volume, then check homogeneity
  let inner = ConstantSampler::all_air();
  let sampler = CountingSampler::new(inner);
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 2);

  let output = presample_node(node, WorkSource::Refinement, &sampler, &config);

  assert!(output.volume.is_none(), "Should detect as homogeneous");
  let count = sampler.count();
  assert_eq!(count, 1, "Should still call sample_volume once");
}

// =============================================================================
// Batch 4: WorkSource Preservation
// =============================================================================

#[test]
fn test_presample_preserves_refinement_work_source() {
  let sampler = ConstantSampler::all_air();
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 2);

  let output = presample_node(node, WorkSource::Refinement, &sampler, &config);

  assert_eq!(output.work_source, WorkSource::Refinement);
}

#[test]
fn test_presample_preserves_invalidation_work_source() {
  let sampler = ConstantSampler::all_air();
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 2);

  let output = presample_node(node, WorkSource::Invalidation, &sampler, &config);

  assert_eq!(output.work_source, WorkSource::Invalidation);
}

// =============================================================================
// Batch 5: Batch Processing
// =============================================================================

#[test]
fn test_presample_batch_processes_multiple_nodes() {
  let sampler = ConstantSampler::all_air();
  let config = test_config();
  let nodes = vec![
    (OctreeNode::new(0, 0, 0, 2), WorkSource::Refinement),
    (OctreeNode::new(1, 0, 0, 2), WorkSource::Refinement),
    (OctreeNode::new(0, 1, 0, 2), WorkSource::Invalidation),
  ];

  let outputs = presample_batch(nodes.clone(), &sampler, &config);

  assert_eq!(outputs.len(), 3);
  for (i, output) in outputs.iter().enumerate() {
    assert_eq!(output.node, nodes[i].0);
    assert_eq!(output.work_source, nodes[i].1);
  }
}

#[test]
fn test_presample_batch_empty_input() {
  let sampler = ConstantSampler::all_air();
  let config = test_config();
  let nodes: Vec<(OctreeNode, WorkSource)> = vec![];

  let outputs = presample_batch(nodes, &sampler, &config);

  assert!(outputs.is_empty());
}

#[test]
fn test_presample_batch_mixed_results() {
  let plane = PlaneSampler::horizontal(16.0);
  let config = test_config();

  let nodes = vec![
    (OctreeNode::new(0, 0, 0, 0), WorkSource::Refinement), // Intersects plane
    (OctreeNode::new(0, 100, 0, 0), WorkSource::Refinement), // All air
  ];

  let outputs = presample_batch(nodes, &plane, &config);

  assert_eq!(outputs.len(), 2);
  assert!(
    outputs[0].volume.is_some(),
    "Node at y=0 should have volume"
  );
  assert!(
    outputs[1].volume.is_none(),
    "Node at y=100 should be homogeneous"
  );
}

// =============================================================================
// Batch 6: Sampling Position Correctness
// =============================================================================

#[test]
fn test_samples_at_correct_positions_lod_0() {
  let config = test_config();
  let node = OctreeNode::new(0, 0, 0, 0);

  struct PositionRecorder {
    grid_offset: std::sync::Mutex<Option<[i64; 3]>>,
    voxel_size: std::sync::Mutex<Option<f64>>,
  }
  impl VolumeSampler for PositionRecorder {
    fn sample_volume(
      &self,
      grid_offset: [i64; 3],
      voxel_size: f64,
      volume: &mut [SdfSample; SAMPLE_SIZE_CB],
      materials: &mut [MaterialId; SAMPLE_SIZE_CB],
    ) {
      *self.grid_offset.lock().unwrap() = Some(grid_offset);
      *self.voxel_size.lock().unwrap() = Some(voxel_size);
      volume.fill(-10);
      materials.fill(0);
    }
  }

  let sampler = PositionRecorder {
    grid_offset: std::sync::Mutex::new(None),
    voxel_size: std::sync::Mutex::new(None),
  };

  let _ = presample_node(node, WorkSource::Refinement, &sampler, &config);

  // Check grid offset is computed correctly: grid_offset = round(node_min / voxel_size)
  // For node at (0,0,0) LOD 0: min = (0,0,0), voxel_size = 1.0, grid_offset = [0,0,0]
  let min = config.get_node_min(&node);
  let voxel_size = config.get_voxel_size(node.lod);
  let expected_offset = [
    (min.x / voxel_size).round() as i64,
    (min.y / voxel_size).round() as i64,
    (min.z / voxel_size).round() as i64,
  ];
  let recorded_offset = sampler
    .grid_offset
    .lock()
    .unwrap()
    .expect("Should have recorded grid_offset");
  assert_eq!(
    recorded_offset, expected_offset,
    "Grid offset should be {:?}, got {:?}",
    expected_offset, recorded_offset
  );

  // Check voxel size is correct for LOD 0
  let expected_voxel_size = config.get_voxel_size(node.lod);
  let recorded_voxel_size = sampler
    .voxel_size
    .lock()
    .unwrap()
    .expect("Should have recorded voxel size");
  assert!(
    (recorded_voxel_size - expected_voxel_size).abs() < 0.001,
    "Voxel size should be {} at LOD 0, got {}",
    expected_voxel_size,
    recorded_voxel_size
  );
}

#[test]
fn test_samples_at_correct_positions_lod_3() {
  let config = test_config();
  let node = OctreeNode::new(1, 1, 1, 3);

  let min = config.get_node_min(&node);
  let cell_size = config.get_cell_size(node.lod);

  // LOD 3 = 2^3 = 8x scale
  // Cell size = 28 * 1.0 * 8 = 224
  let expected_size = 28.0 * 8.0;
  let expected_min = 224.0;

  assert!(
    (min.x - expected_min).abs() < 0.001,
    "Expected min.x = {}, got {}",
    expected_min,
    min.x
  );
  assert!(
    (cell_size - expected_size).abs() < 0.001,
    "Expected cell_size = {}, got {}",
    expected_size,
    cell_size
  );
}
