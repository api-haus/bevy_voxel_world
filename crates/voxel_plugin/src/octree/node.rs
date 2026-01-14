//! OctreeNode - immutable value type representing a position in the octree.
//!
//! Nodes are identified by their grid coordinates at their LOD level.
//! LOD 0 = finest detail (smallest cells), higher LOD = coarser.

/// Octree node - immutable value type.
///
/// Grid coordinates are at the node's own LOD level, not the finest level.
/// This simplifies parent/child calculations compared to finest-LOD alignment.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OctreeNode {
  /// Grid X position at this node's LOD level
  pub x: i32,
  /// Grid Y position at this node's LOD level
  pub y: i32,
  /// Grid Z position at this node's LOD level
  pub z: i32,
  /// Level of detail (0 = finest, higher = coarser)
  pub lod: i32,
}

impl OctreeNode {
  /// Create a new node at the given position and LOD.
  pub fn new(x: i32, y: i32, z: i32, lod: i32) -> Self {
    Self { x, y, z, lod }
  }

  /// Get child node (finer detail: LOD - 1).
  ///
  /// Octant: 0-7 where bits represent +X, +Y, +Z offsets:
  /// - bit 0: X offset (0 or 1)
  /// - bit 1: Y offset (0 or 1)
  /// - bit 2: Z offset (0 or 1)
  ///
  /// Returns None if already at LOD 0 (cannot subdivide further).
  pub fn get_child(&self, octant: u8) -> Option<Self> {
    if self.lod <= 0 {
      return None;
    }
    let cx = (octant & 1) as i32;
    let cy = ((octant >> 1) & 1) as i32;
    let cz = ((octant >> 2) & 1) as i32;
    Some(Self {
      x: self.x * 2 + cx,
      y: self.y * 2 + cy,
      z: self.z * 2 + cz,
      lod: self.lod - 1,
    })
  }

  /// Get parent node (coarser: LOD + 1).
  ///
  /// Returns None if already at max_lod (cannot go coarser).
  pub fn get_parent(&self, max_lod: i32) -> Option<Self> {
    if self.lod >= max_lod {
      return None;
    }
    Some(Self {
      x: self.x / 2,
      y: self.y / 2,
      z: self.z / 2,
      lod: self.lod + 1,
    })
  }
}

#[cfg(test)]
#[path = "node_test.rs"]
mod node_test;
