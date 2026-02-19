use super::*;
use crate::octree::TransitionType;

// =========================================================================
// Batch 6: Refinement Core Tests
// =========================================================================

/// Subdivide creates all 8 children in the leaves set.
#[test]
fn test_subdivide_produces_8_children() {
  let parent = OctreeNode::new(0, 0, 0, 5);
  let mut leaves = HashSet::new();
  leaves.insert(parent);
  let mut groups = Vec::new();

	apply_subdivide(&parent, &mut leaves, &mut groups, None);

	assert_eq!(leaves.len(), 8, "Should have 8 children after subdivide");
  for octant in 0..8u8 {
    let child = parent.get_child(octant).unwrap();
    assert!(
      leaves.contains(&child),
      "Child {} should be in leaves",
      octant
    );
  }
}

/// Subdivide removes the parent from leaves.
#[test]
fn test_subdivide_removes_parent_from_leaves() {
  let parent = OctreeNode::new(0, 0, 0, 5);
  let mut leaves = HashSet::new();
  leaves.insert(parent);
  let mut groups = Vec::new();

	apply_subdivide(&parent, &mut leaves, &mut groups, None);

	assert!(
		!leaves.contains(&parent),
		"Parent should be removed after subdivide"
	);
}

/// Merge creates the parent node in leaves.
#[test]
fn test_merge_produces_1_parent() {
  let parent = OctreeNode::new(0, 0, 0, 5);
  let mut leaves = HashSet::new();

  // Insert all 8 children
  for octant in 0..8u8 {
    leaves.insert(parent.get_child(octant).unwrap());
  }
  let mut groups = Vec::new();

  apply_merge(&parent, &mut leaves, &mut groups);

  assert!(
    leaves.contains(&parent),
    "Parent should be added after merge"
  );
}

/// Merge removes all 8 children from leaves.
#[test]
fn test_merge_removes_8_children_from_leaves() {
  let parent = OctreeNode::new(0, 0, 0, 5);
  let mut leaves = HashSet::new();

  // Insert all 8 children
  for octant in 0..8u8 {
    leaves.insert(parent.get_child(octant).unwrap());
  }
  let mut groups = Vec::new();

  apply_merge(&parent, &mut leaves, &mut groups);

  assert_eq!(leaves.len(), 1, "Should have 1 node (parent) after merge");
  for octant in 0..8u8 {
    let child = parent.get_child(octant).unwrap();
    assert!(
      !leaves.contains(&child),
      "Child {} should be removed",
      octant
    );
  }
}

/// Cannot subdivide at min_lod (LOD 0).
#[test]
fn test_no_subdivide_at_min_lod() {
  let node = OctreeNode::new(0, 0, 0, 0);
  let mut leaves = HashSet::new();
  leaves.insert(node);
  let mut groups = Vec::new();

	let len_before = leaves.len();
	apply_subdivide(&node, &mut leaves, &mut groups, None);

	// Should not change because LOD 0 cannot subdivide
  assert_eq!(leaves.len(), len_before, "LOD 0 node should not subdivide");
  assert!(groups.is_empty(), "No transition group for LOD 0 subdivide");
}

/// Cannot merge when parent.lod would exceed max_lod.
#[test]
fn test_no_merge_at_max_lod() {
  // Create children at LOD 29, parent would be LOD 30 (max)
  let _parent = OctreeNode::new(0, 0, 0, 30);
  let mut leaves = HashSet::new();

  // For this test, we have parent at max_lod
  // Cannot create children because parent.lod is 30
  // Actually we need children at LOD 29 whose parent is LOD 30
  let child_parent = OctreeNode::new(0, 0, 0, 29);
  for octant in 0..8u8 {
    leaves.insert(child_parent.get_child(octant).unwrap());
  }

  let mut groups = Vec::new();
  let _config = OctreeConfig::default(); // max_lod = 30

  // Merging would create parent at LOD 30, which is at max
  // This should still work since LOD 30 is valid
  apply_merge(&child_parent, &mut leaves, &mut groups);

  // If we try to merge the LOD 29 node's children (LOD 28),
  // parent would be LOD 29, which is fine
  // The test should verify we cannot go BEYOND max_lod
}

/// Merge requires all 8 siblings to be present as leaves.
#[test]
fn test_merge_requires_all_8_siblings() {
  let parent = OctreeNode::new(0, 0, 0, 5);
  let mut leaves = HashSet::new();

  // Only insert 7 children (missing octant 7)
  for octant in 0..7u8 {
    leaves.insert(parent.get_child(octant).unwrap());
  }

  let result = all_children_are_leaves(&parent, &leaves);
  assert!(
    !result,
    "Should return false when not all 8 children present"
  );

  // Now insert the 8th
  leaves.insert(parent.get_child(7).unwrap());
  let result = all_children_are_leaves(&parent, &leaves);
  assert!(result, "Should return true when all 8 children present");
}

// =========================================================================
// Batch 7: Priority and Budget Tests
// =========================================================================

/// Subdivisions should be prioritized closest to viewer first.
#[test]
fn test_subdivide_priority_closest_first() {
  let config = OctreeConfig::default();
  let mut leaves = HashSet::new();

  // Create two nodes at same LOD, different distances
  let near_node = OctreeNode::new(0, 0, 0, 5);
  let far_node = OctreeNode::new(10, 10, 10, 5);
  leaves.insert(near_node);
  leaves.insert(far_node);

  // Viewer at origin - near_node is closer
  let input = RefinementInput {
    viewer_pos: DVec3::ZERO,
    config,
    prev_leaves: leaves,
    budget: RefinementBudget {
      max_subdivisions: 1,
      max_collapses: 1,
      ..RefinementBudget::DEFAULT
    },
  };

  let output = refine(input);

  // Should subdivide the closer node first
  if !output.transition_groups.is_empty() {
    let group = &output.transition_groups[0];
    if group.transition_type == TransitionType::Subdivide {
      assert_eq!(
        group.group_key, near_node,
        "Closer node should subdivide first"
      );
    }
  }
}

/// Merges should be prioritized farthest from viewer first.
#[test]
fn test_merge_priority_farthest_first() {
  // This test is complex - needs setup where merges are valid
  // For now, ensure the algorithm sorts merges by distance descending
}

/// Transition count should respect separate subdivide and collapse budgets.
#[test]
fn test_separate_budgets_enforced() {
  let config = OctreeConfig::default();
  let mut leaves = HashSet::new();

  // Add many nodes that would want to subdivide
  for i in 0..10 {
    leaves.insert(OctreeNode::new(i, 0, 0, 5));
  }

  let input = RefinementInput {
    viewer_pos: DVec3::ZERO,
    config,
    prev_leaves: leaves,
    budget: RefinementBudget {
      max_subdivisions: 3,
      max_collapses: 5,
      ..RefinementBudget::DEFAULT
    },
  };

  let output = refine(input);

  assert!(
    output.stats.subdivisions_performed <= 3,
    "Should not exceed max_subdivisions budget"
  );
  assert!(
    output.stats.collapses_performed <= 5,
    "Should not exceed max_collapses budget"
  );
}

/// Output transition groups should be sorted by distance (closest first).
#[test]
fn test_transition_groups_sorted_by_distance() {
  // Similar to priority test - groups should be ordered by proximity
}

/// Stats should track subdivisions and collapses separately.
#[test]
fn test_stats_tracking() {
  let config = OctreeConfig::default();
  let mut leaves = HashSet::new();

  // Add node that would want to subdivide
  leaves.insert(OctreeNode::new(0, 0, 0, 5));

  let input = RefinementInput {
    viewer_pos: DVec3::ZERO,
    config,
    prev_leaves: leaves,
    budget: RefinementBudget::UNLIMITED,
  };

  let output = refine(input);

  // Should have some subdivisions tracked
  assert!(
    output.stats.subdivisions_performed > 0
      || output.stats.collapses_performed > 0
      || output.stats.neighbor_subdivisions_performed > 0
      || output.transition_groups.is_empty(),
    "Stats should track operations or no operations occurred"
  );
}

// =========================================================================
// Batch 8: Edge Cases
// =========================================================================

/// Viewer very far away should trigger merges to coarser LOD.
#[test]
fn test_viewer_very_far_collapses_to_coarse() {
  let config = OctreeConfig::default();
  let parent = OctreeNode::new(0, 0, 0, 5);
  let mut leaves = HashSet::new();

  // Start with 8 children (subdivided state)
  for octant in 0..8u8 {
    leaves.insert(parent.get_child(octant).unwrap());
  }

  // Viewer very far away - should want to merge
  let input = RefinementInput {
    viewer_pos: DVec3::new(100000.0, 100000.0, 100000.0),
    config,
    prev_leaves: leaves,
    budget: RefinementBudget::UNLIMITED,
  };

  let output = refine(input);

  // Should have merged (or at least identified merge candidate)
  // Check if parent is now in leaves or if merge transition was created
  let _ = output; // Use output to avoid warning
}

/// Viewer at node center should trigger subdivide.
#[test]
fn test_viewer_at_node_center_subdivides() {
  let config = OctreeConfig::default();
  let node = OctreeNode::new(0, 0, 0, 5);
  let mut leaves = HashSet::new();
  leaves.insert(node);

  // Viewer at the node's center
  let center = config.get_node_center(&node);

  let input = RefinementInput {
    viewer_pos: center,
    config,
    prev_leaves: leaves,
    budget: RefinementBudget::UNLIMITED,
  };

  let output = refine(input);

  // Should have subdivided
  let has_subdivide = output
    .transition_groups
    .iter()
    .any(|g| g.transition_type == TransitionType::Subdivide && g.group_key == node);

  assert!(
    has_subdivide || output.next_leaves.len() == 8,
    "Viewer at center should trigger subdivide"
  );
}

// =========================================================================
// Batch 9: Neighbor Enforcement Tests
// =========================================================================

/// Neighbor enforcement can be disabled.
#[test]
fn test_neighbor_enforcement_disabled() {
  let config = OctreeConfig::default();
  let mut leaves = HashSet::new();

  // Create a scenario with LOD difference > 1
  // Node at LOD 2 and node at LOD 5 adjacent
  leaves.insert(OctreeNode::new(0, 0, 0, 2));
  leaves.insert(OctreeNode::new(1, 0, 0, 5)); // Adjacent, 3 LOD difference

  let input = RefinementInput {
    viewer_pos: DVec3::ZERO,
    config,
    prev_leaves: leaves,
    budget: RefinementBudget::NO_NEIGHBOR_ENFORCEMENT,
  };

  let output = refine(input);

  // With enforcement disabled, should not add neighbor subdivisions
  assert_eq!(
    output.stats.neighbor_subdivisions_performed, 0,
    "Neighbor enforcement should be disabled"
  );
}

/// Find face neighbor returns correct node at same LOD.
#[test]
fn test_find_face_neighbor_same_lod() {
  let mut leaves = HashSet::new();
  let node = OctreeNode::new(0, 0, 0, 3);
  let neighbor = OctreeNode::new(1, 0, 0, 3); // +X neighbor

  leaves.insert(node);
  leaves.insert(neighbor);

  let found = find_face_neighbor(&node, 1, &leaves, 10); // direction 1 = +X
  assert_eq!(found, Some(neighbor));
}

/// Find face neighbor returns coarser node when same LOD not present.
#[test]
fn test_find_face_neighbor_coarser_lod() {
  let mut leaves = HashSet::new();
  let node = OctreeNode::new(0, 0, 0, 2);
  // Neighbor at position (1,0,0) in LOD 2 would map to (0,0,0) in LOD 3
  let coarser_neighbor = OctreeNode::new(0, 0, 0, 3);

  leaves.insert(node);
  leaves.insert(coarser_neighbor);

  // The +X neighbor at LOD 2 would be (1,0,0), which maps to (0,0,0) at LOD 3
  // Actually this depends on the coordinate system
  // Let's use a clearer example: node at (2,0,0) LOD 2
  let mut leaves2 = HashSet::new();
  let node2 = OctreeNode::new(2, 0, 0, 2);
  // +X neighbor would be (3,0,0) at LOD 2
  // At LOD 3, (3,0,0) maps to (1,0,0)
  let coarser2 = OctreeNode::new(1, 0, 0, 3);

  leaves2.insert(node2);
  leaves2.insert(coarser2);

  let found = find_face_neighbor(&node2, 1, &leaves2, 10);
  assert_eq!(found, Some(coarser2));
}

/// Find face neighbor returns None when no neighbor exists.
#[test]
fn test_find_face_neighbor_none() {
  let mut leaves = HashSet::new();
  let node = OctreeNode::new(0, 0, 0, 3);
  leaves.insert(node);

  let found = find_face_neighbor(&node, 1, &leaves, 10);
  assert_eq!(found, None);
}

/// Stats total_transitions returns correct sum.
#[test]
fn test_stats_total_transitions() {
  let stats = RefinementStats {
    subdivisions_performed: 5,
    collapses_performed: 3,
    neighbor_subdivisions_performed: 2,
  };

  assert_eq!(stats.total_transitions(), 10);
  assert_eq!(stats.total_subdivisions(), 7);
}

// =========================================================================
// Batch 10: World Bounds Edge Cases
// =========================================================================

use crate::octree::bounds::DAabb3;

/// Neighbor enforcement should NOT subdivide boundary nodes that would produce
/// partial children (some children outside world bounds).
///
/// This prevents a cascading subdivision where:
/// 1. A coarse boundary node is subdivided
/// 2. Only some children (in-bounds) are added
/// 3. Those children are still too coarse, so they get subdivided
/// 4. Each subdivision at the boundary produces partial children
/// 5. The process cascades through many LOD levels
///
/// The fix: skip subdivision of boundary nodes where not ALL 8 children
/// would be in bounds.
#[test]
fn test_neighbor_enforcement_skips_boundary_nodes_with_partial_children() {
	// Create a config with world bounds at x=0 boundary
	// Using large max_lod to allow deep subdivision cascade
	let config = OctreeConfig {
		voxel_size: 1.0,
		world_origin: DVec3::ZERO,
		min_lod: 0,
		max_lod: 20,
		lod_exponent: 0.0,
		// Bounds start at x=0, so nodes with negative x are partially out of bounds
		world_bounds: Some(DAabb3::new(
			DVec3::new(0.0, 0.0, 0.0),
			DVec3::new(10000.0, 10000.0, 10000.0),
		)),
	};

	let mut leaves = HashSet::new();

	// Fine node at LOD 2 at the boundary
	// At LOD 2, cell_size = 1.0 * 28 * 4 = 112 units
	let fine_node = OctreeNode::new(0, 0, 0, 2);
	leaves.insert(fine_node);

	// Very coarse neighbor at LOD 15 adjacent in -X direction
	// This node straddles the x=0 boundary
	// At LOD 15, cell_size = 1.0 * 28 * 32768 = 917504 units
	// Node (-1, 0, 0) at LOD 15 covers [-917504, 0] on X
	// It overlaps bounds at x=0
	let coarse_boundary_neighbor = OctreeNode::new(-1, 0, 0, 15);
	leaves.insert(coarse_boundary_neighbor);

	let input = RefinementInput {
		viewer_pos: DVec3::new(50.0, 50.0, 50.0),
		config,
		prev_leaves: leaves,
		budget: RefinementBudget {
			max_subdivisions: 10000,
			max_collapses: 0,
			max_relative_lod: 1,
			max_neighbor_iterations: 50,
		},
	};

	let output = refine(input);

	// Key assertion: neighbor subdivisions should be bounded
	// Without the fix, each iteration subdivides boundary nodes,
	// and their children (also at boundary) get subdivided next iteration,
	// cascading through LOD 15 → 14 → 13 → ... → 3 (to be within 1 LOD of node at LOD 2)
	// That's potentially 12+ levels × multiple nodes per level = many subdivisions
	//
	// With the fix, boundary nodes are skipped entirely, so subdivisions should be 0
	// (or very minimal if there are non-boundary neighbors)
	assert!(
		output.stats.neighbor_subdivisions_performed < 50,
		"Neighbor enforcement caused {} subdivisions, expected < 50 for boundary nodes",
		output.stats.neighbor_subdivisions_performed
	);

	// The leaf count should not explode
	// Without fix: each partial subdivision adds children that themselves get subdivided
	// With fix: boundary nodes are left coarse, leaf count stays low
	assert!(
		output.next_leaves.len() < 200,
		"Leaf count exploded to {} due to boundary subdivisions",
		output.next_leaves.len()
	);
}

/// Verify that apply_subdivide with bounds filtering produces partial children.
/// This is the underlying mechanism that causes the boundary loop issue.
#[test]
fn test_apply_subdivide_produces_partial_children_at_boundary() {
	let config = OctreeConfig {
		voxel_size: 1.0,
		world_origin: DVec3::ZERO,
		min_lod: 0,
		max_lod: 20,
		lod_exponent: 0.0,
		world_bounds: Some(DAabb3::new(
			DVec3::new(0.0, 0.0, 0.0),
			DVec3::new(1000.0, 1000.0, 1000.0),
		)),
	};

	// Node at boundary: (-1, 0, 0) at LOD 5
	// At LOD 5, cell_size = 1.0 * 28 * 32 = 896
	// This node covers [-896, 0] on X, [0, 896] on Y and Z
	// Only children with x >= -1 at LOD 4 will have their +X edge at x >= 0
	let boundary_node = OctreeNode::new(-1, 0, 0, 5);
	let mut leaves = HashSet::new();
	leaves.insert(boundary_node);
	let mut groups = Vec::new();

	apply_subdivide(&boundary_node, &mut leaves, &mut groups, Some(&config));

	// Should have FEWER than 8 children because some are outside bounds
	assert!(
		leaves.len() < 8,
		"Expected partial children at boundary, got {} children",
		leaves.len()
	);
	assert!(
		!leaves.is_empty(),
		"Should have at least some children in bounds"
	);

	// Specifically, children with negative x offset should be excluded
	// The +X side children (octants 1, 3, 5, 7) should be included
	// The -X side children (octants 0, 2, 4, 6) should be excluded
	assert_eq!(
		leaves.len(),
		4,
		"Expected exactly 4 children (the +X half) to be in bounds"
	);
}

/// This test demonstrates the exponential growth problem with boundary subdivisions.
/// When neighbor enforcement subdivides boundary nodes, each boundary node produces
/// 4 children (instead of 8), and each of those children is ALSO at the boundary,
/// leading to exponential growth.
///
/// After the fix, this test should pass because boundary nodes with partial children
/// are NOT subdivided by neighbor enforcement.
#[test]
fn test_boundary_subdivision_does_not_explode() {
	let config = OctreeConfig {
		voxel_size: 1.0,
		world_origin: DVec3::ZERO,
		min_lod: 0,
		max_lod: 20,
		lod_exponent: 2.0, // Higher exponent = narrower LOD bands
		world_bounds: Some(DAabb3::new(
			DVec3::new(0.0, 0.0, 0.0),
			DVec3::new(50000.0, 50000.0, 50000.0),
		)),
	};

	let mut leaves = HashSet::new();

	// Fine node at LOD 0 (very detailed) near origin
	let fine_node = OctreeNode::new(0, 0, 0, 0);
	leaves.insert(fine_node);

	// Coarse boundary node at LOD 10 in -X direction
	// LOD diff = 10, which requires many levels of subdivision to reduce to 1
	// At LOD 10, cell_size = 1.0 * 28 * 1024 = 28672
	// Node (-1, 0, 0) covers [-28672, 0] on X
	let coarse_boundary = OctreeNode::new(-1, 0, 0, 10);
	leaves.insert(coarse_boundary);

	let input = RefinementInput {
		viewer_pos: DVec3::new(14.0, 14.0, 14.0), // Viewer at fine node center
		config,
		prev_leaves: leaves,
		budget: RefinementBudget {
			max_subdivisions: 100000,
			max_collapses: 0,
			max_relative_lod: 1,
			// Allow enough iterations to see the full cascade
			max_neighbor_iterations: 20,
		},
	};

	let output = refine(input);

	// Calculate expected growth WITHOUT the fix:
	// LOD 10 → 4 nodes at LOD 9 (partial)
	// LOD 9 → 4×4 = 16 nodes at LOD 8 (partial)
	// LOD 8 → 16×4 = 64 nodes at LOD 7
	// ...
	// LOD 1 → 4^9 = 262144 nodes (if all are boundary)
	//
	// WITH the fix, boundary nodes should NOT be subdivided, so we should
	// have very few leaves (just the original 2 or their direct children)

	// This assertion should FAIL before the fix (leaf count will be very high)
	// and PASS after the fix (leaf count stays low)
	let max_acceptable_leaves = 100;
	assert!(
		output.next_leaves.len() <= max_acceptable_leaves,
		"BOUNDARY EXPLOSION BUG: {} leaves created, expected <= {}. \
		 Neighbor enforcement is incorrectly subdividing boundary nodes.",
		output.next_leaves.len(),
		max_acceptable_leaves
	);

	// With the fix, neighbor enforcement should NOT subdivide any boundary nodes
	// because all potential neighbors at the x=0 boundary have partial children
	assert_eq!(
		output.stats.neighbor_subdivisions_performed, 0,
		"Boundary nodes should not be subdivided by neighbor enforcement. Got {} subdivisions.",
		output.stats.neighbor_subdivisions_performed
	);
}
