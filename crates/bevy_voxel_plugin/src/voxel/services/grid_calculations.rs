//! Grid calculation services

use crate::voxel::types::*;

use ilattice::prelude::{Extent, IVec3};

/// Calculate world position from chunk coordinates and local position
pub fn world_position_from_local(
	chunk_coords: ChunkCoords,
	local_pos: LocalVoxelPos,
	volume_config: &VolumeConfig,
) -> WorldVoxelPos {
	let chunk_offset = IVec3::new(
		chunk_coords.0.x * volume_config.chunk_core_dims.x as i32,
		chunk_coords.0.y * volume_config.chunk_core_dims.y as i32,
		chunk_coords.0.z * volume_config.chunk_core_dims.z as i32,
	);

	let world_pos = volume_config.origin.0
		+ chunk_offset
		+ IVec3::new(
			local_pos.x as i32 - 1, // Account for apron
			local_pos.y as i32 - 1,
			local_pos.z as i32 - 1,
		);

	WorldVoxelPos(world_pos)
}

/// Calculate chunk coordinates from world position
pub fn chunk_coords_from_world(
	world_pos: WorldVoxelPos,
	volume_config: &VolumeConfig,
) -> ChunkCoords {
	let relative = world_pos.0 - volume_config.origin.0;
	let coords = IVec3::new(
		relative
			.x
			.div_euclid(volume_config.chunk_core_dims.x as i32),
		relative
			.y
			.div_euclid(volume_config.chunk_core_dims.y as i32),
		relative
			.z
			.div_euclid(volume_config.chunk_core_dims.z as i32),
	);
	ChunkCoords(coords)
}

/// Calculate the interior extent of a chunk (core voxels)
pub fn chunk_core_extent(chunk_coords: ChunkCoords, volume_config: &VolumeConfig) -> Extent<IVec3> {
	let offset = IVec3::new(
		chunk_coords.0.x * volume_config.chunk_core_dims.x as i32,
		chunk_coords.0.y * volume_config.chunk_core_dims.y as i32,
		chunk_coords.0.z * volume_config.chunk_core_dims.z as i32,
	);

	let min = volume_config.origin.0 + offset;
	let shape = IVec3::new(
		volume_config.chunk_core_dims.x as i32,
		volume_config.chunk_core_dims.y as i32,
		volume_config.chunk_core_dims.z as i32,
	);

	Extent::from_min_and_shape(min, shape)
}

/// Calculate the sample extent of a chunk (including apron)
pub fn chunk_sample_extent(
	chunk_coords: ChunkCoords,
	volume_config: &VolumeConfig,
) -> Extent<IVec3> {
	let core_extent = chunk_core_extent(chunk_coords, volume_config);
	let min = core_extent.minimum - IVec3::ONE;
	let dims = ChunkDims::from_core(volume_config.chunk_core_dims);
	let shape = IVec3::new(
		dims.sample.x as i32,
		dims.sample.y as i32,
		dims.sample.z as i32,
	);

	Extent::from_min_and_shape(min, shape)
}
