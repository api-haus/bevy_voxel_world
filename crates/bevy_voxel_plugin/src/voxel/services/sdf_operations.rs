//! Signed distance field operations

use crate::voxel::types::*;

/// Evaluate sphere SDF at a point
pub fn evaluate_sphere(point: [f32; 3], sphere: &SdfSphere) -> f32 {
	let dx = point[0] - sphere.center[0];
	let dy = point[1] - sphere.center[1];
	let dz = point[2] - sphere.center[2];
	let distance = (dx * dx + dy * dy + dz * dz).sqrt();
	distance - sphere.radius
}

/// Evaluate box SDF at a point
pub fn evaluate_box(point: [f32; 3], sdf_box: &SdfBox) -> f32 {
	let dx = (point[0] - sdf_box.center[0]).abs() - sdf_box.half_extents[0];
	let dy = (point[1] - sdf_box.center[1]).abs() - sdf_box.half_extents[1];
	let dz = (point[2] - sdf_box.center[2]).abs() - sdf_box.half_extents[2];

	let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2) + dz.max(0.0).powi(2)).sqrt();
	let inside = dx.max(dy).max(dz).min(0.0);

	outside + inside
}

/// Combine two SDF values using a CSG operation
pub fn combine_sdf(a: f32, b: f32, operation: CsgOperation) -> f32 {
	match operation {
		CsgOperation::Union => a.min(b),
		CsgOperation::Intersect => a.max(b),
		CsgOperation::Subtract => a.max(-b),
		CsgOperation::SmoothUnion { k } => smooth_min(a, b, k),
		CsgOperation::SmoothIntersect { k } => -smooth_min(-a, -b, k),
		CsgOperation::SmoothSubtract { k } => smooth_max(a, -b, k),
	}
}

/// Smooth minimum function for smooth CSG operations
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
	let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
	b.lerp(a, h) - k * h * (1.0 - h)
}

/// Smooth maximum function for smooth CSG operations
fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
	smooth_min(a, b, -k)
}

/// Linear interpolation helper
trait Lerp {
	fn lerp(self, other: Self, t: Self) -> Self;
}

impl Lerp for f32 {
	fn lerp(self, other: Self, t: Self) -> Self {
		self * (1.0 - t) + other * t
	}
}
