//! Voxel editing operations

use crate::voxel::entities::*;

use crate::voxel::types::*;

/// Apply a sphere edit operation to a chunk
pub fn apply_sphere_edit(
	chunk: &mut VoxelChunk,
	sphere: &SdfSphere,
	operation: EditOperation,
	chunk_extent: ilattice::prelude::Extent<ilattice::prelude::IVec3>,
) -> bool {
	let mut modified = false;

	// Convert sphere to local chunk space
	let chunk_min = chunk_extent.minimum;

	for z in 0..chunk.dims.sample.z {
		for y in 0..chunk.dims.sample.y {
			for x in 0..chunk.dims.sample.x {
				let world_pos = [
					(chunk_min.x + x as i32) as f32,
					(chunk_min.y + y as i32) as f32,
					(chunk_min.z + z as i32) as f32,
				];

				let local_pos = LocalVoxelPos { x, y, z };
				let sphere_sdf = super::evaluate_sphere(world_pos, sphere);
				let current_sdf = chunk.sdf_at(local_pos);

				let new_sdf = match operation {
					EditOperation::Destroy => {
						// Carve out (make positive)
						SdfValue(current_sdf.0.max(-sphere_sdf))
					}

					EditOperation::Place { .. } => {
						// Add material (make negative)
						SdfValue(current_sdf.0.min(sphere_sdf))
					}
				};

				if (new_sdf.0 - current_sdf.0).abs() > 1e-6 {
					chunk.set_sdf(local_pos, new_sdf);

					// Update material based on transition
					match operation {
						EditOperation::Destroy => {
							if current_sdf.is_solid() && !new_sdf.is_solid() {
								chunk.set_material(local_pos, MaterialId::AIR);
							}
						}

						EditOperation::Place { material } => {
							if !current_sdf.is_solid() && new_sdf.is_solid() {
								chunk.set_material(local_pos, material);
							}
						}
					}

					modified = true;
				}
			}
		}
	}

	modified
}

/// Check if a sphere intersects with an AABB
pub fn sphere_intersects_aabb(sphere: &SdfSphere, aabb_min: [f32; 3], aabb_max: [f32; 3]) -> bool {
	let mut closest_point = [0.0; 3];

	for i in 0..3 {
		closest_point[i] = sphere.center[i].clamp(aabb_min[i], aabb_max[i]);
	}

	let dx = sphere.center[0] - closest_point[0];
	let dy = sphere.center[1] - closest_point[1];
	let dz = sphere.center[2] - closest_point[2];
	let distance_squared = dx * dx + dy * dy + dz * dz;

	distance_squared <= sphere.radius * sphere.radius
}
