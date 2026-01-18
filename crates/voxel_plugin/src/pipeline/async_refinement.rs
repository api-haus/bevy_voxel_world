//! Async Refinement Pipeline
//!
//! Combines octree refinement with mesh generation in a single async task.
//! This moves the CPU-intensive `refine()` call off the main thread.
//!
//! # Flow
//!
//! ```text
//! Main Thread                       Async (rayon)
//! ┌────────────────┐
//! │ Capture inputs │
//! │ (viewer, leaves)│
//! └───────┬────────┘
//!         │ start()
//!         ▼
//!                                  ┌───────────────┐
//!                                  │ refine()      │
//!                                  │ (distance,    │
//!                                  │  sort,        │
//!                                  │  neighbors)   │
//!                                  └───────┬───────┘
//!                                          │
//!                                          ▼
//!                                  ┌───────────────┐
//!                                  │ process_      │
//!                                  │ transitions() │
//!                                  │ (sample,mesh) │
//!                                  └───────┬───────┘
//!                                          │
//! ┌────────────────┐                       │
//! │ poll_results() │◄──────────────────────┘
//! │ - Apply leaves │
//! │ - Queue entities│
//! └────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let mut pipeline = AsyncRefinementPipeline::new();
//!
//! // Start (non-blocking) - refinement runs async
//! pipeline.start(RefinementRequest {
//!     world_id,
//!     viewer_pos,
//!     leaves: world.leaves.clone(),
//!     config: config.clone(),
//!     budget: RefinementBudget::DEFAULT,
//!     sampler,
//! });
//!
//! // Poll each frame
//! if let Some(result) = pipeline.poll_results() {
//!     // Apply transitions to world.leaves
//!     for node in &result.nodes_to_remove {
//!         world.leaves.remove(node);
//!     }
//!     for node in &result.nodes_to_add {
//!         world.leaves.insert(*node);
//!     }
//!     // Queue entity ops (processed with time budget elsewhere)
//!     entity_queue.queue_despawns(result.nodes_to_remove);
//!     entity_queue.queue_spawns(result.ready_chunks);
//! }
//! ```

use std::collections::HashSet;

use crossbeam_channel::{self as channel, Receiver, TryRecvError};
use glam::DVec3;

use super::process::process_transitions;
use super::types::{ReadyChunk, VolumeSampler};
use crate::octree::{refine, OctreeConfig, OctreeNode, RefinementBudget, RefinementInput};
use crate::world::WorldId;

/// Request to start async refinement.
pub struct RefinementRequest<S: VolumeSampler> {
	/// World ID for the chunks.
	pub world_id: WorldId,
	/// Viewer position in world space.
	pub viewer_pos: DVec3,
	/// Current leaf set (snapshot).
	pub leaves: HashSet<OctreeNode>,
	/// Octree configuration.
	pub config: OctreeConfig,
	/// Refinement budget (count limits, neighbor rules).
	pub budget: RefinementBudget,
	/// Volume sampler for mesh generation.
	pub sampler: S,
}

/// A completed transition ready for atomic presentation.
///
/// Must be applied atomically: despawn nodes_to_remove AND spawn ready_chunks
/// in the same frame to avoid visual pops.
#[derive(Debug)]
pub struct CompletedTransition {
	/// The group key identifying this transition (parent node).
	pub group_key: OctreeNode,
	/// Whether this is a collapse (merge) or subdivide transition.
	pub is_collapse: bool,
	/// Nodes to despawn (parent for subdivide, children for merge).
	pub nodes_to_remove: Vec<OctreeNode>,
	/// Nodes to add to leaves set.
	pub nodes_to_add: Vec<OctreeNode>,
	/// Ready chunks to spawn (children for subdivide, parent for merge).
	pub ready_chunks: Vec<ReadyChunk>,
}

/// Result from async refinement pipeline.
pub struct RefinementResult {
	/// World ID.
	pub world_id: WorldId,
	/// Completed transitions, each to be applied atomically.
	pub transitions: Vec<CompletedTransition>,
	/// Stats from refinement.
	pub stats: RefinementStats,
}

/// Statistics from async refinement.
#[derive(Debug, Clone, Copy, Default)]
pub struct RefinementStats {
	/// Number of subdivisions performed.
	pub subdivisions: usize,
	/// Number of collapses performed.
	pub collapses: usize,
	/// Number of neighbor enforcement subdivisions.
	pub neighbor_subdivisions: usize,
	/// Refinement time in microseconds.
	pub refine_us: u64,
	/// Mesh generation time in microseconds.
	pub mesh_us: u64,
}

/// Non-blocking async refinement + mesh pipeline.
///
/// Runs refinement and mesh generation on rayon's thread pool.
pub struct AsyncRefinementPipeline {
	/// Receiver for pending result.
	receiver: Option<Receiver<RefinementResult>>,
}

impl AsyncRefinementPipeline {
	/// Create a new pipeline.
	pub fn new() -> Self {
		Self { receiver: None }
	}

	/// Check if a task is running.
	pub fn is_busy(&self) -> bool {
		self.receiver.is_some()
	}

	/// Start async refinement + mesh generation.
	///
	/// Returns `true` if started, `false` if already busy.
	pub fn start<S: VolumeSampler + Clone + Send + 'static>(&mut self, request: RefinementRequest<S>) -> bool {
		if self.is_busy() {
			return false;
		}

		let (sender, receiver) = channel::bounded(1);
		self.receiver = Some(receiver);

		// Spawn on rayon thread pool
		rayon::spawn(move || {
			let result = run_refinement_pipeline(request);
			// Ignore send error (receiver dropped = cancelled)
			let _ = sender.send(result);
		});

		true
	}

	/// Poll for results (non-blocking).
	///
	/// Returns `Some(result)` when complete, `None` if still running.
	pub fn poll_results(&mut self) -> Option<RefinementResult> {
		let receiver = self.receiver.as_ref()?;

		match receiver.try_recv() {
			Ok(result) => {
				self.receiver = None;
				Some(result)
			}
			Err(TryRecvError::Empty) => None,
			Err(TryRecvError::Disconnected) => {
				self.receiver = None;
				None
			}
		}
	}

	/// Cancel pending task.
	pub fn cancel(&mut self) {
		self.receiver = None;
	}
}

impl Default for AsyncRefinementPipeline {
	fn default() -> Self {
		Self::new()
	}
}

/// Run the full refinement + mesh pipeline (called on worker thread).
#[cfg_attr(feature = "tracing", tracing::instrument(skip_all, name = "pipeline::run_refinement_pipeline"))]
fn run_refinement_pipeline<S: VolumeSampler>(request: RefinementRequest<S>) -> RefinementResult {
	use std::collections::HashMap;
	use web_time::Instant;

	let RefinementRequest {
		world_id,
		viewer_pos,
		leaves,
		config,
		budget,
		sampler,
	} = request;

	// Stage 1: Refinement (distance calc, sort, neighbor enforcement)
	let refine_start = Instant::now();
	let refine_output = {
		#[cfg(feature = "tracing")]
		let _span = tracing::info_span!("refinement_stage").entered();
		let refine_input = RefinementInput {
			viewer_pos,
			config: config.clone(),
			prev_leaves: leaves.clone(),
			budget,
		};
		refine(refine_input)
	};
	let refine_us = refine_start.elapsed().as_micros() as u64;

	// Compute new leaves for neighbor mask calculation
	let mut new_leaves = leaves;
	{
		#[cfg(feature = "tracing")]
		let _span = tracing::info_span!("update_leaves").entered();
		for group in &refine_output.transition_groups {
			for node in &group.nodes_to_remove {
				new_leaves.remove(node);
			}
			for node in &group.nodes_to_add {
				new_leaves.insert(*node);
			}
		}
	}

	// Stage 2+3: Mesh generation (sample + surface nets)
	let mesh_start = Instant::now();
	let ready_chunks = {
		#[cfg(feature = "tracing")]
		let _span = tracing::info_span!("mesh_generation_stage").entered();
		process_transitions(
			world_id,
			&refine_output.transition_groups,
			&sampler,
			&new_leaves,
			&config,
		)
	};
	let mesh_us = mesh_start.elapsed().as_micros() as u64;

	// Group ready chunks by their transition group_key
	let mut chunks_by_group: HashMap<OctreeNode, Vec<ReadyChunk>> = HashMap::new();
	for chunk in ready_chunks {
		// Extract group_key from presentation hint
		let group_key = match &chunk.hint {
			super::types::PresentationHint::FadeIn { group_key } => *group_key,
			super::types::PresentationHint::FadeOut { group_key } => *group_key,
			super::types::PresentationHint::Immediate => chunk.node, // Fallback
		};
		chunks_by_group.entry(group_key).or_default().push(chunk);
	}

	// Build completed transitions with their ready chunks
	let mut transitions: Vec<CompletedTransition> = refine_output
		.transition_groups
		.into_iter()
		.map(|group| {
			use crate::octree::TransitionType;
			let ready_chunks = chunks_by_group
				.remove(&group.group_key)
				.unwrap_or_default();
			let is_collapse = matches!(group.transition_type, TransitionType::Merge);
			CompletedTransition {
				group_key: group.group_key,
				is_collapse,
				nodes_to_remove: group.nodes_to_remove.to_vec(),
				nodes_to_add: group.nodes_to_add.to_vec(),
				ready_chunks,
			}
		})
		.collect();

	// Sort: collapses first (load shedding), then subdivides (add detail)
	// Within each category, maintain distance-based ordering from refinement
	transitions.sort_by_key(|t| !t.is_collapse); // false < true, so collapses come first

	RefinementResult {
		world_id,
		transitions,
		stats: RefinementStats {
			subdivisions: refine_output.stats.subdivisions_performed,
			collapses: refine_output.stats.collapses_performed,
			neighbor_subdivisions: refine_output.stats.neighbor_subdivisions_performed,
			refine_us,
			mesh_us,
		},
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::constants::SAMPLE_SIZE_CB;

	#[derive(Clone)]
	struct TestSampler;

	impl VolumeSampler for TestSampler {
		fn sample_volume(
			&self,
			_grid_offset: [i64; 3],
			_voxel_size: f64,
			volume: &mut [i8; SAMPLE_SIZE_CB],
			materials: &mut [u8; SAMPLE_SIZE_CB],
		) {
			// Surface at z=16
			for x in 0..32 {
				for y in 0..32 {
					for z in 0..32 {
						let idx = x * 32 * 32 + y * 32 + z;
						volume[idx] = if z < 16 { -1 } else { 1 };
						materials[idx] = 1;
					}
				}
			}
		}
	}

	#[test]
	fn test_pipeline_empty_leaves() {
		let mut pipeline = AsyncRefinementPipeline::new();

		assert!(!pipeline.is_busy());

		let started = pipeline.start(RefinementRequest {
			world_id: WorldId::new(),
			viewer_pos: DVec3::ZERO,
			leaves: HashSet::new(),
			config: OctreeConfig::default(),
			budget: RefinementBudget::DEFAULT,
			sampler: TestSampler,
		});

		assert!(started);
		assert!(pipeline.is_busy());

		// Poll until complete
		let mut result = None;
		for _ in 0..1000 {
			if let Some(r) = pipeline.poll_results() {
				result = Some(r);
				break;
			}
			std::thread::sleep(std::time::Duration::from_millis(1));
		}

		assert!(result.is_some());
		let result = result.unwrap();
		assert!(result.transitions.is_empty());
	}

	#[test]
	fn test_cannot_start_when_busy() {
		let mut pipeline = AsyncRefinementPipeline::new();

		let request1 = RefinementRequest {
			world_id: WorldId::new(),
			viewer_pos: DVec3::ZERO,
			leaves: HashSet::new(),
			config: OctreeConfig::default(),
			budget: RefinementBudget::DEFAULT,
			sampler: TestSampler,
		};

		let request2 = RefinementRequest {
			world_id: WorldId::new(),
			viewer_pos: DVec3::ZERO,
			leaves: HashSet::new(),
			config: OctreeConfig::default(),
			budget: RefinementBudget::DEFAULT,
			sampler: TestSampler,
		};

		assert!(pipeline.start(request1));
		assert!(!pipeline.start(request2)); // Already busy
	}
}
