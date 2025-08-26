//! Bevy component adapters

use crate::voxel::{ChunkCoords, VoxelChunk as DomainChunk};

use bevy::prelude::*;

/// Bevy component marking a voxel volume entity
#[derive(Component)]
pub struct VoxelVolumeMarker;

/// Bevy component for chunk metadata
#[derive(Component)]
pub struct VoxelChunkComponent {
	pub coords: ChunkCoords,
}

/// Bevy component storing chunk data
#[derive(Component)]
pub struct VoxelChunkData {
	pub chunk: DomainChunk,
}

/// Bevy component marking chunks that need remeshing
#[derive(Component)]
pub struct NeedsRemesh;

/// Bevy component for SDF authoring shapes
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq)]
#[reflect(Component)]
pub struct SdfSphere {
	pub radius: f32,
	pub material: u8,
	pub priority: u8,
}

#[derive(Component, Reflect, Default, Clone, Copy, PartialEq)]
#[reflect(Component)]
pub struct SdfBox {
	pub half_extents: Vec3,
	pub material: u8,
	pub priority: u8,
}
