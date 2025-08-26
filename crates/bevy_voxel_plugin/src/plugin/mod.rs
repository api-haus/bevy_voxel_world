use bevy::prelude::*;
use fast_surface_nets::SurfaceNetsBuffer;
use ilattice::prelude::{IVec3, UVec3};
use std::time::Duration;

mod apply_mesh;
mod editing;
mod rendering_materials;
mod scheduler;
pub mod tracing;
mod volume_spawn;

mod authoring {
	pub(crate) use crate::authoring::components::{CsgOp, SdfBox, SdfSphere};
	pub(crate) use crate::authoring::seed::seed_terrain_noise_sdf;
}
use apply_mesh::apply_remeshes;
pub use editing::{EditOp, VoxelEditEvent};
pub use rendering_materials::TriplanarExtension;
pub(crate) use rendering_materials::{
	VoxelRenderMaterial, // Temporarily disabled for iOS debugging
	init_texture_loading,
	init_voxel_material_when_ready,
};
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

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum VoxelLoadingState {
	#[default]
	Boot,
	Assets,
	World,
	Ready,
}

#[derive(Component)]
pub struct NeedsInitialMesh;

#[derive(Resource, Debug, Clone, Copy)]
struct OriginalFixedTimestep(Duration);

#[derive(Resource, Debug, Clone, Copy)]
struct OriginalRemeshBudget(RemeshBudget);

fn advance_to_assets(mut next: ResMut<NextState<VoxelLoadingState>>) {
	info!(target: "vox", "VoxelLoadingState: advancing to Assets");
	next.set(VoxelLoadingState::Assets);
}

fn advance_to_world_when_material_ready(
	maybe_mat: Option<Res<VoxelRenderMaterial>>,
	mut next: ResMut<NextState<VoxelLoadingState>>,
) {
	if maybe_mat.is_some() {
		info!(target: "vox", "VoxelLoadingState: material ready → World");
		next.set(VoxelLoadingState::World);
	} else {
		trace!(target: "vox", "VoxelLoadingState: waiting for material in Assets");
	}
}

fn enqueue_initial_meshing_on_enter_world(
	mut queue: ResMut<scheduler::RemeshQueue>,
	q_chunks: Query<Entity, With<VoxelChunk>>,
	mut commands: Commands,
) {
	// Clear any leftover queued entities from earlier phases to avoid duplicates
	queue.inner.clear();
	let mut count = 0usize;
	for entity in q_chunks.iter() {
		queue.inner.push_back(entity);
		commands.entity(entity).insert(NeedsInitialMesh);
		count += 1;
	}
	info!(target: "vox", "VoxelLoadingState: entered World, enqueued {} chunks", count);
}

fn advance_to_ready_when_initial_meshing_done(
	q_pending: Query<Entity, With<NeedsInitialMesh>>,
	mut next: ResMut<NextState<VoxelLoadingState>>,
) {
	let pending = q_pending.iter().count();
	if pending == 0 {
		// info!(target: "vox", "VoxelLoadingState: Ready (initial meshing complete)");
		next.set(VoxelLoadingState::Ready);
	} else {
		// trace!(target: "vox", "VoxelLoadingState: waiting for {} chunks", pending);
	}
}

fn suppress_fixed_time(mut fixed: ResMut<Time<Fixed>>) {
	let over = fixed.overstep();
	if over > Duration::ZERO {
		fixed.discard_overstep(over);
		trace!(target: "vox", "Fixed time suppressed: discarded overstep {:?}", over);
	}
}

fn slow_fixed_time_on_loading(
	mut fixed: ResMut<Time<Fixed>>,
	maybe_original: Option<Res<OriginalFixedTimestep>>,
	mut commands: Commands,
) {
	let huge = Duration::from_secs(3600);
	if maybe_original.is_none() {
		let prev = fixed.timestep();
		commands.insert_resource(OriginalFixedTimestep(prev));
	}
	fixed.set_timestep(huge);
	info!(target: "vox", "Fixed time timestep set huge to suppress physics during loading");
}

fn restore_fixed_time_on_ready(
	mut fixed: ResMut<Time<Fixed>>,
	maybe_original: Option<Res<OriginalFixedTimestep>>,
	mut commands: Commands,
) {
	let target = maybe_original
		.as_ref()
		.map(|o| o.0)
		.unwrap_or_else(|| Duration::from_micros(15625));
	fixed.set_timestep(target);
	// Cleanup the saved value to avoid stale restores on next load sequence
	if maybe_original.is_some() {
		commands.remove_resource::<OriginalFixedTimestep>();
	}
	info!(target: "vox", "Fixed time timestep restored to {:?}", target);
}

fn escalate_remesh_budget_on_world(mut budget: ResMut<RemeshBudget>, mut commands: Commands) {
	commands.insert_resource(OriginalRemeshBudget(*budget));
	budget.max_chunks_per_frame = 64;
	budget.time_slice_ms = 8;
	info!(target: "vox", "Remesh budget escalated: max_chunks_per_frame={} time_slice_ms={}", budget.max_chunks_per_frame, budget.time_slice_ms);
}

fn restore_remesh_budget_on_ready(
	mut budget: ResMut<RemeshBudget>,
	maybe_orig: Option<Res<OriginalRemeshBudget>>,
	mut commands: Commands,
) {
	if let Some(orig) = maybe_orig {
		*budget = orig.0;
		commands.remove_resource::<OriginalRemeshBudget>();
		info!(target: "vox", "Remesh budget restored: max_chunks_per_frame={} time_slice_ms={}", budget.max_chunks_per_frame, budget.time_slice_ms);
	}
}

impl Plugin for VoxelPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<VoxelVolumeDesc>()
			.init_resource::<RemeshBudget>()
			.init_resource::<RemeshQueue>()
			.init_resource::<VoxelTelemetry>()
			.init_resource::<scheduler::RemeshInFlightTimings>()
			.init_state::<VoxelLoadingState>()
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
				OnEnter(VoxelLoadingState::Boot),
				(
					slow_fixed_time_on_loading,
					volume_spawn::spawn_volume_chunks,
					authoring::seed_terrain_noise_sdf,
					advance_to_assets,
				)
					.chain(),
			)
			.add_systems(OnEnter(VoxelLoadingState::Assets), init_texture_loading)
			.add_systems(
				Update,
				(
					init_voxel_material_when_ready.run_if(in_state(VoxelLoadingState::Assets)),
					advance_to_world_when_material_ready.run_if(in_state(VoxelLoadingState::Assets)),
					// Suppress FixedUpdate (physics) while loading
					suppress_fixed_time
						.run_if(in_state(VoxelLoadingState::Assets).or(in_state(VoxelLoadingState::World))),
					editing::apply_edit_events
						.in_set(VoxelSet::Editing)
						.run_if(in_state(VoxelLoadingState::Ready)),
				),
			)
			.add_systems(
				OnEnter(VoxelLoadingState::World),
				(
					enqueue_initial_meshing_on_enter_world,
					escalate_remesh_budget_on_world,
				)
					.chain(),
			)
			.add_systems(
				OnEnter(VoxelLoadingState::Ready),
				(restore_fixed_time_on_ready, restore_remesh_budget_on_ready),
			)
			.add_systems(
				Update,
				(
					advance_to_ready_when_initial_meshing_done,
					update_telemetry_begin
						.in_set(VoxelSet::Schedule)
						.before(drain_queue_and_spawn_jobs),
					drain_queue_and_spawn_jobs.in_set(VoxelSet::Schedule),
					pump_remesh_results.in_set(VoxelSet::Schedule),
					apply_remeshes.in_set(VoxelSet::ApplyMeshes),
					publish_diagnostics.in_set(VoxelSet::ApplyMeshes),
				)
					.run_if(in_state(VoxelLoadingState::World).or(in_state(VoxelLoadingState::Ready))),
			);
	}
}
