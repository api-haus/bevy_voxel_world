//! Refinement algorithm for LOD-based octree updates.
//!
//! Determines which nodes to subdivide (closer to viewer) or merge (farther
//! away) based on distance thresholds derived from OctreeConfig.
//!
//! # Scheduling Strategy
//!
//! The algorithm uses a **collapse-first** strategy:
//! 1. Collapses are processed FIRST (farthest nodes first) - "load shedding"
//! 2. Subdivisions are processed SECOND (closest nodes first) - "add detail"
//!
//! This prioritizes shedding distant load before adding nearby detail,
//! which maintains responsiveness during rapid camera movement.
//!
//! # Neighbor Enforcement
//!
//! To prevent T-junction artifacts at LOD boundaries, the algorithm enforces
//! a maximum LOD difference between adjacent nodes. By default, adjacent nodes
//! can differ by at most 1 LOD level.

use std::collections::HashSet;

use glam::DVec3;

use super::budget::{RefinementBudget, RefinementStats};
use super::{OctreeConfig, OctreeNode, TransitionGroup};

/// Input for refinement calculation.
pub struct RefinementInput {
  /// Viewer position in world space (double precision for huge worlds).
  pub viewer_pos: DVec3,
  /// Octree configuration with LOD thresholds.
  pub config: OctreeConfig,
  /// Current set of leaf nodes.
  pub prev_leaves: HashSet<OctreeNode>,
  /// Budget configuration for rate limiting.
  pub budget: RefinementBudget,
}

/// Output from refinement calculation.
pub struct RefinementOutput {
  /// Updated set of leaf nodes.
  pub next_leaves: HashSet<OctreeNode>,
  /// Transition groups to apply (sorted by distance).
  pub transition_groups: Vec<TransitionGroup>,
  /// Statistics from refinement execution.
  pub stats: RefinementStats,
}

/// Direction offsets for 6 face neighbors.
const FACE_OFFSETS: [(i32, i32, i32); 6] = [
  (-1, 0, 0), // -X
  (1, 0, 0),  // +X
  (0, -1, 0), // -Y
  (0, 1, 0),  // +Y
  (0, 0, -1), // -Z
  (0, 0, 1),  // +Z
];

/// Check if all 8 children of a parent are present in the leaves set.
pub fn all_children_are_leaves(parent: &OctreeNode, leaves: &HashSet<OctreeNode>) -> bool {
  (0..8u8).all(|octant| {
    parent
      .get_child(octant)
      .map(|child| leaves.contains(&child))
      .unwrap_or(false)
  })
}

/// Apply a subdivide operation: remove parent, add children.
///
/// When world bounds are set, only adds children that overlap the bounds.
pub fn apply_subdivide(
	parent: &OctreeNode,
	leaves: &mut HashSet<OctreeNode>,
	groups: &mut Vec<TransitionGroup>,
	config: Option<&OctreeConfig>,
) {
	// Cannot subdivide at LOD 0
	if parent.lod <= 0 {
		return;
	}

	// Collect children, optionally filtering by world bounds
	let children: smallvec::SmallVec<[OctreeNode; 8]> = (0..8u8)
		.filter_map(|octant| parent.get_child(octant))
		.filter(|child| {
			// If config provided, filter by bounds; otherwise include all
			config.map_or(true, |c| c.node_overlaps_bounds(child))
		})
		.collect();

	if children.is_empty() {
		return;
	}

	// Create transition group with filtered children
	if let Some(group) = TransitionGroup::new_subdivide_filtered(*parent, children) {
		// Remove parent from leaves
		leaves.remove(parent);

		// Add filtered children to leaves
		for child in &group.nodes_to_add {
			leaves.insert(*child);
		}

		groups.push(group);
	}
}

/// Apply a merge operation: remove 8 children, add parent.
pub fn apply_merge(
  parent: &OctreeNode,
  leaves: &mut HashSet<OctreeNode>,
  groups: &mut Vec<TransitionGroup>,
) {
  // Collect children
  let children: smallvec::SmallVec<[OctreeNode; 8]> = (0..8u8)
    .filter_map(|octant| parent.get_child(octant))
    .collect();

  if children.len() != 8 {
    return;
  }

  // Create transition group
  if let Some(group) = TransitionGroup::new_merge(*parent, children) {
    // Remove all children from leaves
    for child in &group.nodes_to_remove {
      leaves.remove(child);
    }

    // Add parent to leaves
    leaves.insert(*parent);

    groups.push(group);
  }
}

/// Find the face neighbor of a node in the given direction.
///
/// Returns the neighboring node at the same or coarser LOD level.
/// First checks same LOD, then progressively coarser LODs up to max_lod.
fn find_face_neighbor(
  node: &OctreeNode,
  direction: usize,
  leaves: &HashSet<OctreeNode>,
  max_lod: i32,
) -> Option<OctreeNode> {
  let (dx, dy, dz) = FACE_OFFSETS[direction];
  let neighbor_pos = (node.x + dx, node.y + dy, node.z + dz);

  // Check same LOD first
  let same_level = OctreeNode::new(neighbor_pos.0, neighbor_pos.1, neighbor_pos.2, node.lod);
  if leaves.contains(&same_level) {
    return Some(same_level);
  }

  // Check coarser LODs (parent, grandparent, etc.)
  for lod in (node.lod + 1)..=max_lod {
    let scale = 1 << (lod - node.lod);
    let coarser_pos = (
      neighbor_pos.0.div_euclid(scale),
      neighbor_pos.1.div_euclid(scale),
      neighbor_pos.2.div_euclid(scale),
    );
    let coarser = OctreeNode::new(coarser_pos.0, coarser_pos.1, coarser_pos.2, lod);
    if leaves.contains(&coarser) {
      return Some(coarser);
    }
  }

  None
}

/// Enforce neighbor LOD gradation (Phase 6).
///
/// Ensures adjacent nodes don't differ by more than `max_relative_lod` levels.
/// This prevents T-junction artifacts at LOD boundaries.
///
/// Returns the number of neighbor enforcement subdivisions performed.
fn enforce_neighbor_gradation(
  leaves: &mut HashSet<OctreeNode>,
  groups: &mut Vec<TransitionGroup>,
  config: &OctreeConfig,
  budget: &RefinementBudget,
) -> usize {
  if !budget.neighbor_enforcement_enabled() {
    return 0;
  }

  let mut neighbor_subdivisions = 0;
  let max_iterations = if budget.max_neighbor_iterations > 0 {
    budget.max_neighbor_iterations
  } else {
    4
  };

  for _iteration in 0..max_iterations {
    let mut changed = false;

    // Snapshot current leaves (can't iterate while modifying)
    let snapshot: Vec<_> = leaves.iter().copied().collect();

    for node in snapshot {
      for dir in 0..6 {
        if let Some(neighbor) = find_face_neighbor(&node, dir, leaves, config.max_lod) {
          let lod_diff = neighbor.lod - node.lod;

					// If neighbor is too coarse, subdivide it
					if lod_diff > budget.max_relative_lod {
						// Can only subdivide if neighbor LOD > MinLOD
						if neighbor.lod > config.min_lod && leaves.contains(&neighbor) {
							apply_subdivide(&neighbor, leaves, groups, Some(config));
							neighbor_subdivisions += 1;
							changed = true;
						}
					}
        }
      }
    }

    // Stop if no changes needed (converged)
    if !changed {
      break;
    }
  }

  neighbor_subdivisions
}

/// Main refinement function.
///
/// Determines which nodes to subdivide or merge based on viewer distance.
///
/// # Algorithm Phases
///
/// 1. **Identify candidates**: Find nodes that need subdivision (too close) or
///    collapse (too far)
/// 2. **Validate collapses**: Ensure all 8 children are leaves before merging
/// 3. **Sort by priority**: Subdivisions closest-first, collapses
///    farthest-first
/// 4. **Apply collapses**: Shed distant load first (budget-limited)
/// 5. **Apply subdivisions**: Add nearby detail (budget-limited)
/// 6. **Enforce neighbors**: Fix LOD gradation to prevent T-junctions
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all, name = "octree::refine"))]
pub fn refine(input: RefinementInput) -> RefinementOutput {
  let mut next_leaves = input.prev_leaves.clone();
  let mut to_subdivide: Vec<OctreeNode> = Vec::new();
  let mut coarsen_candidates: HashSet<OctreeNode> = HashSet::new();
  let mut stats = RefinementStats::default();

  // Phase 1: Identify candidates
  {
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!("identify_candidates").entered();
    for node in &input.prev_leaves {
      // Skip nodes outside world bounds
      if !input.config.node_overlaps_bounds(node) {
        continue;
      }

      // Check subdivision (LOD > MinLOD)
      if node.lod > input.config.min_lod {
        let center = input.config.get_node_center(node);
        let dist = input.viewer_pos.distance(center);
        let threshold = input.config.get_threshold(node.lod);

        if dist < threshold {
          to_subdivide.push(*node);
          continue;
        }
      }

      // Check coarsening (LOD < MaxLOD)
      if node.lod < input.config.max_lod {
        if let Some(parent) = node.get_parent(input.config.max_lod) {
          let parent_center = input.config.get_node_center(&parent);
          let parent_dist = input.viewer_pos.distance(parent_center);
          let parent_threshold = input.config.get_threshold(parent.lod);

          if parent_dist >= parent_threshold {
            coarsen_candidates.insert(parent);
          }
        }
      }
    }
  }

  // Phase 2: Validate coarsening (all 8 children must be leaves)
  let valid_coarsen: Vec<_> = {
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!("validate_coarsen").entered();
    coarsen_candidates
      .into_iter()
      .filter(|parent| all_children_are_leaves(parent, &next_leaves))
      .collect()
  };

  // Phase 3: Sort by priority
  let config = &input.config;
  let viewer_pos = input.viewer_pos;

  let mut to_subdivide = to_subdivide;
  let mut valid_coarsen = valid_coarsen;
  {
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!("sort_by_priority").entered();
    // Subdivisions: closest first (highest priority)
    to_subdivide.sort_by(|a, b| {
      let da = viewer_pos.distance_squared(config.get_node_center(a));
      let db = viewer_pos.distance_squared(config.get_node_center(b));
      da.partial_cmp(&db).unwrap()
    });

    // Collapses: farthest first (shed distant load)
    valid_coarsen.sort_by(|a, b| {
      let da = viewer_pos.distance_squared(config.get_node_center(a));
      let db = viewer_pos.distance_squared(config.get_node_center(b));
      db.partial_cmp(&da).unwrap() // Reversed!
    });
  }

  let mut transition_groups = Vec::new();

  // Phase 4: Apply collapses first (shed load)
  {
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!("apply_collapses").entered();
    for parent in valid_coarsen.into_iter() {
      if !input.budget.can_collapse(stats.collapses_performed) {
        break;
      }
      apply_merge(&parent, &mut next_leaves, &mut transition_groups);
      stats.collapses_performed += 1;
    }
  }

  // Phase 5: Apply subdivisions
  {
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!("apply_subdivisions").entered();
    for node in to_subdivide.into_iter() {
      if !input.budget.can_subdivide(stats.subdivisions_performed) {
        break;
      }
      // Skip if already removed by a collapse
      if !next_leaves.contains(&node) {
        continue;
      }
      apply_subdivide(&node, &mut next_leaves, &mut transition_groups, Some(config));
      stats.subdivisions_performed += 1;
    }
  }

  // Phase 6: Neighbor enforcement
  {
    #[cfg(feature = "tracing")]
    let _span = tracing::info_span!("neighbor_enforcement").entered();
    stats.neighbor_subdivisions_performed = enforce_neighbor_gradation(
      &mut next_leaves,
      &mut transition_groups,
      &input.config,
      &input.budget,
    );
  }

  // Sort transition groups by proximity (for presentation priority)
  transition_groups.sort_by(|a, b| {
    let da = viewer_pos.distance_squared(config.get_node_center(&a.group_key));
    let db = viewer_pos.distance_squared(config.get_node_center(&b.group_key));
    da.partial_cmp(&db).unwrap()
  });

  RefinementOutput {
    next_leaves,
    transition_groups,
    stats,
  }
}

#[cfg(test)]
#[path = "refinement_test.rs"]
mod refinement_test;
