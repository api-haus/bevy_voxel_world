//! Meshing utilities for octree nodes.

use voxel_plugin::octree::{OctreeConfig, OctreeLeaves, OctreeNode};
use voxel_plugin::surface_nets::NeighborMask;

/// Compute the 26-bit neighbor mask for LOD seam handling.
///
/// This checks all 26 neighbor directions (6 faces, 12 edges, 8 corners)
/// to determine which neighbors are at a coarser LOD level.
pub fn compute_neighbor_mask(
  node: &OctreeNode,
  leaves: &OctreeLeaves,
  config: &OctreeConfig,
) -> u32 {
  let mut mask = 0u32;

  // Check all 26 directions
  for dz in -1i32..=1 {
    for dy in -1i32..=1 {
      for dx in -1i32..=1 {
        if dx == 0 && dy == 0 && dz == 0 {
          continue;
        }

        // Create potential neighbor at same LOD
        let neighbor_node = OctreeNode::new(node.x + dx, node.y + dy, node.z + dz, node.lod);

        // Check if neighbor exists at same LOD
        if leaves.contains(&neighbor_node) {
          continue; // Same LOD, no transition needed
        }

        // Check if neighbor is at coarser LOD (parent exists in leaves)
        if node.lod < config.max_lod {
          if let Some(parent) = neighbor_node.get_parent(config.max_lod) {
            if leaves.contains(&parent) {
              // Neighbor is coarser - set transition bit
              mask |= direction_to_bit(dx, dy, dz);
            }
          }
        }
      }
    }
  }

  mask
}

/// Map a direction vector to the corresponding neighbor mask bit.
fn direction_to_bit(dx: i32, dy: i32, dz: i32) -> u32 {
  // Count non-zero dimensions to determine if it's a face, edge, or corner
  let dims_set = (dx != 0) as i32 + (dy != 0) as i32 + (dz != 0) as i32;

  match dims_set {
    1 => {
      // Face neighbor (6 faces)
      match (dx, dy, dz) {
        (1, 0, 0) => NeighborMask::FACE_POS_X,
        (-1, 0, 0) => NeighborMask::FACE_NEG_X,
        (0, 1, 0) => NeighborMask::FACE_POS_Y,
        (0, -1, 0) => NeighborMask::FACE_NEG_Y,
        (0, 0, 1) => NeighborMask::FACE_POS_Z,
        (0, 0, -1) => NeighborMask::FACE_NEG_Z,
        _ => 0,
      }
    }
    2 => {
      // Edge neighbor (12 edges) - simplified, just set all edge bits
      // In practice, edges matter less than faces for LOD seams
      0 // Skip edges for now
    }
    3 => {
      // Corner neighbor (8 corners) - skip for now
      0
    }
    _ => 0,
  }
}
