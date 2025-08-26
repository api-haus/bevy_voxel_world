use avian3d::prelude::{Collider, RigidBody};

use bevy::prelude::*;

use bevy::render::mesh::Mesh;

use std::time::Instant;

use tracing::{info_span, trace};

use crate::voxel_plugin::meshing::bevy_mesh::buffer_to_meshes_per_material;

use crate::voxel_plugin::meshing::surface_nets::select_vertex_materials_from_positions;

use crate::voxel_plugin::voxels::storage::VoxelStorage;

use ilattice::prelude::IVec3 as ILVec3;

pub(crate) fn apply_remeshes(
	desc: Res<super::VoxelVolumeDesc>,
	render_mat: Option<Res<super::VoxelRenderMaterial>>,
	mut telemetry: ResMut<super::VoxelTelemetry>,
	mut meshes: ResMut<Assets<Mesh>>,
	mut commands: Commands,
	mut evr: EventReader<super::RemeshReady>,
	mut q_chunk_tf: Query<(&super::VoxelChunk, &VoxelStorage, &mut Transform)>,
) {
	let Some(render_mat) = render_mat else {
		return;
	};

	for ev in evr.read() {
		let span = info_span!(
				"apply_mesh",
				entity = ?ev.entity,
				positions = ev.buffer.positions.len() as i64,
				indices = ev.buffer.indices.len() as i64
		);
		let _enter = span.enter();
		let t0 = Instant::now();

		// If buffer is empty (no surface), mark done and continue
		if ev.buffer.positions.is_empty() {
			commands
				.entity(ev.entity)
				.remove::<super::NeedsInitialMesh>();
			continue;
		}

		let vertex_colors: Option<Vec<[f32; 4]>> = if let Some(colors) = &ev.vertex_colors {
			Some(colors.clone())
		} else if let Ok((_chunk, storage, _tf)) = q_chunk_tf.get_mut(ev.entity) {
			let mats = select_vertex_materials_from_positions(storage, &ev.buffer.positions);
			Some(
				mats
					.iter()
					.map(|&m| [(m as f32) / 255.0, 0.0, 0.0, 1.0])
					.collect(),
			)
		} else {
			None
		};

		let meshes_vec = buffer_to_meshes_per_material(&ev.buffer, vertex_colors.as_deref());

		if meshes_vec.is_empty() {
			// trace!(target: "vox", "apply_mesh: empty meshes_vec for entity {:?}", ev.entity);
			commands
				.entity(ev.entity)
				.remove::<super::NeedsInitialMesh>();
			continue;
		}

		let mesh = meshes_vec.into_iter().next().unwrap();
		trace!("compute_aabb_begin");
		// info!(target: "vox", "apply_mesh: mesh bounds {:?} for entity {:?}", aabb, ev.entity);
		let mesh_handle = meshes.add(mesh);
		let mesh_id = mesh_handle.id();

		if let Ok((chunk, _storage, mut transform)) = q_chunk_tf.get_mut(ev.entity) {
			let core = desc.chunk_core_dims;
			let offset = ILVec3::new(
				(core.x as i32) * chunk.chunk_coords.x,
				(core.y as i32) * chunk.chunk_coords.y,
				(core.z as i32) * chunk.chunk_coords.z,
			);
			let min = desc.origin_cell + offset - ILVec3::ONE;
			transform.translation = Vec3::new(min.x as f32, min.y as f32, min.z as f32);

			// info!(target: "vox", "apply_mesh: adding mesh to entity {:?} at position {:?}", ev.entity, transform.translation);
			commands.entity(ev.entity).insert((
				Mesh3d(mesh_handle.clone()),
				MeshMaterial3d(render_mat.handle.clone()),
			));
			// Mark initial meshing done for loading state tracking
			commands
				.entity(ev.entity)
				.remove::<super::NeedsInitialMesh>();

			if let Some(mesh_ref) = meshes.get(mesh_id) {
				trace!("collider_build_begin");

				if let Some(collider) = Collider::trimesh_from_mesh(mesh_ref) {
					commands
						.entity(ev.entity)
						.insert((RigidBody::Static, collider));
				}
			}

			telemetry.total_meshed += 1;
			telemetry.meshed_this_frame += 1;
			telemetry.apply_time_ms_frame += t0.elapsed().as_secs_f32() * 1000.0;
		}
	}
}
