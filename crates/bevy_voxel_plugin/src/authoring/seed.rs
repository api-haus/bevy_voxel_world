use bevy::prelude::*;
use bevy_prng::*;
use bevy_rand::global::GlobalEntropy;
use ilattice::prelude::{IVec3 as ILVec3, UVec3};
use rand::Rng;
use rayon::prelude::*;
use tracing::{debug, info, info_span, trace};

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
#[allow(dead_code)]
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
	let chunks_total = q_chunks.iter().count();
	info!(target: "vox", "seed_random_spheres_sdf: begin, chunks_total={}", chunks_total);
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

	debug!("seed_tasks spheres={}, tasks={}", sphere_count, tasks.len());
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

			let mut _any_solid = false;
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
							_any_solid = true;
							material_from_noise(p)
						} else {
							AIR_ID
						};
					}
				}
			}

			(task.entity, sdf, mat)
		})
		.collect();

	let mut enq = 0usize;
	for (entity, sdf, mat) in results.into_iter() {
		if let Ok((e, mut storage, _chunk)) = q_chunks.get_mut(entity) {
			storage.sdf.copy_from_slice(&sdf);
			storage.mat.copy_from_slice(&mat);
			queue.inner.push_back(e);
			trace!(target: "vox", "seed_chunk_enqueued entity={:?}", e);
			enq += 1;
		}
	}

	info!(target: "vox", "seed_random_spheres_sdf: end, enqueued={} of {}", enq, chunks_total);
}

/// Terrain heightfield with random rotated cubes embedded.
/// Approximately fills ~60% of the volume by centering height near 60% of Y with coarse variation.
pub(crate) fn seed_terrain_noise_sdf(
	desc: Res<crate::plugin::VoxelVolumeDesc>,
	mut queue: ResMut<crate::plugin::RemeshQueue>,
	mut q_chunks: Query<(Entity, &mut VoxelStorage, &crate::plugin::VoxelChunk)>,
	mut rng: GlobalEntropy<WyRand>,
) {
	let span = info_span!(
			"seed_terrain_noise_sdf",
			grid = ?desc.grid_dims,
			core = ?desc.chunk_core_dims,
			origin = ?desc.origin_cell
	);
	let _enter = span.enter();
	let chunks_total = q_chunks.iter().count();
	info!(target: "vox", "seed_terrain_noise_sdf: begin, chunks_total={}", chunks_total);

	let vol_shape = ILVec3::new(
		(desc.grid_dims.x * desc.chunk_core_dims.x) as i32,
		(desc.grid_dims.y * desc.chunk_core_dims.y) as i32,
		(desc.grid_dims.z * desc.chunk_core_dims.z) as i32,
	);

	// Smooth value-noise-based height function
	#[inline]
	fn smoothstep(t: f32) -> f32 {
		t * t * (3.0 - 2.0 * t)
	}

	#[inline]
	fn value_noise2d(x: f32, z: f32, cell: f32, seed: u32) -> f32 {
		let xf = x / cell;
		let zf = z / cell;
		let x0 = xf.floor();
		let z0 = zf.floor();
		let tx = smoothstep(xf - x0);
		let tz = smoothstep(zf - z0);
		let x0i = x0 as i32;
		let z0i = z0 as i32;
		let x1i = x0i + 1;
		let z1i = z0i + 1;
		let v00 = (hash3(x0i as u32, 0, z0i as u32, seed) as f32) / (u32::MAX as f32);
		let v10 = (hash3(x1i as u32, 0, z0i as u32, seed) as f32) / (u32::MAX as f32);
		let v01 = (hash3(x0i as u32, 0, z1i as u32, seed) as f32) / (u32::MAX as f32);
		let v11 = (hash3(x1i as u32, 0, z1i as u32, seed) as f32) / (u32::MAX as f32);
		let v0 = v00 + (v10 - v00) * tx;
		let v1 = v01 + (v11 - v01) * tx;
		let v = v0 + (v1 - v0) * tz; // 0..1
		v * 2.0 - 1.0 // -> -1..1
	}

	#[inline]
	fn height_with_noise(x: f32, z: f32, base: f32, amp: f32) -> f32 {
		let n = 0.5 * value_noise2d(x, z, 48.0, 0xA53C_9E21)
			+ 0.3 * value_noise2d(x, z, 24.0, 0xBEEF_CAFE)
			+ 0.2 * value_noise2d(x, z, 12.0, 0x1234_5678);
		base + n * amp
	}

	// Pre-generate rotated cubes scattered through the world
	#[derive(Clone, Copy)]
	struct RotCube {
		center: Vec3,
		half_extents: Vec3,
		rot: Quat,
		aabb_min: ILVec3,
		aabb_max: ILVec3,
	}

	let base_h = 0.6f32 * vol_shape.y as f32;
	let amp = 0.18f32 * vol_shape.y as f32;
	let cube_count =
		((vol_shape.x * vol_shape.y * vol_shape.z) as f32 / 262_144.0).clamp(6.0, 24.0) as usize;
	let cubes: Vec<RotCube> = (0..cube_count)
		.map(|_| {
			let cx = rng.random_range(0..vol_shape.x) as f32;
			let cz = rng.random_range(0..vol_shape.z) as f32;
			// Place around smoothed terrain height with some spread
			let cy = (height_with_noise(cx, cz, base_h, amp) + rng.random_range(-4.0f32..6.0f32))
				.clamp(2.0, vol_shape.y as f32 - 2.0);

			let hx = rng.random_range(2.0f32..6.0f32);
			let hy = rng.random_range(2.0f32..6.0f32);
			let hz = rng.random_range(2.0f32..6.0f32);
			let yaw = rng.random_range(-std::f32::consts::PI..std::f32::consts::PI);
			let pitch = rng.random_range(-0.4f32..0.4f32);
			let roll = rng.random_range(-0.4f32..0.4f32);
			let rot = Quat::from_euler(EulerRot::ZYX, roll, pitch, yaw);
			let center = Vec3::new(cx, cy, cz);
			// Conservative AABB radius
			let r = Vec3::new(hx, hy, hz).length() + 2.0;
			let aabb_min = ILVec3::new(
				(center.x - r).floor() as i32,
				(center.y - r).floor() as i32,
				(center.z - r).floor() as i32,
			);
			let aabb_max = ILVec3::new(
				(center.x + r).ceil() as i32,
				(center.y + r).ceil() as i32,
				(center.z + r).ceil() as i32,
			);
			RotCube {
				center,
				half_extents: Vec3::new(hx, hy, hz),
				rot,
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

			// Filter cubes intersecting this chunk
			let intersecting: Vec<_> = cubes
				.iter()
				.filter(|c| {
					!(c.aabb_max.x < chunk_min.x
						|| c.aabb_min.x > chunk_max.x
						|| c.aabb_max.y < chunk_min.y
						|| c.aabb_min.y > chunk_max.y
						|| c.aabb_max.z < chunk_min.z
						|| c.aabb_min.z > chunk_max.z)
				})
				.copied()
				.collect();

			for z in 0..sz {
				for y in 0..sy {
					for x in 0..sx {
						let p = ILVec3::new(x as i32, y as i32, z as i32) + chunk_min;
						let idx = linear_index(x, y, z, task.sample_dims);

						// Terrain height at (x,z)
						let height = height_with_noise(p.x as f32, p.z as f32, base_h, amp);
						let d_terrain = (p.y as f32) - height;

						// Rotated cubes SDF union
						let mut d = d_terrain;
						if !intersecting.is_empty() {
							let pf = Vec3::new(p.x as f32, p.y as f32, p.z as f32);
							for c in &intersecting {
								let local = c.rot.inverse() * (pf - c.center);
								let q = local.abs() - c.half_extents;
								let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0));
								let dist_out = outside.length();
								let dist_in = q.x.max(q.y.max(q.z)).min(0.0);
								let d_box = dist_out + dist_in;
								d = d.min(d_box);
							}
						}

						sdf[idx] = d;
						if d <= 0.0 {
							// mark that this chunk has some solids; currently unused but kept for debugging potential
							mat[idx] = material_from_noise(p);
						} else {
							mat[idx] = AIR_ID;
						}
					}
				}
			}

			(task.entity, sdf, mat)
		})
		.collect();

	let mut enq = 0usize;
	for (entity, sdf, mat) in results.into_iter() {
		if let Ok((e, mut storage, _chunk)) = q_chunks.get_mut(entity) {
			storage.sdf.copy_from_slice(&sdf);
			storage.mat.copy_from_slice(&mat);
			queue.inner.push_back(e);
			trace!(target: "vox", "seed_chunk_enqueued entity={:?}", e);
			enq += 1;
		}
	}

	info!(target: "vox", "seed_terrain_noise_sdf: end, enqueued={} of {}", enq, chunks_total);
}
