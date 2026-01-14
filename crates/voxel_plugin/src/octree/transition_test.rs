use super::*;

// =========================================================================
// Batch 5: TransitionGroup Tests
// =========================================================================

/// Group key should always be the parent node.
#[test]
fn test_transition_group_key_is_parent() {
  let parent = OctreeNode::new(1, 2, 3, 5);

  let group = TransitionGroup::new_subdivide(parent).expect("Should create subdivide group");

  assert_eq!(group.group_key, parent, "Group key should be parent");
}

/// Subdivide: 8 nodes to add (children), 1 to remove (parent).
#[test]
fn test_subdivide_group_invariants() {
  let parent = OctreeNode::new(1, 2, 3, 5);

  let group = TransitionGroup::new_subdivide(parent).expect("Should create subdivide group");

  assert_eq!(group.transition_type, TransitionType::Subdivide);
  assert_eq!(group.nodes_to_add.len(), 8, "Subdivide adds 8 children");
  assert_eq!(group.nodes_to_remove.len(), 1, "Subdivide removes 1 parent");
  assert_eq!(group.nodes_to_remove[0], parent);
}

/// Merge: 1 node to add (parent), 8 to remove (children).
#[test]
fn test_merge_group_invariants() {
  let parent = OctreeNode::new(1, 2, 3, 5);

  // Generate all 8 children
  let children: SmallVec<[OctreeNode; 8]> = (0..8u8)
    .map(|octant| parent.get_child(octant).unwrap())
    .collect();

  let group =
    TransitionGroup::new_merge(parent, children.clone()).expect("Should create merge group");

  assert_eq!(group.transition_type, TransitionType::Merge);
  assert_eq!(group.nodes_to_add.len(), 1, "Merge adds 1 parent");
  assert_eq!(group.nodes_to_remove.len(), 8, "Merge removes 8 children");
  assert_eq!(group.nodes_to_add[0], parent);
}
