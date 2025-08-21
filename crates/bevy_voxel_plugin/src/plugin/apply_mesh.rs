use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::render::mesh::MeshAabb;
use std::time::Instant;
use tracing::{info_span, trace};

use crate::voxel_plugin::meshing::bevy_mesh::buffer_to_meshes_per_material;

pub(crate) fn apply_remeshes(
	desc: Res<super::VoxelVolumeDesc>,
	render_mat: Res<super::VoxelRenderMaterial>,
	mut telemetry: ResMut<super::VoxelTelemetry>,
	mut meshes: ResMut<Assets<Mesh>>,
	mut commands: Commands,
	mut evr: EventReader<super::RemeshReady>,
	mut q_chunk_tf: Query<(&super::VoxelChunk, &mut Transform)>,
) {
	for ev in evr.read() {
		let span = info_span!(
				"apply_mesh",
				entity = ?ev.entity,
				positions = ev.buffer.positions.len() as i64,
				indices = ev.buffer.indices.len() as i64
		);
		let _enter = span.enter();
		let t0 = Instant::now();

		let meshes_vec = buffer_to_meshes_per_material(&ev.buffer, None);
		if meshes_vec.is_empty() {
			continue;
		}
		let mesh = meshes_vec.into_iter().next().unwrap();
		trace!("compute_aabb_begin");
		let _ = mesh.compute_aabb();
		let mesh_handle = meshes.add(mesh);
		let mesh_id = mesh_handle.id();

		if let Ok((chunk, mut transform)) = q_chunk_tf.get_mut(ev.entity) {
			let min = super::sample_min(&desc, chunk.chunk_coords);
			transform.translation = Vec3::new(min.x as f32, min.y as f32, min.z as f32);

			commands.entity(ev.entity).insert((
				Mesh3d(mesh_handle.clone()),
				MeshMaterial3d(render_mat.handle.clone()),
			));

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
