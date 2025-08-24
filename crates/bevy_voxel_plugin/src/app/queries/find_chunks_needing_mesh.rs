//! Find chunks that need meshing

use crate::voxel::{ChunkCoords, VoxelVolume};

/// Query to find chunks that have surfaces and need meshing
#[derive(Debug, Clone)]
pub struct FindChunksNeedingMeshQuery;

/// Execute the find chunks needing mesh query
pub fn execute_find_chunks_needing_mesh(
	_query: FindChunksNeedingMeshQuery,
	volume: &VoxelVolume,
) -> Vec<ChunkCoords> {
	volume
		.chunks
		.iter()
		.filter(|chunk| chunk.has_surface())
		.map(|chunk| chunk.coords)
		.collect()
}
