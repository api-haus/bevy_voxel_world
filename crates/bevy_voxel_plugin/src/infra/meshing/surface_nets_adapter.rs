//! Surface nets meshing adapter

use crate::voxel::{ports::Mesher, MeshData, VoxelChunk};
use fast_surface_nets::{ndshape::ConstShape3u32, surface_nets, SurfaceNetsBuffer};

/// Surface nets mesher implementation
pub struct SurfaceNetsMesher;

impl Mesher for SurfaceNetsMesher {
	type Error = MeshingError;

	fn generate_mesh(&self, chunk: &VoxelChunk) -> Result<Option<MeshData>, Self::Error> {
		// Check if chunk has a surface
		if !chunk.has_surface() {
			return Ok(None);
		}

		let dims = chunk.dims.sample;

		// Currently only support specific chunk sizes
		if dims.x == 18 && dims.y == 18 && dims.z == 18 {
			generate_mesh_18(chunk)
		} else if dims.x == 34 && dims.y == 34 && dims.z == 34 {
			generate_mesh_34(chunk)
		} else {
			Err(MeshingError::UnsupportedChunkSize)
		}
	}
}

fn generate_mesh_18(chunk: &VoxelChunk) -> Result<Option<MeshData>, MeshingError> {
	let mut buffer = SurfaceNetsBuffer::default();

	surface_nets(
		chunk.sdf_data(),
		&ConstShape3u32::<18, 18, 18>,
		[0; 3],
		[17, 17, 17],
		&mut buffer,
	);

	if buffer.positions.is_empty() {
		return Ok(None);
	}

	// Select materials for vertices
	let material_ids = super::material_selection::select_vertex_materials(chunk, &buffer.positions);

	Ok(Some(MeshData {
		positions: buffer.positions,
		normals: buffer.normals,
		indices: buffer.indices,
		material_ids,
	}))
}

fn generate_mesh_34(chunk: &VoxelChunk) -> Result<Option<MeshData>, MeshingError> {
	let mut buffer = SurfaceNetsBuffer::default();

	surface_nets(
		chunk.sdf_data(),
		&ConstShape3u32::<34, 34, 34>,
		[0; 3],
		[33, 33, 33],
		&mut buffer,
	);

	if buffer.positions.is_empty() {
		return Ok(None);
	}

	let material_ids = super::material_selection::select_vertex_materials(chunk, &buffer.positions);

	Ok(Some(MeshData {
		positions: buffer.positions,
		normals: buffer.normals,
		indices: buffer.indices,
		material_ids,
	}))
}

#[derive(Debug, thiserror::Error)]
pub enum MeshingError {
	#[error("Unsupported chunk size")]
	UnsupportedChunkSize,
}
