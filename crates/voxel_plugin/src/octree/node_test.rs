use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::*;

// =========================================================================
// Batch 2: OctreeNode Tests
// =========================================================================

/// Two nodes with same x, y, z, lod should be equal.
#[test]
fn test_node_equality() {
  let node1 = OctreeNode::new(1, 2, 3, 5);
  let node2 = OctreeNode::new(1, 2, 3, 5);
  let node3 = OctreeNode::new(1, 2, 3, 6); // different lod

  assert_eq!(node1, node2);
  assert_ne!(node1, node3);
}

/// Equal nodes must produce equal hashes (HashMap invariant).
#[test]
fn test_node_hash_consistency() {
  let node1 = OctreeNode::new(10, 20, 30, 4);
  let node2 = OctreeNode::new(10, 20, 30, 4);

  let hash1 = {
    let mut hasher = DefaultHasher::new();
    node1.hash(&mut hasher);
    hasher.finish()
  };
  let hash2 = {
    let mut hasher = DefaultHasher::new();
    node2.hash(&mut hasher);
    hasher.finish()
  };

  assert_eq!(hash1, hash2, "Equal nodes must have equal hashes");
}

/// Child node should have LOD - 1 (finer detail).
#[test]
fn test_get_child_returns_finer_lod() {
  let parent = OctreeNode::new(0, 0, 0, 5);
  let child = parent.get_child(0).expect("Should return child at LOD > 0");

  assert_eq!(
    child.lod,
    parent.lod - 1,
    "Child LOD should be parent LOD - 1"
  );
}

/// All 8 octants (0-7) should produce valid children with correct coordinates.
///
/// Octant bits: X (bit 0), Y (bit 1), Z (bit 2)
/// child.x = parent.x * 2 + (octant & 1)
/// child.y = parent.y * 2 + ((octant >> 1) & 1)
/// child.z = parent.z * 2 + ((octant >> 2) & 1)
#[test]
fn test_get_child_all_8_octants() {
  let parent = OctreeNode::new(3, 4, 5, 10);

  for octant in 0u8..8 {
    let child = parent
      .get_child(octant)
      .unwrap_or_else(|| panic!("Octant {} should return a child", octant));

    let expected_x = parent.x * 2 + (octant & 1) as i32;
    let expected_y = parent.y * 2 + ((octant >> 1) & 1) as i32;
    let expected_z = parent.z * 2 + ((octant >> 2) & 1) as i32;

    assert_eq!(child.x, expected_x, "Octant {} X mismatch", octant);
    assert_eq!(child.y, expected_y, "Octant {} Y mismatch", octant);
    assert_eq!(child.z, expected_z, "Octant {} Z mismatch", octant);
    assert_eq!(child.lod, parent.lod - 1, "Octant {} LOD mismatch", octant);
  }
}

/// Cannot subdivide at LOD 0 (finest level).
#[test]
fn test_get_child_at_lod_0_returns_none() {
  let node = OctreeNode::new(100, 200, 300, 0);

  for octant in 0u8..8 {
    assert!(
      node.get_child(octant).is_none(),
      "LOD 0 node should not produce children for octant {}",
      octant
    );
  }
}

/// Parent node should have LOD + 1 (coarser).
#[test]
fn test_get_parent_returns_coarser_lod() {
  let child = OctreeNode::new(10, 20, 30, 5);
  let max_lod = 30;
  let parent = child
    .get_parent(max_lod)
    .expect("Should return parent when LOD < max");

  assert_eq!(
    parent.lod,
    child.lod + 1,
    "Parent LOD should be child LOD + 1"
  );
  assert_eq!(parent.x, child.x / 2, "Parent X should be child X / 2");
  assert_eq!(parent.y, child.y / 2, "Parent Y should be child Y / 2");
  assert_eq!(parent.z, child.z / 2, "Parent Z should be child Z / 2");
}

/// Cannot go coarser than max_lod.
#[test]
fn test_get_parent_at_max_lod_returns_none() {
  let max_lod = 15;
  let node = OctreeNode::new(0, 0, 0, max_lod);

  assert!(
    node.get_parent(max_lod).is_none(),
    "Node at max_lod should not have a parent"
  );
}

/// parent(child(node, octant)) should equal node for any octant.
///
/// This verifies the coordinate math is consistent: subdividing and then
/// merging back should return to the original node.
#[test]
fn test_child_parent_roundtrip() {
  let original = OctreeNode::new(7, 8, 9, 10);
  let max_lod = 30;

  for octant in 0u8..8 {
    let child = original
      .get_child(octant)
      .expect("Should get child at LOD 10");
    let back_to_parent = child
      .get_parent(max_lod)
      .expect("Should get parent of child");

    assert_eq!(
      back_to_parent, original,
      "Roundtrip failed for octant {}: {:?} -> {:?} -> {:?}",
      octant, original, child, back_to_parent
    );
  }
}
