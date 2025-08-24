//! Chunk generation services

use crate::voxel::entities::*;
use crate::voxel::ports::RandomGenerator;
use crate::voxel::types::*;

/// Generate random spheres in a volume
pub fn generate_random_spheres(
	volume: &mut VoxelVolume,
	sphere_count: usize,
	min_radius: f32,
	max_radius: f32,
	rng: &mut impl RandomGenerator,
) {
	let vol_dims = volume.config.grid_dims;
	let chunk_dims = volume.config.chunk_core_dims;
	let total_size = ilattice::prelude::IVec3::new(
		(vol_dims.x * chunk_dims.x) as i32,
		(vol_dims.y * chunk_dims.y) as i32,
		(vol_dims.z * chunk_dims.z) as i32,
	);

	for _ in 0..sphere_count {
		let sphere = SdfSphere {
			center: [
				rng.random_range_f32(0.0, total_size.x as f32),
				rng.random_range_f32(0.0, total_size.y as f32),
				rng.random_range_f32(0.0, total_size.z as f32),
			],
			radius: rng.random_range_f32(min_radius, max_radius),
		};

		// Apply sphere to all affected chunks
		for chunk in volume.chunks.iter_mut() {
			let extent = super::chunk_sample_extent(chunk.coords, &volume.config);
			let aabb_min = [
				extent.minimum.x as f32,
				extent.minimum.y as f32,
				extent.minimum.z as f32,
			];
			let aabb_max = [
				(extent.minimum.x + extent.shape.x - 1) as f32,
				(extent.minimum.y + extent.shape.y - 1) as f32,
				(extent.minimum.z + extent.shape.z - 1) as f32,
			];

			if super::sphere_intersects_aabb(&sphere, aabb_min, aabb_max) {
				// Generate a material ID based on position (deterministic)
				let material = material_from_position(&sphere.center);
				super::apply_sphere_edit(chunk, &sphere, EditOperation::Place { material }, extent);
			}
		}
	}
}

/// Generate deterministic material ID from position
fn material_from_position(pos: &[f32; 3]) -> MaterialId {
	const REGION: i32 = 5;
	const SEED: u32 = 0xA53C_9E21;

	let gx = (pos[0] as i32).div_euclid(REGION) as u32;
	let gy = (pos[1] as i32).div_euclid(REGION) as u32;
	let gz = (pos[2] as i32).div_euclid(REGION) as u32;

	let h = hash32(gx ^ hash32(gy ^ hash32(gz ^ SEED)));
	let id = (h % 255) as u8;
	MaterialId(if id == 0 { 1 } else { id })
}

/// Simple integer hash function
#[inline]
fn hash32(mut x: u32) -> u32 {
	x ^= x >> 16;
	x = x.wrapping_mul(0x7feb_352d);
	x ^= x >> 15;
	x = x.wrapping_mul(0x846c_a68b);
	x ^= x >> 16;
	x
}
