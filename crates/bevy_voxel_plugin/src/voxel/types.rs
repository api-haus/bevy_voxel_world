//! Voxel value objects and newtypes

use ilattice::prelude::{IVec3, UVec3};

/// Material identifier for voxels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialId(pub u8);

impl MaterialId {
	/// Air material (empty space)
	pub const AIR: Self = Self(0);
}

/// Chunk coordinates in the grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoords(pub IVec3);

/// Voxel position within a chunk (local coordinates)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalVoxelPos {
	pub x: u32,
	pub y: u32,
	pub z: u32,
}

/// World-space voxel position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldVoxelPos(pub IVec3);

/// Signed distance field value
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SdfValue(pub f32);

impl SdfValue {
	pub fn is_solid(&self) -> bool {
		self.0 <= 0.0
	}
}

/// Dimensions of a voxel chunk
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkDims {
	/// Interior editable voxels (N×N×N)
	pub core: UVec3,
	/// Total samples including 1-voxel apron on each face
	pub sample: UVec3,
}

impl ChunkDims {
	pub fn from_core(core: UVec3) -> Self {
		let sample = core + UVec3::splat(2);
		Self { core, sample }
	}

	pub fn sample_count(&self) -> usize {
		(self.sample.x * self.sample.y * self.sample.z) as usize
	}
}

/// Volume grid configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeConfig {
	/// Dimensions of each chunk (core voxels)
	pub chunk_core_dims: UVec3,
	/// Number of chunks in each dimension
	pub grid_dims: UVec3,
	/// Origin position in world voxel coordinates
	pub origin: WorldVoxelPos,
}

/// CSG operation types for SDF combinations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CsgOperation {
	Union,
	Intersect,
	Subtract,
	SmoothUnion { k: f32 },
	SmoothIntersect { k: f32 },
	SmoothSubtract { k: f32 },
}

/// Edit operation for voxel modification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditOperation {
	Place { material: MaterialId },
	Destroy,
}

/// Sphere primitive for SDF operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SdfSphere {
	pub center: [f32; 3],
	pub radius: f32,
}

/// Box primitive for SDF operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SdfBox {
	pub center: [f32; 3],
	pub half_extents: [f32; 3],
}

/// Result of a mesh generation operation
#[derive(Debug, Clone)]
pub struct MeshData {
	pub positions: Vec<[f32; 3]>,
	pub normals: Vec<[f32; 3]>,
	pub indices: Vec<u32>,
	pub material_ids: Vec<MaterialId>,
}
