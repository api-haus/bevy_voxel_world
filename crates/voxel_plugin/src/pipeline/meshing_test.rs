//! Tests for Stage 3: Meshing
//!
//! Thin wrapper around surface_nets::generate() that tracks timing
//! and preserves work_source through the pipeline.

use super::{mesh_batch, mesh_node};
use crate::constants::SAMPLE_SIZE_CB;
use crate::octree::OctreeNode;
use crate::pipeline::test_utils::*;
use crate::pipeline::types::{MeshInput, WorkSource};
use crate::types::MeshConfig;

// =============================================================================
// Batch 1: Single Node Meshing
// =============================================================================

#[test]
fn test_mesh_node_produces_mesh_from_sphere_volume() {
  let (volume, materials) = make_sphere_volume(12.0);
  let input = MeshInput {
    node: OctreeNode::new(0, 0, 0, 0),
    volume,
    materials,
    config: MeshConfig::default(),
    work_source: WorkSource::Refinement,
  };

  let result = mesh_node(input);

  assert!(
    !result.output.is_empty(),
    "Sphere volume should produce mesh"
  );
  assert!(result.output.vertices.len() > 0);
  assert!(result.output.indices.len() > 0);
}

#[test]
fn test_mesh_node_preserves_node_identity() {
  let (volume, materials) = make_sphere_volume(12.0);
  let node = OctreeNode::new(5, 3, 2, 2);
  let input = MeshInput {
    node,
    volume,
    materials,
    config: MeshConfig::default(),
    work_source: WorkSource::Refinement,
  };

  let result = mesh_node(input);

  assert_eq!(result.node, node);
}

#[test]
fn test_mesh_node_records_timing() {
  let (volume, materials) = make_sphere_volume(12.0);
  let input = MeshInput {
    node: OctreeNode::new(0, 0, 0, 0),
    volume,
    materials,
    config: MeshConfig::default(),
    work_source: WorkSource::Refinement,
  };

  let result = mesh_node(input);

  // Timing should be non-zero for non-trivial volume
  assert!(result.timing_us > 0, "Expected non-zero timing");
}

#[test]
fn test_mesh_node_preserves_refinement_work_source() {
  let (volume, materials) = make_sphere_volume(12.0);
  let input = MeshInput {
    node: OctreeNode::new(0, 0, 0, 0),
    volume,
    materials,
    config: MeshConfig::default(),
    work_source: WorkSource::Refinement,
  };

  let result = mesh_node(input);

  assert_eq!(result.work_source, WorkSource::Refinement);
}

#[test]
fn test_mesh_node_preserves_invalidation_work_source() {
  let (volume, materials) = make_sphere_volume(12.0);
  let input = MeshInput {
    node: OctreeNode::new(0, 0, 0, 0),
    volume,
    materials,
    config: MeshConfig::default(),
    work_source: WorkSource::Invalidation,
  };

  let result = mesh_node(input);

  assert_eq!(result.work_source, WorkSource::Invalidation);
}

// =============================================================================
// Batch 2: Empty/Degenerate Cases
// =============================================================================

#[test]
fn test_all_air_volume_produces_empty_mesh() {
  // All positive SDF = no surface
  let volume = Box::new([127i8; SAMPLE_SIZE_CB]);
  let materials = Box::new([0u8; SAMPLE_SIZE_CB]);
  let input = MeshInput {
    node: OctreeNode::new(0, 0, 0, 0),
    volume,
    materials,
    config: MeshConfig::default(),
    work_source: WorkSource::Refinement,
  };

  let result = mesh_node(input);

  assert!(
    result.output.is_empty(),
    "All-air volume should produce empty mesh"
  );
}

#[test]
fn test_all_solid_volume_produces_empty_mesh() {
  // All negative SDF = no surface
  let volume = Box::new([-127i8; SAMPLE_SIZE_CB]);
  let materials = Box::new([0u8; SAMPLE_SIZE_CB]);
  let input = MeshInput {
    node: OctreeNode::new(0, 0, 0, 0),
    volume,
    materials,
    config: MeshConfig::default(),
    work_source: WorkSource::Refinement,
  };

  let result = mesh_node(input);

  assert!(
    result.output.is_empty(),
    "All-solid volume should produce empty mesh"
  );
}

// =============================================================================
// Batch 3: Batch Processing
// =============================================================================

#[test]
fn test_mesh_batch_processes_multiple_nodes() {
  let inputs: Vec<MeshInput> = (0..4)
    .map(|i| {
      let (volume, materials) = make_sphere_volume(12.0);
      MeshInput {
        node: OctreeNode::new(i, 0, 0, 0),
        volume,
        materials,
        config: MeshConfig::default(),
        work_source: WorkSource::Refinement,
      }
    })
    .collect();

  let results = mesh_batch(inputs);

  assert_eq!(results.len(), 4);
  for (i, result) in results.iter().enumerate() {
    assert_eq!(result.node.x, i as i32);
    assert!(!result.output.is_empty());
  }
}

#[test]
fn test_mesh_batch_empty_input() {
  let inputs: Vec<MeshInput> = vec![];

  let results = mesh_batch(inputs);

  assert!(results.is_empty());
}

#[test]
fn test_mesh_batch_preserves_work_sources() {
  let inputs: Vec<MeshInput> = vec![
    {
      let (volume, materials) = make_sphere_volume(12.0);
      MeshInput {
        node: OctreeNode::new(0, 0, 0, 0),
        volume,
        materials,
        config: MeshConfig::default(),
        work_source: WorkSource::Refinement,
      }
    },
    {
      let (volume, materials) = make_sphere_volume(12.0);
      MeshInput {
        node: OctreeNode::new(1, 0, 0, 0),
        volume,
        materials,
        config: MeshConfig::default(),
        work_source: WorkSource::Invalidation,
      }
    },
  ];

  let results = mesh_batch(inputs);

  assert_eq!(results.len(), 2);
  // Results may be reordered, so check by node
  let ref_result = results.iter().find(|r| r.node.x == 0).unwrap();
  let inv_result = results.iter().find(|r| r.node.x == 1).unwrap();

  assert_eq!(ref_result.work_source, WorkSource::Refinement);
  assert_eq!(inv_result.work_source, WorkSource::Invalidation);
}
