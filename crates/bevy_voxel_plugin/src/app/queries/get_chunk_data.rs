//! Get chunk data query

use crate::voxel::{ChunkCoords, VoxelChunk, VoxelVolume};

/// Query to get chunk data
#[derive(Debug, Clone)]
pub struct GetChunkDataQuery {
	pub chunk_coords: ChunkCoords,
}

/// Execute the get chunk data query
pub fn execute_get_chunk_data(
	query: GetChunkDataQuery,
	volume: &VoxelVolume,
) -> Option<&VoxelChunk> {
	volume.chunk_at(query.chunk_coords)
}
