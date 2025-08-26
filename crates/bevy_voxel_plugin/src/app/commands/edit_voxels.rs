//! Edit voxels command

use crate::voxel::{EditOperation, SdfSphere, VoxelVolume, services};

/// Command to edit voxels using a sphere
#[derive(Debug, Clone)]
pub struct EditVoxelsCommand {
	pub sphere: SdfSphere,
	pub operation: EditOperation,
}

/// Result of edit operation
#[derive(Debug, Clone)]
pub struct EditResult {
	pub modified_chunks: Vec<ilattice::prelude::IVec3>,
}

/// Execute the edit voxels command
pub fn execute_edit_voxels(command: EditVoxelsCommand, volume: &mut VoxelVolume) -> EditResult {
	let mut modified_chunks = Vec::new();

	for chunk in volume.chunks.iter_mut() {
		let extent = services::chunk_sample_extent(chunk.coords, &volume.config);
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

		if services::sphere_intersects_aabb(&command.sphere, aabb_min, aabb_max) {
			let modified = services::apply_sphere_edit(chunk, &command.sphere, command.operation, extent);

			if modified {
				modified_chunks.push(chunk.coords.0);
			}
		}
	}

	EditResult { modified_chunks }
}
