//! OctreeConfig - configuration for octree refinement and world coordinate
//! mapping.

use glam::DVec3;

use super::OctreeNode;
use crate::constants::INTERIOR_CELLS;

/// Number of voxels per cell for world size calculations.
pub const VOXELS_PER_CELL: usize = INTERIOR_CELLS; // 28

/// Configuration for octree refinement and world coordinate mapping.
#[derive(Clone, Debug)]
pub struct OctreeConfig {
  /// Base voxel size in world units.
  pub voxel_size: f64,

  /// World-space origin for coordinate calculations.
  pub world_origin: DVec3,

  /// Finest LOD level (highest detail). Typically 0.
  pub min_lod: i32,

  /// Coarsest LOD level allowed.
  pub max_lod: i32,

  /// LOD exponent: scales distance thresholds.
  /// threshold = cell_size * 2^lod_exponent
  pub lod_exponent: f64,
}

impl OctreeConfig {
  /// Calculate cell size at given LOD.
  /// cell_size = voxel_size * VOXELS_PER_CELL * 2^LOD
  #[inline]
  pub fn get_cell_size(&self, lod: i32) -> f64 {
    self.voxel_size * (VOXELS_PER_CELL as f64) * (1u64 << lod) as f64
  }

  /// Calculate voxel size at given LOD (for sampling).
  /// voxel_at_lod = voxel_size * 2^LOD
  #[inline]
  pub fn get_voxel_size(&self, lod: i32) -> f64 {
    self.voxel_size * (1u64 << lod) as f64
  }

  /// Calculate refinement threshold for LOD.
  /// threshold = cell_size * 2^lod_exponent
  #[inline]
  pub fn get_threshold(&self, lod: i32) -> f64 {
    let cell_size = self.get_cell_size(lod);
    let lod_scale = 2.0_f64.powf(self.lod_exponent);
    cell_size * lod_scale
  }

  /// Get world-space minimum corner of a node.
  #[inline]
  pub fn get_node_min(&self, node: &OctreeNode) -> DVec3 {
    let cell_size = self.get_cell_size(node.lod);
    self.world_origin
      + DVec3::new(
        node.x as f64 * cell_size,
        node.y as f64 * cell_size,
        node.z as f64 * cell_size,
      )
  }

  /// Get world-space center of a node.
  #[inline]
  pub fn get_node_center(&self, node: &OctreeNode) -> DVec3 {
    let cell_size = self.get_cell_size(node.lod);
    self.get_node_min(node) + DVec3::splat(cell_size * 0.5)
  }
}

impl Default for OctreeConfig {
  fn default() -> Self {
    Self {
      voxel_size: 1.0,
      world_origin: DVec3::ZERO,
      min_lod: 0,
      max_lod: 30,
      lod_exponent: 0.0,
    }
  }
}

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;
