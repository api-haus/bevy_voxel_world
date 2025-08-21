use bevy::prelude::*;
use bevy_prng::*;
use bevy_rand::global::GlobalEntropy;
use ilattice::prelude::{IVec3 as ILVec3, UVec3};
use rand::Rng;
use rayon::prelude::*;
use tracing::{debug, info_span, trace};

use crate::core::index::linear_index;
use crate::voxel_plugin::voxels::storage::{AIR_ID, VoxelStorage};

/// Simple integer hash mix (wyhash-inspired) for generating deterministic noise
#[inline]
fn hash32(mut x: u32) -> u32 {
	x ^= x >> 16;
	x = x.wrapping_mul(0x7feb_352d);
	x ^= x >> 15;
	x = x.wrapping_mul(0x846c_a68b);
	x ^= x >> 16;
	x
}

#[inline]
fn hash3(x: u32, y: u32, z: u32, seed: u32) -> u32 {
	hash32(x ^ hash32(y ^ hash32(z ^ seed)))
}

/// Deterministic material id from integer position using coarse 3D cells.
/// Returns a non-zero material id in [1, 255].
#[inline]
fn material_from_noise(pos: ILVec3) -> u8 {
	const REGION: i32 = 5;
	const SEED: u32 = 0xA53C_9E21;
	let gx = pos.x.div_euclid(REGION) as u32;
	let gy = pos.y.div_euclid(REGION) as u32;
	let gz = pos.z.div_euclid(REGION) as u32;
	let h = hash3(gx, gy, gz, SEED);
	let id = (h % 255u32) as u8;
	if id == 0 { 1 } else { id }
}

/// Random sphere field seeding used in the demo. Behavior identical to the previous inline version.
pub(crate) fn seed_random_spheres_sdf(
	desc: Res<crate::plugin::VoxelVolumeDesc>,
	mut queue: ResMut<crate::plugin::RemeshQueue>,
	mut q_chunks: Query<(Entity, &mut VoxelStorage, &crate::plugin::VoxelChunk)>,
	mut rng: GlobalEntropy<WyRand>,
) {
	let span = info_span!(
			"seed_random_spheres_sdf",
			grid = ?desc.grid_dims,
			core = ?desc.chunk_core_dims,
			origin = ?desc.origin_cell
	);
	let _enter = span.enter();
	let vol_shape = ILVec3::new(
		(desc.grid_dims.x * desc.chunk_core_dims.x) as i32,
		(desc.grid_dims.y * desc.chunk_core_dims.y) as i32,
		(desc.grid_dims.z * desc.chunk_core_dims.z) as i32,
	);
	let sphere_count = 400usize;
	#[derive(Clone, Copy)]
	struct Sphere {
		center: ILVec3,
		radius: f32,
		aabb_min: ILVec3,
		aabb_max: ILVec3,
	}
	let spheres: Vec<Sphere> = (0..sphere_count)
		.map(|_| {
			let cx = rng.random_range(0..vol_shape.x);
			let cy = rng.random_range(0..vol_shape.y);
			let cz = rng.random_range(0..vol_shape.z);
			let r = rng.random_range(2.0f32..16.0f32);
			let center = ILVec3::new(cx, cy, cz);
			let rr = r.ceil() as i32 + 1;
			let aabb_min = center - ILVec3::splat(rr);
			let aabb_max = center + ILVec3::splat(rr);
			Sphere {
				center,
				radius: r,
				aabb_min,
				aabb_max,
			}
		})
		.collect();

	#[derive(Clone, Copy)]
	struct ChunkTask {
		entity: Entity,
		sample_min: ILVec3,
		sample_dims: UVec3,
	}
	let tasks: Vec<ChunkTask> = q_chunks
		.iter_mut()
		.map(|(e, storage, chunk)| {
			let core = desc.chunk_core_dims;
			let offset = ILVec3::new(
				(core.x as i32) * chunk.chunk_coords.x,
				(core.y as i32) * chunk.chunk_coords.y,
				(core.z as i32) * chunk.chunk_coords.z,
			);
			let sample_min = desc.origin_cell + offset - ILVec3::ONE;
			ChunkTask {
				entity: e,
				sample_min,
				sample_dims: storage.dims.sample,
			}
		})
		.collect();

	debug!("seed_tasks count={}", tasks.len());
	let results: Vec<(Entity, Vec<f32>, Vec<u8>)> = tasks
		.par_iter()
		.map(|task| {
			let sx = task.sample_dims.x;
			let sy = task.sample_dims.y;
			let sz = task.sample_dims.z;
			let len = (sx * sy * sz) as usize;
			let mut sdf = vec![f32::INFINITY; len];
			let mut mat = vec![AIR_ID; len];
			let chunk_min = task.sample_min;
			let chunk_max = ILVec3::new(
				chunk_min.x + sx as i32 - 1,
				chunk_min.y + sy as i32 - 1,
				chunk_min.z + sz as i32 - 1,
			);

			let intersecting: Vec<_> = spheres
				.iter()
				.filter(|s| {
					!(s.aabb_max.x < chunk_min.x
						|| s.aabb_min.x > chunk_max.x
						|| s.aabb_max.y < chunk_min.y
						|| s.aabb_min.y > chunk_max.y
						|| s.aabb_max.z < chunk_min.z
						|| s.aabb_min.z > chunk_max.z)
				})
				.copied()
				.collect();

			if intersecting.is_empty() {
				trace!(target: "vox", "seed_chunk_empty entity={:?} sample_min={:?} dims={:?}", task.entity, task.sample_min, task.sample_dims);
				return (task.entity, sdf, mat);
			}

			let mut any_solid = false;
			for z in 0..sz {
				for y in 0..sy {
					for x in 0..sx {
						let p = ILVec3::new(x as i32, y as i32, z as i32) + chunk_min;
						let idx = linear_index(x, y, z, task.sample_dims);
						let mut dmin = sdf[idx];
						for s in &intersecting {
							if p.x < s.aabb_min.x
								|| p.x > s.aabb_max.x
								|| p.y < s.aabb_min.y
								|| p.y > s.aabb_max.y
								|| p.z < s.aabb_min.z
								|| p.z > s.aabb_max.z
							{
								continue;
							}
							let dx = (p.x - s.center.x) as f32;
							let dy = (p.y - s.center.y) as f32;
							let dz = (p.z - s.center.z) as f32;
							let dist = (dx * dx + dy * dy + dz * dz).sqrt();
							dmin = dmin.min(dist - s.radius);
						}
						sdf[idx] = dmin;
						mat[idx] = if dmin <= 0.0 {
							any_solid = true;
							material_from_noise(p)
						} else {
							AIR_ID
						};
					}
				}
			}

			if any_solid {
				trace!(target: "vox", "seed_chunk_solid entity={:?}", task.entity);
			} else {
				trace!(target: "vox", "seed_chunk_no_solid entity={:?}", task.entity);
			}

			(task.entity, sdf, mat)
		})
		.collect();

	for (entity, sdf, mat) in results.into_iter() {
		if let Ok((e, mut storage, _chunk)) = q_chunks.get_mut(entity) {
			storage.sdf.copy_from_slice(&sdf);
			storage.mat.copy_from_slice(&mat);
			queue.inner.push_back(e);
			trace!(target: "vox", "seed_chunk_enqueued entity={:?}", e);
		}
	}
}
