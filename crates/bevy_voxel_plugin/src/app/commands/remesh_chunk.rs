//! Remesh chunk command

use crate::voxel::{ChunkCoords, MeshData, VoxelVolume, ports::Mesher};

/// Command to remesh a chunk
#[derive(Debug, Clone)]
pub struct RemeshChunkCommand {
	pub chunk_coords: ChunkCoords,
}

/// Result of remesh operation
#[derive(Debug)]
pub struct RemeshResult {
	pub mesh_data: Option<MeshData>,
}

/// Execute the remesh chunk command
pub fn execute_remesh_chunk<M: Mesher>(
	command: RemeshChunkCommand,
	volume: &VoxelVolume,
	mesher: &M,
) -> Result<RemeshResult, M::Error> {
	let chunk = volume
		.chunk_at(command.chunk_coords)
		.unwrap_or_else(|| panic!("Chunk not found at {:?}", command.chunk_coords));

	let mesh_data = mesher.generate_mesh(chunk)?;

	Ok(RemeshResult { mesh_data })
}
