//! Async Pipeline Processor
//!
//! Non-blocking wrapper around `process_transitions` using rayon and channels.
//!
//! # Usage
//!
//! ```ignore
//! let mut pipeline = AsyncPipeline::new();
//!
//! // Start processing (non-blocking)
//! pipeline.start(world_id, transitions, sampler, leaves, config);
//!
//! // Poll each frame
//! if let Some(events) = pipeline.poll_events() {
//!     for event in events {
//!         // Handle PipelineEvent::NodesExpired, PipelineEvent::ChunksReady
//!     }
//! }
//! ```

use std::collections::HashSet;

use crossbeam_channel::{self as channel, Receiver, TryRecvError};

use super::process::process_transitions;
use crate::octree::{OctreeConfig, OctreeNode, TransitionGroup};
use crate::pipeline::types::{PipelineEvent, ReadyChunk, VolumeSampler};
use crate::world::WorldId;

/// Non-blocking async pipeline processor.
///
/// Wraps `process_transitions` to run on rayon's thread pool without blocking
/// the main thread. Uses channels for result delivery.
pub struct AsyncPipeline {
  /// Receiver for the pending task's result (None if idle)
  receiver: Option<Receiver<Vec<ReadyChunk>>>,
  /// Stored when start() is called, emitted with poll_events()
  pending_world_id: Option<WorldId>,
  pending_expired_nodes: Vec<OctreeNode>,
}

impl AsyncPipeline {
  /// Create a new async pipeline.
  ///
  /// Thread count is managed by rayon's global thread pool.
  pub fn new() -> Self {
    Self {
      receiver: None,
      pending_world_id: None,
      pending_expired_nodes: Vec::new(),
    }
  }

  /// Check if a task is currently running.
  pub fn is_busy(&self) -> bool {
    self.receiver.is_some()
  }

  /// Start processing transitions (non-blocking).
  ///
  /// Returns `true` if processing started, `false` if already busy.
  pub fn start<S: VolumeSampler + Clone + 'static>(
    &mut self,
    world_id: WorldId,
    transition_groups: Vec<TransitionGroup>,
    sampler: S,
    leaves: HashSet<OctreeNode>,
    config: OctreeConfig,
  ) -> bool {
    if self.is_busy() {
      return false;
    }

    // Extract nodes_to_remove from all transition groups for NodesExpired event
    self.pending_expired_nodes = transition_groups
      .iter()
      .flat_map(|group| group.nodes_to_remove.iter().copied())
      .collect();
    self.pending_world_id = Some(world_id);

    // Create channel for result
    let (sender, receiver) = channel::bounded(1);
    self.receiver = Some(receiver);

    // Spawn processing on rayon's thread pool
    rayon::spawn(move || {
      let result = process_transitions(world_id, &transition_groups, &sampler, &leaves, &config);
      // Ignore send error (receiver dropped = task cancelled)
      let _ = sender.send(result);
    });

    true
  }

  /// Poll for pipeline events (non-blocking).
  ///
  /// Returns `Some(events)` when processing completes, with events in order:
  /// 1. `NodesExpired` - nodes that should be despawned
  /// 2. `ChunksReady` - new meshes to spawn
  ///
  /// Returns `None` if still running or no task was started.
  pub fn poll_events(&mut self) -> Option<Vec<PipelineEvent>> {
    let receiver = self.receiver.as_ref()?;
    let world_id = self.pending_world_id?;

    match receiver.try_recv() {
      Ok(chunks) => {
        self.receiver = None;
        self.pending_world_id = None;

        let expired_nodes = std::mem::take(&mut self.pending_expired_nodes);

        let mut events = Vec::with_capacity(2);

        // NodesExpired always comes first (despawn before spawn)
        if !expired_nodes.is_empty() {
          events.push(PipelineEvent::NodesExpired {
            world_id,
            nodes: expired_nodes,
          });
        }

        // ChunksReady with new meshes
        if !chunks.is_empty() {
          events.push(PipelineEvent::ChunksReady { world_id, chunks });
        }

        Some(events)
      }
      Err(TryRecvError::Empty) => None, // Still running
      Err(TryRecvError::Disconnected) => {
        // Sender dropped without sending (shouldn't happen)
        self.receiver = None;
        self.pending_world_id = None;
        self.pending_expired_nodes.clear();
        None
      }
    }
  }

  /// Cancel any pending task.
  ///
  /// Note: The task will still run to completion on the worker thread,
  /// but results will be discarded.
  pub fn cancel(&mut self) {
    self.receiver = None;
    self.pending_world_id = None;
    self.pending_expired_nodes.clear();
  }

  /// Get the number of worker threads in rayon's pool.
  pub fn num_threads(&self) -> usize {
    rayon::current_num_threads()
  }
}

impl Default for AsyncPipeline {
  fn default() -> Self {
    Self::new()
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
  fn test_async_pipeline_empty() {
    let mut pipeline = AsyncPipeline::new();

    assert!(!pipeline.is_busy());
    assert!(pipeline.poll_events().is_none());
  }

  #[test]
  fn test_async_pipeline_process() {
    let mut pipeline = AsyncPipeline::new();

    let world_id = WorldId::new();
    let config = OctreeConfig::default();
    let sampler = TestSampler;
    let leaves = HashSet::new();

    // Empty transitions should complete quickly
    let started = pipeline.start(world_id, vec![], sampler, leaves, config);
    assert!(started);

    // Poll until complete
    let mut result = None;
    for _ in 0..1000 {
      if let Some(r) = pipeline.poll_events() {
        result = Some(r);
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert!(result.is_some());
    // No transitions = no events (empty Vec)
    assert!(result.unwrap().is_empty());
  }
}
