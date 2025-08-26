//! Bevy event adapters

use crate::voxel::{EditOperation, SdfSphere};

use bevy::prelude::*;

/// Event for voxel editing
#[derive(Event, Clone, Debug)]
pub struct VoxelEditEvent {
	pub sphere: SdfSphere,
	pub operation: EditOperation,
}

/// Event signaling mesh generation is complete
#[derive(Event)]
pub struct MeshReady {
	pub entity: Entity,
	pub mesh: Handle<Mesh>,
}
