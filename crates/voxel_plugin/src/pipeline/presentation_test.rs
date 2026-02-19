//! Tests for Stage 5: Presentation
//!
//! ```text
//! PresentationHint:
//!   - Immediate       → Invalidation: swap mesh instantly
//!   - FadeIn { key }  → Subdivide: fade in new children
//!   - FadeOut { key } → Merge: fade out children, keep parent
//! ```

use smallvec::smallvec;

use super::super::composition::CompositionOutput;
use super::{present, present_grouped, present_ungrouped};
use crate::octree::{OctreeNode, TransitionType};
use crate::pipeline::test_utils::*;
use crate::pipeline::types::{GroupedMesh, NodeMesh, PresentationHint, WorkSource};
use crate::world::WorldId;

/// Create a test WorldId for presentation tests.
fn test_world_id() -> WorldId {
  WorldId::new()
}

// =============================================================================
// Batch 1: PresentationHint from GroupedMesh
// =============================================================================

#[test]
fn test_subdivide_produces_fade_in_hints() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let children: Vec<OctreeNode> = (0..8u8)
    .filter_map(|octant| parent.get_child(octant))
    .collect();

  let grouped = GroupedMesh {
    group_key: parent,
    meshes: children
      .iter()
      .map(|&node| NodeMesh {
        node,
        output: make_sphere_mesh(),
        timing_us: 0,
      })
      .collect(),
    transition_type: TransitionType::Subdivide,
  };

  let chunks = present_grouped(test_world_id(), vec![grouped]);

  assert_eq!(chunks.len(), 8, "Should produce 8 ready chunks");
  for chunk in &chunks {
    match &chunk.hint {
      PresentationHint::FadeIn { group_key } => {
        assert_eq!(*group_key, parent);
      }
      other => panic!("Expected FadeIn, got {:?}", other),
    }
  }
}

#[test]
fn test_merge_produces_fade_out_hints() {
  let parent = OctreeNode::new(0, 0, 0, 3);

  let grouped = GroupedMesh {
    group_key: parent,
    meshes: smallvec![NodeMesh {
      node: parent,
      output: make_sphere_mesh(),
      timing_us: 0,
    }],
    transition_type: TransitionType::Merge,
  };

  let chunks = present_grouped(test_world_id(), vec![grouped]);

  assert_eq!(chunks.len(), 1, "Should produce 1 ready chunk");
  match &chunks[0].hint {
    PresentationHint::FadeOut { group_key } => {
      assert_eq!(*group_key, parent);
    }
    other => panic!("Expected FadeOut, got {:?}", other),
  }
}

#[test]
fn test_invalidation_produces_immediate_hints() {
  let node = OctreeNode::new(5, 3, 2, 2);
  let mesh_result = mock_mesh_result(node, WorkSource::Invalidation);

  let chunks = present_ungrouped(test_world_id(), vec![mesh_result]);

  assert_eq!(chunks.len(), 1);
  assert_eq!(chunks[0].hint, PresentationHint::Immediate);
  assert_eq!(chunks[0].node, node);
}

// =============================================================================
// Batch 2: MeshOutput Preservation
// =============================================================================

#[test]
fn test_mesh_output_preserved_in_ready_chunk() {
  let node = OctreeNode::new(0, 0, 0, 2);
  let mesh_result = mock_mesh_result(node, WorkSource::Invalidation);
  let vertex_count = mesh_result.output.vertices.len();
  let index_count = mesh_result.output.indices.len();
  let original_bounds = mesh_result.output.bounds;

  let chunks = present_ungrouped(test_world_id(), vec![mesh_result]);

  let output = &chunks[0].output;
  assert_eq!(output.vertices.len(), vertex_count);
  assert_eq!(output.indices.len(), index_count);
  assert_eq!(output.bounds.min, original_bounds.min);
  assert_eq!(output.bounds.max, original_bounds.max);
}

#[test]
fn test_empty_mesh_preserved() {
  use crate::types::MeshOutput;

  let node = OctreeNode::new(0, 0, 0, 2);
  let mesh_result = crate::pipeline::types::MeshResult {
    node,
    output: MeshOutput::new(), // Empty mesh
    timing_us: 0,
    work_source: WorkSource::Invalidation,
  };

  let chunks = present_ungrouped(test_world_id(), vec![mesh_result]);

  assert_eq!(chunks.len(), 1);
  assert!(chunks[0].output.vertices.is_empty());
  assert!(chunks[0].output.indices.is_empty());
}

// =============================================================================
// Batch 3: Full Present Function
// =============================================================================

#[test]
fn test_present_combines_grouped_and_ungrouped() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let children: Vec<OctreeNode> = (0..8u8)
    .filter_map(|octant| parent.get_child(octant))
    .collect();

  let grouped = GroupedMesh {
    group_key: parent,
    meshes: children
      .iter()
      .map(|&node| NodeMesh {
        node,
        output: make_sphere_mesh(),
        timing_us: 0,
      })
      .collect(),
    transition_type: TransitionType::Subdivide,
  };

  let invalidation_node = OctreeNode::new(10, 0, 0, 2);
  let ungrouped = mock_mesh_result(invalidation_node, WorkSource::Invalidation);

  let output = CompositionOutput {
    grouped: vec![grouped],
    ungrouped: vec![ungrouped],
  };

  let chunks = present(test_world_id(), output);

  assert_eq!(chunks.len(), 9, "Should have 8 grouped + 1 ungrouped");

  // Count by hint type
  let fade_in_count = chunks
    .iter()
    .filter(|c| matches!(c.hint, PresentationHint::FadeIn { .. }))
    .count();
  let immediate_count = chunks
    .iter()
    .filter(|c| matches!(c.hint, PresentationHint::Immediate))
    .count();

  assert_eq!(fade_in_count, 8);
  assert_eq!(immediate_count, 1);
}

#[test]
fn test_group_key_correct_in_all_chunks_of_group() {
  let parent = OctreeNode::new(7, 3, 5, 4);
  let children: Vec<OctreeNode> = (0..8u8)
    .filter_map(|octant| parent.get_child(octant))
    .collect();

  let grouped = GroupedMesh {
    group_key: parent,
    meshes: children
      .iter()
      .map(|&node| NodeMesh {
        node,
        output: make_sphere_mesh(),
        timing_us: 0,
      })
      .collect(),
    transition_type: TransitionType::Subdivide,
  };

  let chunks = present_grouped(test_world_id(), vec![grouped]);

  for chunk in &chunks {
    match &chunk.hint {
      PresentationHint::FadeIn { group_key } => {
        assert_eq!(
          *group_key, parent,
          "All children should reference same parent"
        );
      }
      _ => panic!("Expected FadeIn hint"),
    }
  }
}

// =============================================================================
// Batch 4: Edge Cases
// =============================================================================

#[test]
fn test_present_empty_composition() {
  let output = CompositionOutput {
    grouped: vec![],
    ungrouped: vec![],
  };

  let chunks = present(test_world_id(), output);

  assert!(chunks.is_empty());
}

#[test]
fn test_present_partial_group() {
  // Group with only 5 meshes (3 were skipped)
  let parent = OctreeNode::new(0, 0, 0, 3);
  let children: Vec<OctreeNode> = (0..5u8)
    .filter_map(|octant| parent.get_child(octant))
    .collect();

  let grouped = GroupedMesh {
    group_key: parent,
    meshes: children
      .iter()
      .map(|&node| NodeMesh {
        node,
        output: make_sphere_mesh(),
        timing_us: 0,
      })
      .collect(),
    transition_type: TransitionType::Subdivide,
  };

  let chunks = present_grouped(test_world_id(), vec![grouped]);

  assert_eq!(chunks.len(), 5, "Should produce 5 chunks for partial group");
}
