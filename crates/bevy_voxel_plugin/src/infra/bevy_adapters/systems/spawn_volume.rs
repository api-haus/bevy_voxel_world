//! System for spawning voxel volume entities

use crate::infra::bevy_adapters::{
	VolumeServiceResource, VoxelChunkComponent, VoxelChunkData, VoxelVolumeMarker,
};
use bevy::prelude::*;

pub fn spawn_volume_system(mut commands: Commands, volume_service: Res<VolumeServiceResource>) {
	let service = volume_service.service.lock().unwrap();
	let volume = &service.volume;

	// Spawn parent volume entity
	let volume_entity = commands
		.spawn((
			Name::new("VoxelVolume"),
			VoxelVolumeMarker,
			Transform::default(),
			GlobalTransform::default(),
			Visibility::default(),
			InheritedVisibility::default(),
			ViewVisibility::default(),
		))
		.id();

	// Spawn chunk entities
	for chunk in &volume.chunks {
		let chunk_entity = commands
			.spawn((
				Name::new(format!(
					"Chunk_{}_{}_{}",
					chunk.coords.0.x, chunk.coords.0.y, chunk.coords.0.z
				)),
				VoxelChunkComponent {
					coords: chunk.coords,
				},
				VoxelChunkData {
					chunk: chunk.clone(),
				},
				Transform::default(),
				GlobalTransform::default(),
				Visibility::default(),
				InheritedVisibility::default(),
				ViewVisibility::default(),
			))
			.id();

		commands.entity(volume_entity).add_child(chunk_entity);
	}
}
