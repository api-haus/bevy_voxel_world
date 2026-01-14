use super::*;

// =========================================================================
// Batch 4: OctreeLeaves Tests
// =========================================================================

/// Empty leaves should have no transitions possible.
#[test]
fn test_empty_leaves_no_transitions() {
  let leaves = OctreeLeaves::new();
  assert!(leaves.is_empty());
  assert_eq!(leaves.len(), 0);
}

/// Single leaf at min_lod (0) cannot subdivide further.
#[test]
fn test_single_leaf_at_min_lod_no_subdivide() {
  let leaves = OctreeLeaves::new_with_initial(0);
  assert_eq!(leaves.len(), 1);

  // The single leaf is at LOD 0, get_child should return None
  let leaf = leaves.iter().next().unwrap();
  assert_eq!(leaf.lod, 0);
  assert!(leaf.get_child(0).is_none(), "LOD 0 node cannot subdivide");
}
