use bevy::prelude::*;
use fast_surface_nets::SurfaceNetsBuffer;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, info_span, trace};

use crate::voxel_plugin::meshing::surface_nets::select_vertex_materials_from_positions_arrays;
use crate::voxel_plugin::voxels::storage::VoxelStorage;

// Budget for remeshing work per frame
#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct RemeshBudget {
	pub(crate) max_chunks_per_frame: usize,
	pub(crate) time_slice_ms: u64,
}

impl Default for RemeshBudget {
	fn default() -> Self {
		Self {
			max_chunks_per_frame: 4,
			time_slice_ms: 2,
		}
	}
}

// FIFO queue of chunk entities needing remesh
#[derive(Resource, Default)]
pub(crate) struct RemeshQueue {
	pub(crate) inner: VecDeque<Entity>,
}

// Cross-thread channel to forward meshing results back to the main thread
#[derive(Resource)]
pub(crate) struct RemeshResultChannel {
	pub(crate) tx: Sender<super::RemeshReady>,
	pub(crate) rx: Arc<Mutex<Receiver<super::RemeshReady>>>,
}

// Track spawn times of in-flight jobs to estimate mesh worker durations
#[derive(Resource, Default)]
pub(crate) struct RemeshInFlightTimings {
	pub(crate) start_times: HashMap<Entity, Instant>,
}

// Drain the queue within the budget and spawn background meshing jobs
pub(crate) fn drain_queue_and_spawn_jobs(
	budget: Res<RemeshBudget>,
	mut queue: ResMut<RemeshQueue>,
	channels: Res<RemeshResultChannel>,
	q_storage: Query<&VoxelStorage>,
	mut telemetry: ResMut<super::VoxelTelemetry>,
	mut timings: ResMut<RemeshInFlightTimings>,
) {
	let span = info_span!(
		"remesh_drain",
		queue_len = queue.inner.len() as i64,
		max_chunks_per_frame = budget.max_chunks_per_frame as i64,
		time_slice_ms = budget.time_slice_ms as i64
	);
	let _enter = span.enter();

	let start = Instant::now();
	let time_slice = Duration::from_millis(budget.time_slice_ms);

	let mut processed = 0usize;
	while processed < budget.max_chunks_per_frame && start.elapsed() <= time_slice {
		let Some(entity) = queue.inner.pop_front() else {
			break;
		};
		processed += 1;

		let Ok(storage) = q_storage.get(entity) else {
			trace!(target: "vox", "remesh_drain: storage missing for entity={:?}", entity);
			continue;
		};
		let s = storage.dims.sample;
		if !(s.x == 18 && s.y == 18 && s.z == 18) {
			trace!(target: "vox", "remesh_drain: skipping entity={:?} unexpected dims={:?}", entity, s);
			continue;
		}

		// Copy SDF and materials to move into the rayon task
		let sdf: Vec<f32> = storage.sdf.to_vec();
		let mat: Vec<u8> = storage.mat.to_vec();
		let tx = channels.tx.clone();
		let job_span = info_span!("remesh_job_spawn", entity = ?entity, sample_dims = ?s);
		let _job_enter = job_span.enter();
		telemetry.jobs_spawned_frame = telemetry.jobs_spawned_frame.saturating_add(1);
		timings.start_times.insert(entity, Instant::now());

		rayon::spawn(move || {
			let fsn_span = info_span!("fsn_run", entity = ?entity, sample_dims = ?s);
			let _fsn_enter = fsn_span.enter();
			let fsn_start = Instant::now();
			// Early skip when the SDF has no zero-crossing (no surface)
			let mut any_pos = false;
			let mut any_neg = false;
			for &v in &sdf {
				if v <= 0.0 {
					any_neg = true;
				} else {
					any_pos = true;
				}
				if any_pos && any_neg {
					break;
				}
			}
			if !(any_pos && any_neg) {
				trace!(target: "vox", "fsn_early_skip entity={:?}", entity);
				// Signal completion with empty buffer so loading can advance
				let _ = tx.send(super::RemeshReady {
					entity,
					buffer: SurfaceNetsBuffer::default(),
					vertex_colors: None,
				});
				return;
			}

			let mut buffer = SurfaceNetsBuffer::default();
			fast_surface_nets::surface_nets(
				&sdf,
				&fast_surface_nets::ndshape::ConstShape3u32::<18, 18, 18>,
				[0; 3],
				[17, 17, 17],
				&mut buffer,
			);

			let dur_ms = fsn_start.elapsed().as_secs_f32() * 1000.0;
			if buffer.positions.is_empty() {
				trace!(target: "vox", "fsn_empty_output entity={:?} duration_ms={:.3}", entity, dur_ms);
				// Signal completion with empty buffer
				let _ = tx.send(super::RemeshReady {
					entity,
					buffer,
					vertex_colors: None,
				});
				return;
			}
			trace!(target: "vox", "fsn_done entity={:?} positions={} indices={} duration_ms={:.3}", entity, buffer.positions.len(), buffer.indices.len(), dur_ms);

			// Compute per-vertex materials and convert to colors
			let vmat = select_vertex_materials_from_positions_arrays(s, &sdf, &mat, &buffer.positions);
			let vertex_colors: Vec<[f32; 4]> = vmat
				.iter()
				.map(|&m| [(m as f32) / 255.0, 0.0, 0.0, 1.0])
				.collect();

			let _ = tx.send(super::RemeshReady {
				entity,
				buffer,
				vertex_colors: Some(vertex_colors),
			});
		});
	}
	debug!(target: "vox", "remesh_drain: processed={} queue_remaining={}", processed, queue.inner.len());
}

// Pump results from background tasks into the Bevy event queue
pub(crate) fn pump_remesh_results(
	channels: Res<RemeshResultChannel>,
	mut evw: EventWriter<super::RemeshReady>,
	mut telemetry: ResMut<super::VoxelTelemetry>,
	mut timings: ResMut<RemeshInFlightTimings>,
	maybe_render_mat: Option<Res<super::VoxelRenderMaterial>>,
) {
	// Defer draining until voxel material is ready to avoid dropping results before meshes can be applied
	if maybe_render_mat.is_none() {
		trace!(target: "vox", "pump_remesh_results: material not ready; deferring pump");
		return;
	}
	let mut pumped = 0usize;
	loop {
		let Ok(guard) = channels.rx.lock() else { break };
		match guard.try_recv() {
			Ok(result) => {
				drop(guard);
				if let Some(t0) = timings.start_times.remove(&result.entity) {
					let dt_ms = t0.elapsed().as_secs_f32() * 1000.0;
					telemetry.mesh_time_ms_frame += dt_ms;
				}
				telemetry.jobs_completed_frame = telemetry.jobs_completed_frame.saturating_add(1);
				evw.write(result);
				pumped += 1;
			}
			Err(std::sync::mpsc::TryRecvError::Empty) => break,
			Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
		}
	}
	if pumped > 0 {
		debug!(target: "vox", "pump_remesh_results: pumped={} events", pumped);
	}
}
