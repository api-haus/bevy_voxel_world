use bevy::prelude::*;
use fast_surface_nets::SurfaceNetsBuffer;
use ilattice::prelude::{IVec3, UVec3};

mod apply_mesh;
mod editing;
mod materials;
mod scheduler;
pub mod tracing;
mod volume_spawn;

mod authoring {
	pub(crate) use crate::authoring::components::{CsgOp, SdfBox, SdfSphere};
	pub(crate) use crate::authoring::seed::seed_random_spheres_sdf;
}
use apply_mesh::apply_remeshes;
pub use editing::{EditOp, VoxelEditEvent};
pub use materials::TriplanarExtension;
pub(crate) use materials::VoxelRenderMaterial;
pub(crate) use materials::setup_voxel_material;
pub(crate) use scheduler::{
	RemeshBudget, RemeshQueue, drain_queue_and_spawn_jobs, pump_remesh_results,
};
pub(crate) use tracing::telemetry::VoxelTelemetry;
use tracing::telemetry::{publish_diagnostics, register_voxel_diagnostics, update_telemetry_begin};

#[derive(Resource, Debug, Clone, Copy)]
pub struct VoxelVolumeDesc {
	pub chunk_core_dims: UVec3,
	pub grid_dims: UVec3,
	pub origin_cell: IVec3,
}

impl Default for VoxelVolumeDesc {
	fn default() -> Self {
		Self {
			chunk_core_dims: UVec3::new(16, 16, 16),
			grid_dims: UVec3::new(16, 16, 16),
			origin_cell: IVec3::new(0, 0, 0),
		}
	}
}

#[derive(Component)]
pub struct VoxelVolume {
	pub chunk_core_dims: UVec3,
	pub grid_dims: UVec3,
	pub origin_cell: IVec3,
}

#[derive(Component)]
pub struct VoxelChunk {
	pub chunk_coords: IVec3,
}

// Editing types moved to editing.rs

#[derive(Event)]
pub struct RemeshReady {
	pub entity: Entity,
	pub buffer: SurfaceNetsBuffer,
	pub vertex_colors: Option<Vec<[f32; 4]>>,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum VoxelSet {
	Authoring,
	Editing,
	Schedule,
	ApplyMeshes,
	Physics,
}

pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<VoxelVolumeDesc>()
			.init_resource::<RemeshBudget>()
			.init_resource::<RemeshQueue>()
			.init_resource::<VoxelTelemetry>()
			.init_resource::<scheduler::RemeshInFlightTimings>()
			.add_event::<VoxelEditEvent>()
			.add_event::<RemeshReady>()
			.configure_sets(
				Update,
				(
					VoxelSet::Authoring,
					VoxelSet::Editing,
					VoxelSet::Schedule,
					VoxelSet::ApplyMeshes,
					VoxelSet::Physics,
				),
			)
			.insert_resource({
				use std::sync::mpsc::channel;
				use std::sync::{Arc, Mutex};
				let (tx, rx) = channel();
				scheduler::RemeshResultChannel {
					tx,
					rx: Arc::new(Mutex::new(rx)),
				}
			});

		// Register diagnostics and custom Perf UI entries
		register_voxel_diagnostics(app);

		// Register authoring reflection types for scene I/O
		app
			.register_type::<authoring::CsgOp>()
			.register_type::<authoring::SdfSphere>()
			.register_type::<authoring::SdfBox>();

		app
			.add_systems(
				Startup,
				(
					volume_spawn::spawn_volume_chunks,
					setup_voxel_material,
					authoring::seed_random_spheres_sdf,
				)
					.chain(),
			)
			.add_systems(
				Update,
				(
					editing::apply_edit_events.in_set(VoxelSet::Editing),
					update_telemetry_begin
						.in_set(VoxelSet::Schedule)
						.before(drain_queue_and_spawn_jobs),
					drain_queue_and_spawn_jobs.in_set(VoxelSet::Schedule),
					pump_remesh_results.in_set(VoxelSet::Schedule),
					apply_remeshes.in_set(VoxelSet::ApplyMeshes),
					publish_diagnostics.in_set(VoxelSet::ApplyMeshes),
				),
			);
	}
}
