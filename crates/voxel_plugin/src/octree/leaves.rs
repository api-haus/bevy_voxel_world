//! OctreeLeaves - implicit octree represented as a set of leaf nodes.
//!
//! The tree structure is implicit: parent/child relationships are computed
//! on-demand via coordinate math. Only leaves are stored.

use std::collections::HashSet;

use super::OctreeNode;

/// Implicit octree - leaves ARE the state.
///
/// No explicit tree structure. Parent/child relationships computed on demand.
pub struct OctreeLeaves {
  leaves: HashSet<OctreeNode>,
}

impl OctreeLeaves {
  /// Create empty leaves set.
  pub fn new() -> Self {
    Self {
      leaves: HashSet::new(),
    }
  }

  /// Initialize with a single node at given LOD.
  pub fn new_with_initial(lod: i32) -> Self {
    let mut leaves = HashSet::new();
    leaves.insert(OctreeNode::new(0, 0, 0, lod));
    Self { leaves }
  }

  /// Number of leaves.
  pub fn len(&self) -> usize {
    self.leaves.len()
  }

  /// Check if empty.
  pub fn is_empty(&self) -> bool {
    self.leaves.is_empty()
  }

  /// Check if a node is a leaf.
  pub fn contains(&self, node: &OctreeNode) -> bool {
    self.leaves.contains(node)
  }

  /// Insert a leaf node.
  pub fn insert(&mut self, node: OctreeNode) -> bool {
    self.leaves.insert(node)
  }

  /// Remove a leaf node.
  pub fn remove(&mut self, node: &OctreeNode) -> bool {
    self.leaves.remove(node)
  }

  /// Iterate over leaves.
  pub fn iter(&self) -> impl Iterator<Item = &OctreeNode> {
    self.leaves.iter()
  }

  /// Find the effective max LOD (coarsest node in leaves).
  pub fn effective_max_lod(&self) -> i32 {
    self.leaves.iter().map(|n| n.lod).max().unwrap_or(0)
  }

  /// Get inner set (for cloning in refinement).
  pub fn as_set(&self) -> &HashSet<OctreeNode> {
    &self.leaves
  }
}

impl Default for OctreeLeaves {
  fn default() -> Self {
    Self::new()
  }
}

impl From<HashSet<OctreeNode>> for OctreeLeaves {
  fn from(leaves: HashSet<OctreeNode>) -> Self {
    Self { leaves }
  }
}

#[cfg(test)]
#[path = "leaves_test.rs"]
mod leaves_test;
