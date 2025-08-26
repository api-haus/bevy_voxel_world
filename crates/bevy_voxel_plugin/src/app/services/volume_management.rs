//! Volume management service

use crate::app::commands::{CreateVolumeCommand, EditVoxelsCommand, SeedVolumeCommand};

use crate::voxel::{ChunkCoords, VolumeConfig, VoxelVolume, ports::*};

/// Service for managing voxel volumes
pub struct VolumeManagementService<R: RandomGenerator> {
	pub volume: VoxelVolume,
	pub rng: R,
}

impl<R: RandomGenerator> VolumeManagementService<R> {
	/// Create a new volume management service
	pub fn new(config: VolumeConfig, rng: R) -> Self {
		let command = CreateVolumeCommand { config };
		let volume = super::super::commands::execute_create_volume(command);
		Self { volume, rng }
	}

	/// Edit voxels in the volume
	pub fn edit_voxels(&mut self, command: EditVoxelsCommand) -> Vec<ChunkCoords> {
		let result = super::super::commands::execute_edit_voxels(command, &mut self.volume);
		result
			.modified_chunks
			.into_iter()
			.map(ChunkCoords)
			.collect()
	}

	/// Seed the volume with random content
	pub fn seed_volume(&mut self, command: SeedVolumeCommand) -> usize {
		let result =
			super::super::commands::execute_seed_volume(command, &mut self.volume, &mut self.rng);
		result.chunks_with_content
	}

	/// Get chunks that need meshing
	pub fn chunks_needing_mesh(&self) -> Vec<ChunkCoords> {
		use crate::app::queries::{FindChunksNeedingMeshQuery, execute_find_chunks_needing_mesh};
		execute_find_chunks_needing_mesh(FindChunksNeedingMeshQuery, &self.volume)
	}
}
