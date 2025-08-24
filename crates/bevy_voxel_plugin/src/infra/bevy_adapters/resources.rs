//! Bevy resource adapters

use crate::app::services::{MeshGenerationService, VolumeManagementService};
use crate::infra::{SurfaceNetsMesher, WyRandGenerator};
use crate::voxel::VolumeConfig;
use bevy::prelude::*;
use bevy_prng::WyRand;
use std::sync::{Arc, Mutex};

/// Resource holding the volume configuration
#[derive(Resource, Debug, Clone, Copy)]
pub struct VoxelVolumeConfig(pub VolumeConfig);

impl Default for VoxelVolumeConfig {
	fn default() -> Self {
		Self(VolumeConfig {
			chunk_core_dims: ilattice::prelude::UVec3::new(16, 16, 16),
			grid_dims: ilattice::prelude::UVec3::new(16, 16, 16),
			origin: crate::voxel::WorldVoxelPos(ilattice::prelude::IVec3::ZERO),
		})
	}
}

/// Resource wrapping the volume management service
#[derive(Resource)]
pub struct VolumeServiceResource {
	pub service: Arc<Mutex<VolumeManagementService<WyRandGenerator<WyRand>>>>,
}

/// Resource wrapping the mesh generation service
#[derive(Resource)]
pub struct MeshServiceResource {
	pub service: Arc<Mutex<MeshGenerationService<SurfaceNetsMesher>>>,
}

/// Resource for mesh generation budget
#[derive(Resource, Debug, Clone, Copy)]
pub struct MeshingBudget {
	pub chunks_per_frame: usize,
}

impl Default for MeshingBudget {
	fn default() -> Self {
		Self {
			chunks_per_frame: 4,
		}
	}
}
