//! System for processing mesh generation queue

use crate::infra::bevy_adapters::{
	MeshReady, MeshServiceResource, MeshingBudget, VolumeServiceResource,
};
use crate::voxel::MaterialId;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh};
use bevy::render::render_resource::PrimitiveTopology;

pub fn process_meshing_queue_system(
	mut meshes: ResMut<Assets<Mesh>>,
	mut events: EventWriter<MeshReady>,
	volume_service: Res<VolumeServiceResource>,
	mesh_service: Res<MeshServiceResource>,
	budget: Res<MeshingBudget>,
	chunks: Query<(Entity, &crate::infra::bevy_adapters::VoxelChunkComponent)>,
) {
	let results = {
		let volume = &volume_service.service.lock().unwrap().volume;
		let mut mesh_service = mesh_service.service.lock().unwrap();
		mesh_service.process_queue(volume, budget.chunks_per_frame)
	};

	for (coords, mesh_data) in results {
		if let Some(mesh_data) = mesh_data {
			// Find entity with these coords
			if let Some((entity, _)) = chunks.iter().find(|(_, comp)| comp.coords == coords) {
				// Convert to Bevy mesh
				let mut mesh = Mesh::new(
					PrimitiveTopology::TriangleList,
					RenderAssetUsages::default(),
				);
				mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, mesh_data.positions);

				if !mesh_data.normals.is_empty() {
					mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_data.normals);
				}

				// Convert material IDs to vertex colors
				let colors: Vec<[f32; 4]> = mesh_data
					.material_ids
					.iter()
					.map(|mat| material_to_color(*mat))
					.collect();
				mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

				mesh.insert_indices(Indices::U32(mesh_data.indices));

				let handle = meshes.add(mesh);
				events.write(MeshReady {
					entity,
					mesh: handle,
				});
			}
		}
	}
}

fn material_to_color(mat: MaterialId) -> [f32; 4] {
	// Simple material to color mapping
	let value = mat.0 as f32 / 255.0;
	[value, 0.0, 0.0, 1.0]
}
