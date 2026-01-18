//! TransitionGroup - atomic octree state changes.
//!
//! A transition group represents either a subdivide (1 parent → 8 children)
//! or merge (8 children → 1 parent) operation.

use smallvec::SmallVec;

use super::OctreeNode;

/// Type of octree transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransitionType {
  /// 1 parent → 8 children (finer detail)
  Subdivide,
  /// 8 children → 1 parent (coarser detail)
  Merge,
}

/// Atomic octree state change.
///
/// All nodes in a group transition together. Group key is always the parent
/// node.
#[derive(Clone)]
pub struct TransitionGroup {
  /// Type of transition.
  pub transition_type: TransitionType,

  /// Key: the parent node (for both subdivide and merge).
  pub group_key: OctreeNode,

  /// Nodes to add to octree leaves.
  /// - Subdivide: 8 children
  /// - Merge: 1 parent
  pub nodes_to_add: SmallVec<[OctreeNode; 8]>,

  /// Nodes to remove from octree leaves.
  /// - Subdivide: 1 parent
  /// - Merge: 8 children
  pub nodes_to_remove: SmallVec<[OctreeNode; 8]>,
}

impl TransitionGroup {
	/// Create a subdivide transition: parent → 8 children.
	pub fn new_subdivide(parent: OctreeNode) -> Option<Self> {
		// Cannot subdivide at LOD 0
		if parent.lod <= 0 {
			return None;
		}

		// Generate all 8 children
		let nodes_to_add: SmallVec<[OctreeNode; 8]> = (0..8u8)
			.filter_map(|octant| parent.get_child(octant))
			.collect();

		// Should have exactly 8 children
		if nodes_to_add.len() != 8 {
			return None;
		}

		let mut nodes_to_remove = SmallVec::new();
		nodes_to_remove.push(parent);

		Some(Self {
			transition_type: TransitionType::Subdivide,
			group_key: parent,
			nodes_to_add,
			nodes_to_remove,
		})
	}

	/// Create a subdivide transition with pre-filtered children.
	///
	/// Used when world bounds filtering reduces the child count below 8.
	pub fn new_subdivide_filtered(
		parent: OctreeNode,
		children: SmallVec<[OctreeNode; 8]>,
	) -> Option<Self> {
		// Cannot subdivide at LOD 0
		if parent.lod <= 0 {
			return None;
		}

		// Must have at least 1 child
		if children.is_empty() {
			return None;
		}

		let mut nodes_to_remove = SmallVec::new();
		nodes_to_remove.push(parent);

		Some(Self {
			transition_type: TransitionType::Subdivide,
			group_key: parent,
			nodes_to_add: children,
			nodes_to_remove,
		})
	}

  /// Create a merge transition: 8 children → parent.
  ///
  /// `children` must contain exactly 8 nodes that are siblings.
  pub fn new_merge(parent: OctreeNode, children: SmallVec<[OctreeNode; 8]>) -> Option<Self> {
    // Must have exactly 8 children
    if children.len() != 8 {
      return None;
    }

    let mut nodes_to_add = SmallVec::new();
    nodes_to_add.push(parent);

    Some(Self {
      transition_type: TransitionType::Merge,
      group_key: parent,
      nodes_to_add,
      nodes_to_remove: children,
    })
  }
}

#[cfg(test)]
#[path = "transition_test.rs"]
mod transition_test;
