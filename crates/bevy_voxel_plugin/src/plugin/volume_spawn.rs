use bevy::prelude::*;

use crate::voxel_plugin::voxels::storage::VoxelStorage;

pub(crate) fn spawn_volume_chunks(mut commands: Commands, desc: Res<super::VoxelVolumeDesc>) {
	let volume_entity = commands
		.spawn((
			Name::new("VoxelVolume"),
			super::VoxelVolume {
				chunk_core_dims: desc.chunk_core_dims,
				grid_dims: desc.grid_dims,
				origin_cell: desc.origin_cell,
			},
			Transform::default(),
			GlobalTransform::default(),
			Visibility::Visible,
			InheritedVisibility::VISIBLE,
			ViewVisibility::default(),
		))
		.id();

	let grid = desc.grid_dims;
	for z in 0..grid.z as i32 {
		for y in 0..grid.y as i32 {
			for x in 0..grid.x as i32 {
				let chunk_coords = ilattice::prelude::IVec3::new(x, y, z);
				let storage = VoxelStorage::new(desc.chunk_core_dims);

				let child = commands
					.spawn((
						Name::new(format!("VoxelChunk {:?}", chunk_coords)),
						super::VoxelChunk { chunk_coords },
						storage,
						Transform::default(),
						GlobalTransform::default(),
						Visibility::Visible,
						InheritedVisibility::VISIBLE,
						ViewVisibility::default(),
					))
					.id();
				commands.entity(volume_entity).add_child(child);
			}
		}
	}
}
