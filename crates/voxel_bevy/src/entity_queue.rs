//! Entity Operation Queue with Atomic Transition Groups
//!
//! Spreads transition group application across frames while ensuring each
//! group is applied atomically (despawn + spawn in same frame) to avoid
//! visual pops.
//!
//! # Usage
//!
//! ```ignore
//! let mut queue = EntityQueue::new(EntityQueueConfig {
//!     max_groups_per_frame: 4,  // Apply up to 4 transition groups per frame
//! });
//!
//! // Queue transitions from async pipeline
//! queue.queue_transitions(result.transitions);
//!
//! // Process within budget each frame (atomic per group)
//! queue.process_frame(|transition| {
//!     // Despawn old nodes
//!     for node in &transition.nodes_to_remove {
//!         despawn_entity(node);
//!     }
//!     // Spawn new chunks (same frame = no pop)
//!     for chunk in &transition.ready_chunks {
//!         spawn_entity(chunk);
//!     }
//! });
//! ```

use std::collections::VecDeque;
// WASM compat: std::time::Instant panics on wasm32
use web_time::Instant;

use voxel_plugin::pipeline::CompletedTransition;

/// Configuration for entity queue.
#[derive(Clone, Debug)]
pub struct EntityQueueConfig {
	/// Maximum transition groups to apply per frame.
	/// Each group is applied atomically (despawn + spawn together).
	pub max_groups_per_frame: usize,
	/// Maximum milliseconds to spend on entity ops per frame.
	/// Groups are applied atomically, so this is a soft limit -
	/// we finish the current group even if over budget.
	pub max_ms_per_frame: f32,
}

impl Default for EntityQueueConfig {
	fn default() -> Self {
		Self {
			max_groups_per_frame: 8,
			max_ms_per_frame: 4.0, // 4ms leaves headroom in 16.6ms frame
		}
	}
}

/// Entity operation queue with atomic transition groups.
pub struct EntityQueue {
	config: EntityQueueConfig,
	pending_transitions: VecDeque<CompletedTransition>,
}

/// Statistics from queue processing.
#[derive(Debug, Clone, Copy, Default)]
pub struct QueueStats {
	/// Number of transition groups applied this frame.
	pub groups_applied: usize,
	/// Total spawns performed this frame.
	pub spawns: usize,
	/// Total despawns performed this frame.
	pub despawns: usize,
	/// Time spent in microseconds.
	pub elapsed_us: u64,
	/// Transition groups remaining in queue.
	pub pending_groups: usize,
}

impl EntityQueue {
	/// Create a new entity queue with the given configuration.
	pub fn new(config: EntityQueueConfig) -> Self {
		Self {
			config,
			pending_transitions: VecDeque::new(),
		}
	}

	/// Queue transitions for atomic application.
	pub fn queue_transitions(&mut self, transitions: impl IntoIterator<Item = CompletedTransition>) {
		self.pending_transitions.extend(transitions);
	}

	/// Check if queue has pending work.
	pub fn has_pending(&self) -> bool {
		!self.pending_transitions.is_empty()
	}

	/// Get number of pending transition groups.
	pub fn pending_count(&self) -> usize {
		self.pending_transitions.len()
	}

	/// Process transition groups atomically within budget.
	///
	/// Each transition group is applied completely (despawn + spawn)
	/// before moving to the next. This prevents visual pops from
	/// partial transitions.
	pub fn process_frame<F>(&mut self, mut handler: F) -> QueueStats
	where
		F: FnMut(&CompletedTransition),
	{
		let start = Instant::now();
		let budget_us = (self.config.max_ms_per_frame * 1000.0) as u64;

		let mut stats = QueueStats::default();

		while stats.groups_applied < self.config.max_groups_per_frame {
			// Check time budget (but always finish at least one group if we started)
			if stats.groups_applied > 0 && start.elapsed().as_micros() as u64 >= budget_us {
				break;
			}

			let Some(transition) = self.pending_transitions.pop_front() else {
				break;
			};

			// Track stats before applying
			stats.despawns += transition.nodes_to_remove.len();
			stats.spawns += transition.ready_chunks.len();

			// Apply atomically (handler does despawn + spawn)
			handler(&transition);

			stats.groups_applied += 1;
		}

		stats.elapsed_us = start.elapsed().as_micros() as u64;
		stats.pending_groups = self.pending_transitions.len();

		stats
	}

	/// Clear all pending transitions.
	pub fn clear(&mut self) {
		self.pending_transitions.clear();
	}

	/// Update configuration.
	pub fn set_config(&mut self, config: EntityQueueConfig) {
		self.config = config;
	}
}

impl Default for EntityQueue {
	fn default() -> Self {
		Self::new(EntityQueueConfig::default())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use voxel_plugin::octree::OctreeNode;
	use voxel_plugin::pipeline::{PresentationHint, ReadyChunk};
	use voxel_plugin::world::WorldId;
	use voxel_plugin::MeshOutput;

	fn make_transition(
		group_key: OctreeNode,
		remove_count: usize,
		spawn_count: usize,
		is_collapse: bool,
	) -> CompletedTransition {
		let world_id = WorldId::new();
		CompletedTransition {
			group_key,
			is_collapse,
			nodes_to_remove: (0..remove_count)
				.map(|i| OctreeNode::new(i as i32, 0, 0, 1))
				.collect(),
			nodes_to_add: (0..spawn_count)
				.map(|i| OctreeNode::new(i as i32, 0, 0, 0))
				.collect(),
			ready_chunks: (0..spawn_count)
				.map(|i| ReadyChunk {
					world_id,
					node: OctreeNode::new(i as i32, 0, 0, 0),
					output: MeshOutput::default(),
					hint: PresentationHint::FadeIn { group_key },
					timing_us: 0,
				})
				.collect(),
		}
	}

	#[test]
	fn test_empty_queue() {
		let mut queue = EntityQueue::default();

		assert!(!queue.has_pending());
		assert_eq!(queue.pending_count(), 0);

		let stats = queue.process_frame(|_| {});
		assert_eq!(stats.groups_applied, 0);
		assert_eq!(stats.spawns, 0);
		assert_eq!(stats.despawns, 0);
	}

	#[test]
	fn test_atomic_application() {
		let mut queue = EntityQueue::default();

		let parent = OctreeNode::new(0, 0, 0, 2);
		queue.queue_transitions(vec![make_transition(parent, 1, 8, false)]); // Subdivide: 1 parent -> 8 children

		assert!(queue.has_pending());
		assert_eq!(queue.pending_count(), 1);

		let mut applied = Vec::new();
		let stats = queue.process_frame(|t| {
			applied.push((t.nodes_to_remove.len(), t.ready_chunks.len()));
		});

		// Should apply the entire group atomically
		assert_eq!(applied.len(), 1);
		assert_eq!(applied[0], (1, 8)); // 1 despawn + 8 spawns in same call
		assert_eq!(stats.groups_applied, 1);
		assert_eq!(stats.despawns, 1);
		assert_eq!(stats.spawns, 8);
		assert!(!queue.has_pending());
	}

	#[test]
	fn test_group_limit() {
		let mut queue = EntityQueue::new(EntityQueueConfig {
			max_groups_per_frame: 2,
			max_ms_per_frame: 1000.0, // High time budget
		});

		// Queue 5 transitions
		for i in 0..5 {
			let parent = OctreeNode::new(i, 0, 0, 2);
			queue.queue_transitions(vec![make_transition(parent, 1, 4, false)]);
		}

		assert_eq!(queue.pending_count(), 5);

		// First frame: apply 2 groups
		let stats = queue.process_frame(|_| {});
		assert_eq!(stats.groups_applied, 2);
		assert_eq!(stats.pending_groups, 3);

		// Second frame: apply 2 more
		let stats = queue.process_frame(|_| {});
		assert_eq!(stats.groups_applied, 2);
		assert_eq!(stats.pending_groups, 1);

		// Third frame: apply last 1
		let stats = queue.process_frame(|_| {});
		assert_eq!(stats.groups_applied, 1);
		assert_eq!(stats.pending_groups, 0);
	}
}
