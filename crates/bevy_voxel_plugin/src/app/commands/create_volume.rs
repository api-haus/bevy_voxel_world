//! Create volume command

use crate::voxel::{VolumeConfig, VoxelVolume};

/// Command to create a new voxel volume
#[derive(Debug, Clone)]
pub struct CreateVolumeCommand {
	pub config: VolumeConfig,
}

/// Execute the create volume command
pub fn execute_create_volume(command: CreateVolumeCommand) -> VoxelVolume {
	VoxelVolume::new(command.config)
}
