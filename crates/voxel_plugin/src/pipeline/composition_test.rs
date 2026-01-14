//! Tests for Stage 4: Composition
//!
//! ```text
//! Subdivide (1→8):              Merge (8→1):
//! ┌───────────────────┐         ┌───────────────────┐
//! │ Group contains:   │         │ Group contains:   │
//! │  - 8 child meshes │         │  - 1 parent mesh  │
//! │  - parent ref     │         │  - 8 child refs   │
//! │    (for fadeout)  │         │    (for fadeout)  │
//! └───────────────────┘         └───────────────────┘
//!
//! INVALIDATION work_source bypasses this stage entirely.
//! ```

use super::compose;
use crate::octree::{OctreeNode, TransitionGroup, TransitionType};
use crate::pipeline::test_utils::*;
use crate::pipeline::types::WorkSource;

// =============================================================================
// Batch 1: Subdivide Grouping (1 parent → 8 children)
// =============================================================================

#[test]
fn test_subdivide_groups_8_meshes_by_parent() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let transition = TransitionGroup::new_subdivide(parent).unwrap();
  let mesh_results = child_mesh_results(&parent);

  let output = compose(mesh_results, &[transition]);

  assert_eq!(output.grouped.len(), 1, "Should produce 1 group");
  assert_eq!(
    output.grouped[0].meshes.len(),
    8,
    "Group should have 8 meshes"
  );
}

#[test]
fn test_subdivide_group_key_is_parent() {
  let parent = OctreeNode::new(5, 3, 2, 4);
  let transition = TransitionGroup::new_subdivide(parent).unwrap();
  let mesh_results = child_mesh_results(&parent);

  let output = compose(mesh_results, &[transition]);

  assert_eq!(output.grouped[0].group_key, parent);
}

#[test]
fn test_subdivide_transition_type_preserved() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let transition = TransitionGroup::new_subdivide(parent).unwrap();
  let mesh_results = child_mesh_results(&parent);

  let output = compose(mesh_results, &[transition]);

  assert_eq!(output.grouped[0].transition_type, TransitionType::Subdivide);
}

// =============================================================================
// Batch 2: Merge Grouping (8 children → 1 parent)
// =============================================================================

#[test]
fn test_merge_groups_1_mesh() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let transition = merge_fixture(3);
  let mesh_result = mock_mesh_result(parent, WorkSource::Refinement);

  let output = compose(vec![mesh_result], &[transition]);

  assert_eq!(output.grouped.len(), 1, "Should produce 1 group");
  assert_eq!(
    output.grouped[0].meshes.len(),
    1,
    "Merge group should have 1 mesh"
  );
}

#[test]
fn test_merge_group_key_is_parent() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let transition = merge_fixture(3);
  let mesh_result = mock_mesh_result(parent, WorkSource::Refinement);

  let output = compose(vec![mesh_result], &[transition]);

  assert_eq!(output.grouped[0].group_key, parent);
}

#[test]
fn test_merge_transition_type_preserved() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let transition = merge_fixture(3);
  let mesh_result = mock_mesh_result(parent, WorkSource::Refinement);

  let output = compose(vec![mesh_result], &[transition]);

  assert_eq!(output.grouped[0].transition_type, TransitionType::Merge);
}

// =============================================================================
// Batch 3: Multiple Groups
// =============================================================================

#[test]
fn test_multiple_transition_groups_produce_multiple_grouped_meshes() {
  let parent1 = OctreeNode::new(0, 0, 0, 3);
  let parent2 = OctreeNode::new(1, 0, 0, 3);
  let parent3 = OctreeNode::new(0, 1, 0, 3);

  let transition1 = TransitionGroup::new_subdivide(parent1).unwrap();
  let transition2 = TransitionGroup::new_subdivide(parent2).unwrap();
  let transition3 = TransitionGroup::new_subdivide(parent3).unwrap();

  let mut mesh_results = Vec::new();
  mesh_results.extend(child_mesh_results(&parent1));
  mesh_results.extend(child_mesh_results(&parent2));
  mesh_results.extend(child_mesh_results(&parent3));

  let output = compose(mesh_results, &[transition1, transition2, transition3]);

  assert_eq!(output.grouped.len(), 3, "Should produce 3 groups");
}

// =============================================================================
// Batch 4: Invalidation Bypass
// =============================================================================

#[test]
fn test_invalidation_work_source_bypasses_composition() {
  let node = OctreeNode::new(0, 0, 0, 2);
  let mesh_result = mock_mesh_result(node, WorkSource::Invalidation);

  // No transition groups - invalidation doesn't need them
  let output = compose(vec![mesh_result], &[]);

  assert!(
    output.grouped.is_empty(),
    "Invalidation should not be grouped"
  );
  assert_eq!(
    output.ungrouped.len(),
    1,
    "Invalidation should be in ungrouped"
  );
  assert_eq!(output.ungrouped[0].node, node);
}

#[test]
fn test_mixed_work_sources_routed_correctly() {
  let parent = OctreeNode::new(0, 0, 0, 3);
  let transition = TransitionGroup::new_subdivide(parent).unwrap();

  // 8 refinement meshes (should be grouped)
  let mut mesh_results = child_mesh_results(&parent);

  // 2 invalidation meshes (should bypass)
  mesh_results.push(mock_mesh_result(
    OctreeNode::new(10, 0, 0, 2),
    WorkSource::Invalidation,
  ));
  mesh_results.push(mock_mesh_result(
    OctreeNode::new(11, 0, 0, 2),
    WorkSource::Invalidation,
  ));

  let output = compose(mesh_results, &[transition]);

  assert_eq!(output.grouped.len(), 1, "Should have 1 grouped mesh");
  assert_eq!(
    output.grouped[0].meshes.len(),
    8,
    "Group should have 8 children"
  );
  assert_eq!(
    output.ungrouped.len(),
    2,
    "Should have 2 ungrouped (invalidation)"
  );
}

// =============================================================================
// Batch 5: Edge Cases
// =============================================================================

#[test]
fn test_missing_mesh_for_group_member() {
  // TransitionGroup expects 8 children, but only 6 meshes provided
  // (e.g., 2 were skipped by prefilter)
  let parent = OctreeNode::new(0, 0, 0, 3);
  let transition = TransitionGroup::new_subdivide(parent).unwrap();

  // Only provide 6 of 8 child meshes
  let mut mesh_results = child_mesh_results(&parent);
  mesh_results.truncate(6);

  let output = compose(mesh_results, &[transition]);

  assert_eq!(output.grouped.len(), 1, "Should still produce 1 group");
  assert_eq!(
    output.grouped[0].meshes.len(),
    6,
    "Group should have 6 meshes (partial)"
  );
}

#[test]
fn test_composition_with_no_transitions() {
  // No transition groups, only invalidation meshes
  let mesh_results = vec![
    mock_mesh_result(OctreeNode::new(0, 0, 0, 2), WorkSource::Invalidation),
    mock_mesh_result(OctreeNode::new(1, 0, 0, 2), WorkSource::Invalidation),
  ];

  let output = compose(mesh_results, &[]);

  assert!(output.grouped.is_empty());
  assert_eq!(output.ungrouped.len(), 2);
}

#[test]
fn test_empty_inputs() {
  let output = compose(vec![], &[]);

  assert!(output.grouped.is_empty());
  assert!(output.ungrouped.is_empty());
}
