//! Axis-aligned bounding box with double precision for huge worlds.

use glam::DVec3;

/// Double-precision axis-aligned bounding box.
///
/// Used to define world boundaries that constrain octree refinement.
/// Nodes outside the bounds are ignored for refinement, scheduling, and rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DAabb3 {
	/// Minimum corner (inclusive).
	pub min: DVec3,
	/// Maximum corner (inclusive).
	pub max: DVec3,
}

impl DAabb3 {
	/// Create a new AABB from min and max corners.
	///
	/// # Panics
	/// Debug-asserts that min <= max on all axes.
	pub fn new(min: DVec3, max: DVec3) -> Self {
		debug_assert!(
			min.x <= max.x && min.y <= max.y && min.z <= max.z,
			"AABB min must be <= max on all axes"
		);
		Self { min, max }
	}

	/// Create a new AABB from center and half-extents.
	///
	/// Useful for defining world bounds symmetrically around an origin.
	pub fn from_center_half_extents(center: DVec3, half_extents: DVec3) -> Self {
		Self {
			min: center - half_extents,
			max: center + half_extents,
		}
	}

	/// Check if this AABB overlaps with another.
	///
	/// Two AABBs overlap if they share any interior or boundary points.
	#[inline]
	pub fn overlaps(&self, other: &DAabb3) -> bool {
		self.min.x <= other.max.x
			&& self.max.x >= other.min.x
			&& self.min.y <= other.max.y
			&& self.max.y >= other.min.y
			&& self.min.z <= other.max.z
			&& self.max.z >= other.min.z
	}

	/// Check if this AABB contains a point.
	#[inline]
	pub fn contains_point(&self, point: DVec3) -> bool {
		point.x >= self.min.x
			&& point.x <= self.max.x
			&& point.y >= self.min.y
			&& point.y <= self.max.y
			&& point.z >= self.min.z
			&& point.z <= self.max.z
	}

	/// Get the size of the AABB (max - min).
	#[inline]
	pub fn size(&self) -> DVec3 {
		self.max - self.min
	}

	/// Get the center of the AABB.
	#[inline]
	pub fn center(&self) -> DVec3 {
		(self.min + self.max) * 0.5
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new() {
		let aabb = DAabb3::new(DVec3::new(-1.0, -2.0, -3.0), DVec3::new(1.0, 2.0, 3.0));
		assert_eq!(aabb.min, DVec3::new(-1.0, -2.0, -3.0));
		assert_eq!(aabb.max, DVec3::new(1.0, 2.0, 3.0));
	}

	#[test]
	fn test_from_center_half_extents() {
		let aabb = DAabb3::from_center_half_extents(DVec3::ZERO, DVec3::splat(10.0));
		assert_eq!(aabb.min, DVec3::splat(-10.0));
		assert_eq!(aabb.max, DVec3::splat(10.0));
	}

	#[test]
	fn test_overlaps_true() {
		let a = DAabb3::new(DVec3::ZERO, DVec3::splat(10.0));
		let b = DAabb3::new(DVec3::splat(5.0), DVec3::splat(15.0));
		assert!(a.overlaps(&b));
		assert!(b.overlaps(&a));
	}

	#[test]
	fn test_overlaps_touching() {
		// Touching at boundary should count as overlapping
		let a = DAabb3::new(DVec3::ZERO, DVec3::splat(10.0));
		let b = DAabb3::new(DVec3::splat(10.0), DVec3::splat(20.0));
		assert!(a.overlaps(&b));
		assert!(b.overlaps(&a));
	}

	#[test]
	fn test_overlaps_false() {
		let a = DAabb3::new(DVec3::ZERO, DVec3::splat(10.0));
		let b = DAabb3::new(DVec3::splat(11.0), DVec3::splat(20.0));
		assert!(!a.overlaps(&b));
		assert!(!b.overlaps(&a));
	}

	#[test]
	fn test_contains_point() {
		let aabb = DAabb3::new(DVec3::ZERO, DVec3::splat(10.0));

		// Inside
		assert!(aabb.contains_point(DVec3::splat(5.0)));

		// On boundary
		assert!(aabb.contains_point(DVec3::ZERO));
		assert!(aabb.contains_point(DVec3::splat(10.0)));

		// Outside
		assert!(!aabb.contains_point(DVec3::splat(-1.0)));
		assert!(!aabb.contains_point(DVec3::splat(11.0)));
	}

	#[test]
	fn test_size() {
		let aabb = DAabb3::new(DVec3::new(-1.0, -2.0, -3.0), DVec3::new(1.0, 2.0, 3.0));
		assert_eq!(aabb.size(), DVec3::new(2.0, 4.0, 6.0));
	}

	#[test]
	fn test_center() {
		let aabb = DAabb3::new(DVec3::new(-1.0, -2.0, -3.0), DVec3::new(1.0, 2.0, 3.0));
		assert_eq!(aabb.center(), DVec3::ZERO);
	}
}
