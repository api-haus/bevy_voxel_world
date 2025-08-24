//! Remesh chunk command

use crate::voxel::{ports::Mesher, ChunkCoords, MeshData, VoxelVolume};

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
	let chunk = volume.chunk_at(command.chunk_coords).ok_or_else(|| {
		// This is a bit hacky since we don't have a concrete error type
		// In a real implementation, we'd have a proper error type
		use std::fmt;
		#[derive(Debug)]
		struct ChunkNotFound;
		impl fmt::Display for ChunkNotFound {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				write!(f, "Chunk not found")
			}
		}
		impl std::error::Error for ChunkNotFound {}
		// We can't return this directly, so we'll panic for now
		panic!("Chunk not found at {:?}", command.chunk_coords)
	})?;

	let mesh_data = mesher.generate_mesh(chunk)?;

	Ok(RemeshResult { mesh_data })
}
