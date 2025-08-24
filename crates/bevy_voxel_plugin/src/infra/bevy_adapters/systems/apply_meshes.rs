//! System for applying generated meshes to entities

use crate::infra::bevy_adapters::{MeshReady, NeedsRemesh, VoxelChunkComponent};
use crate::voxel::services;
use bevy::prelude::*;

pub fn apply_generated_meshes_system(
	mut commands: Commands,
	mut events: EventReader<MeshReady>,
	materials: Res<super::super::materials::VoxelMaterialResource>,
	volume_config: Res<super::super::VoxelVolumeConfig>,
	mut chunks: Query<(&VoxelChunkComponent, &mut Transform), With<NeedsRemesh>>,
) {
	for event in events.read() {
		if let Ok((chunk_comp, mut transform)) = chunks.get_mut(event.entity) {
			// Set chunk position
			let extent = services::chunk_sample_extent(chunk_comp.coords, &volume_config.0);
			transform.translation = Vec3::new(
				extent.minimum.x as f32,
				extent.minimum.y as f32,
				extent.minimum.z as f32,
			);

			// Apply mesh and material
			commands
				.entity(event.entity)
				.insert((
					Mesh3d(event.mesh.clone()),
					MeshMaterial3d(materials.handle.clone()),
				))
				.remove::<NeedsRemesh>();
		}
	}
}
