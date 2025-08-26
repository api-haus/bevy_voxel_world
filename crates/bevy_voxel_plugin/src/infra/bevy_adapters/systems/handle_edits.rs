//! System for handling voxel edit events

use crate::app::commands::EditVoxelsCommand;

use crate::infra::bevy_adapters::{
	MeshServiceResource, NeedsRemesh, VolumeServiceResource, VoxelChunkComponent, VoxelChunkData,
	VoxelEditEvent,
};

use bevy::prelude::*;

pub fn handle_edit_events_system(
	mut commands: Commands,
	mut events: EventReader<VoxelEditEvent>,
	volume_service: Res<VolumeServiceResource>,
	mesh_service: Res<MeshServiceResource>,
	mut chunks: Query<(Entity, &VoxelChunkComponent, &mut VoxelChunkData)>,
) {
	for event in events.read() {
		// Execute edit command
		let command = EditVoxelsCommand {
			sphere: event.sphere,
			operation: event.operation,
		};

		let modified_coords = {
			let mut service = volume_service.service.lock().unwrap();
			service.edit_voxels(command)
		};

		// Update chunk data and mark for remeshing
		for (entity, chunk_comp, mut chunk_data) in chunks.iter_mut() {
			if modified_coords.contains(&chunk_comp.coords) {
				// Get updated chunk from service
				let service = volume_service.service.lock().unwrap();

				if let Some(updated_chunk) = service.volume.chunk_at(chunk_comp.coords) {
					chunk_data.chunk = updated_chunk.clone();
					commands.entity(entity).insert(NeedsRemesh);
				}
			}
		}

		// Queue chunks for meshing
		{
			let mut mesh_service = mesh_service.service.lock().unwrap();
			mesh_service.queue_chunks(modified_coords);
		}
	}
}
