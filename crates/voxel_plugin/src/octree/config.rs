//! OctreeConfig - configuration for octree refinement and world coordinate
//! mapping.

use glam::DVec3;

use super::bounds::DAabb3;
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

	/// Optional world bounds - nodes outside are ignored.
	/// None = unbounded (backward compatible).
	pub world_bounds: Option<DAabb3>,
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

	/// Get world-space AABB of a node.
	#[inline]
	pub fn get_node_aabb(&self, node: &OctreeNode) -> DAabb3 {
		let min = self.get_node_min(node);
		let cell_size = self.get_cell_size(node.lod);
		DAabb3::new(min, min + DVec3::splat(cell_size))
	}

	/// Check if a node overlaps the world bounds.
	///
	/// Returns true if:
	/// - No world bounds set (unbounded)
	/// - Node AABB overlaps world bounds
	#[inline]
	pub fn node_overlaps_bounds(&self, node: &OctreeNode) -> bool {
		match &self.world_bounds {
			None => true, // Unbounded - all nodes are valid
			Some(bounds) => {
				let node_aabb = self.get_node_aabb(node);
				bounds.overlaps(&node_aabb)
			}
		}
	}

	/// Compute initial leaves that cover the world bounds at target LOD.
	///
	/// Returns an empty vec if no world bounds are set.
	pub fn compute_initial_leaves(&self, target_lod: i32) -> Vec<OctreeNode> {
		let Some(bounds) = &self.world_bounds else {
			return Vec::new();
		};

		// Convert world bounds to grid coordinates at target LOD
		let (min_x, min_y, min_z) = self.world_to_grid(bounds.min, target_lod);
		let (max_x, max_y, max_z) = self.world_to_grid(bounds.max, target_lod);

		let mut leaves = Vec::new();
		for x in min_x..=max_x {
			for y in min_y..=max_y {
				for z in min_z..=max_z {
					let node = OctreeNode::new(x, y, z, target_lod);
					// Double-check overlap (handles edge cases)
					if self.node_overlaps_bounds(&node) {
						leaves.push(node);
					}
				}
			}
		}

		leaves
	}

	/// Suggest an initial LOD based on world bounds.
	///
	/// Tries to find an LOD where the world fits in a reasonable number of cells
	/// (targeting roughly 2-4 cells per axis for the initial view).
	pub fn suggest_initial_lod(&self) -> i32 {
		let Some(bounds) = &self.world_bounds else {
			return self.max_lod / 2; // Default fallback
		};

		let world_size = bounds.size();
		let max_extent = world_size.x.max(world_size.y).max(world_size.z);

		// Target: ~3 cells per axis at initial LOD
		// cell_size = voxel_size * VOXELS_PER_CELL * 2^LOD
		// max_extent / cell_size ≈ 3
		// cell_size ≈ max_extent / 3
		// voxel_size * VOXELS_PER_CELL * 2^LOD ≈ max_extent / 3
		// 2^LOD ≈ max_extent / (3 * voxel_size * VOXELS_PER_CELL)
		// LOD ≈ log2(max_extent / (3 * voxel_size * VOXELS_PER_CELL))

		let base_cell = self.voxel_size * VOXELS_PER_CELL as f64;
		let target_cell_size = max_extent / 3.0;
		let lod_float = (target_cell_size / base_cell).log2();
		let lod = lod_float.round() as i32;

		// Clamp to valid range
		lod.clamp(self.min_lod, self.max_lod)
	}

	/// Convert world position to grid coordinates at given LOD.
	fn world_to_grid(&self, world_pos: DVec3, lod: i32) -> (i32, i32, i32) {
		let cell_size = self.get_cell_size(lod);
		let relative = world_pos - self.world_origin;
		(
			(relative.x / cell_size).floor() as i32,
			(relative.y / cell_size).floor() as i32,
			(relative.z / cell_size).floor() as i32,
		)
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
			world_bounds: None,
		}
	}
}

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;
