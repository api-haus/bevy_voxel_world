use bevy::prelude::*;

use ilattice::prelude::IVec3 as ILVec3;

use crate::core::index::linear_index;

use crate::voxel_plugin::voxels::storage::{AIR_ID, VoxelStorage};

#[derive(Clone, Copy, Debug)]
pub enum EditOp {
	Destroy,
	Place,
}

#[derive(Event, Clone, Copy, Debug)]
pub struct VoxelEditEvent {
	pub center_world: Vec3,
	pub radius: f32,
	pub op: EditOp,
}

pub(crate) fn apply_edit_events(
	desc: Res<super::VoxelVolumeDesc>,
	mut queue: ResMut<super::scheduler::RemeshQueue>,
	mut evr: EventReader<VoxelEditEvent>,
	mut q_chunks: Query<(Entity, &mut VoxelStorage, &super::VoxelChunk)>,
	q_volume_xf: Query<&GlobalTransform, With<super::VoxelVolume>>,
) {
	for ev in evr.read() {
		let center = if let Ok(vol_xf) = q_volume_xf.single() {
			let to_local = vol_xf.compute_transform().compute_matrix().inverse();
			to_local.transform_point3(ev.center_world)
		} else {
			ev.center_world
		};
		let radius = ev.radius;

		for (entity, mut storage, chunk) in q_chunks.iter_mut() {
			let s = storage.dims.sample;
			let core = desc.chunk_core_dims;
			let offset = ILVec3::new(
				(core.x as i32) * chunk.chunk_coords.x,
				(core.y as i32) * chunk.chunk_coords.y,
				(core.z as i32) * chunk.chunk_coords.z,
			);
			let min = desc.origin_cell + offset - ILVec3::ONE;
			let max = ILVec3::new(
				min.x + (s.x as i32 - 1),
				min.y + (s.y as i32 - 1),
				min.z + (s.z as i32 - 1),
			);

			if !sphere_aabb_intersects(center, radius, min, max) {
				continue;
			}

			let mut changed = false;

			for z in 0..s.z {
				for y in 0..s.y {
					for x in 0..s.x {
						let p = Vec3::new(
							(min.x + x as i32) as f32,
							(min.y + y as i32) as f32,
							(min.z + z as i32) as f32,
						);
						let b = p.distance(center) - radius; // sphere SDF (negative inside)
						let idx = linear_index(x, y, z, s);
						let s_old = storage.sdf[idx];
						let s_new = match ev.op {
							EditOp::Destroy => s_old.max(-b),
							EditOp::Place => s_old.min(b),
						};

						if s_new != s_old {
							match ev.op {
								EditOp::Destroy => {
									if s_old < 0.0 && s_new >= 0.0 {
										storage.mat[idx] = AIR_ID;
									}
								}

								EditOp::Place => {
									if s_old >= 0.0 && s_new < 0.0 {
										// TODO: select material; default 1 for now
										storage.mat[idx] = 1;
									}
								}
							}

							storage.sdf[idx] = s_new;
							changed = true;
						}
					}
				}
			}

			if changed {
				queue.inner.push_back(entity);
			}
		}
	}
}

fn sphere_aabb_intersects(center: Vec3, radius: f32, min: ILVec3, max: ILVec3) -> bool {
	let mut d2 = 0.0f32;
	let c = center;
	let clamp = |v: f32, lo: f32, hi: f32| v.max(lo).min(hi);
	let px = clamp(c.x, min.x as f32, max.x as f32);
	let py = clamp(c.y, min.y as f32, max.y as f32);
	let pz = clamp(c.z, min.z as f32, max.z as f32);
	d2 += (c.x - px) * (c.x - px);
	d2 += (c.y - py) * (c.y - py);
	d2 += (c.z - pz) * (c.z - pz);
	d2 <= radius * radius
}
