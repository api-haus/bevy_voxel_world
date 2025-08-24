//! Domain ports (traits) for external dependencies

use crate::voxel::entities::*;
use crate::voxel::types::*;

/// Port for mesh generation
pub trait Mesher {
	type Error: std::error::Error;

	/// Generate mesh from voxel chunk
	fn generate_mesh(&self, chunk: &VoxelChunk) -> Result<Option<MeshData>, Self::Error>;
}

/// Port for random number generation
pub trait RandomGenerator {
	/// Generate random integer in range [min, max)
	fn random_range_i32(&mut self, min: i32, max: i32) -> i32;

	/// Generate random float in range [min, max)
	fn random_range_f32(&mut self, min: f32, max: f32) -> f32;
}

/// Port for chunk persistence
pub trait ChunkRepository {
	type Error: std::error::Error;

	/// Save chunk data
	fn save_chunk(&self, chunk: &VoxelChunk) -> Result<(), Self::Error>;

	/// Load chunk data
	fn load_chunk(&self, coords: ChunkCoords) -> Result<Option<VoxelChunk>, Self::Error>;
}
