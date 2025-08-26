//! Voxel entities

use crate::voxel::types::*;

use ilattice::prelude::IVec3;

/// A voxel chunk containing SDF and material data
#[derive(Debug, Clone)]
pub struct VoxelChunk {
	pub coords: ChunkCoords,
	pub dims: ChunkDims,
	sdf_data: Box<[f32]>,
	material_data: Box<[u8]>,
}

impl VoxelChunk {
	/// Create a new chunk filled with default values
	pub fn new(coords: ChunkCoords, dims: ChunkDims) -> Self {
		let count = dims.sample_count();
		let sdf_data = vec![f32::INFINITY; count].into_boxed_slice();
		let material_data = vec![MaterialId::AIR.0; count].into_boxed_slice();

		Self {
			coords,
			dims,
			sdf_data,
			material_data,
		}
	}

	/// Get the linear index for a local position
	#[inline]
	pub fn index_of(&self, pos: LocalVoxelPos) -> usize {
		let xy = self.dims.sample.x * self.dims.sample.y;
		(pos.z * xy + pos.y * self.dims.sample.x + pos.x) as usize
	}

	/// Get SDF value at position
	pub fn sdf_at(&self, pos: LocalVoxelPos) -> SdfValue {
		SdfValue(self.sdf_data[self.index_of(pos)])
	}

	/// Get material at position
	pub fn material_at(&self, pos: LocalVoxelPos) -> MaterialId {
		MaterialId(self.material_data[self.index_of(pos)])
	}

	/// Set SDF value at position
	pub fn set_sdf(&mut self, pos: LocalVoxelPos, value: SdfValue) {
		let idx = self.index_of(pos);
		self.sdf_data[idx] = value.0;
	}

	/// Set material at position
	pub fn set_material(&mut self, pos: LocalVoxelPos, material: MaterialId) {
		let idx = self.index_of(pos);
		self.material_data[idx] = material.0;
	}

	/// Get raw SDF data (for meshing)
	pub fn sdf_data(&self) -> &[f32] {
		&self.sdf_data
	}

	/// Get raw material data (for meshing)
	pub fn material_data(&self) -> &[u8] {
		&self.material_data
	}

	/// Check if chunk has any solid voxels
	pub fn has_solids(&self) -> bool {
		self.sdf_data.iter().any(|&v| v <= 0.0)
	}

	/// Check if chunk has a surface (both positive and negative SDF values)
	pub fn has_surface(&self) -> bool {
		let mut has_positive = false;
		let mut has_negative = false;

		for &sdf in self.sdf_data.iter() {
			if sdf > 0.0 {
				has_positive = true;
			} else if sdf <= 0.0 {
				has_negative = true;
			}

			if has_positive && has_negative {
				return true;
			}
		}

		false
	}
}

/// A voxel volume containing multiple chunks
#[derive(Debug, Clone)]
pub struct VoxelVolume {
	pub config: VolumeConfig,
	pub chunks: Vec<VoxelChunk>,
}

impl VoxelVolume {
	/// Create a new empty volume
	pub fn new(config: VolumeConfig) -> Self {
		let chunk_count = (config.grid_dims.x * config.grid_dims.y * config.grid_dims.z) as usize;
		let mut chunks = Vec::with_capacity(chunk_count);

		let dims = ChunkDims::from_core(config.chunk_core_dims);

		for z in 0..config.grid_dims.z as i32 {
			for y in 0..config.grid_dims.y as i32 {
				for x in 0..config.grid_dims.x as i32 {
					let coords = ChunkCoords(IVec3::new(x, y, z));
					chunks.push(VoxelChunk::new(coords, dims));
				}
			}
		}

		Self { config, chunks }
	}

	/// Get chunk at grid coordinates
	pub fn chunk_at(&self, coords: ChunkCoords) -> Option<&VoxelChunk> {
		let idx = self.chunk_index(coords)?;
		self.chunks.get(idx)
	}

	/// Get mutable chunk at grid coordinates
	pub fn chunk_at_mut(&mut self, coords: ChunkCoords) -> Option<&mut VoxelChunk> {
		let idx = self.chunk_index(coords)?;
		self.chunks.get_mut(idx)
	}

	/// Calculate linear index for chunk coordinates
	fn chunk_index(&self, coords: ChunkCoords) -> Option<usize> {
		let c = coords.0;

		if c.x < 0
			|| c.y < 0
			|| c.z < 0
			|| c.x >= self.config.grid_dims.x as i32
			|| c.y >= self.config.grid_dims.y as i32
			|| c.z >= self.config.grid_dims.z as i32
		{
			return None;
		}

		let idx = (c.z as usize * (self.config.grid_dims.y * self.config.grid_dims.x) as usize)
			+ (c.y as usize * self.config.grid_dims.x as usize)
			+ c.x as usize;
		Some(idx)
	}
}
