//! Async Pipeline Processor
//!
//! Non-blocking wrapper around `process_transitions` using the cross-platform
//! `TaskExecutor`.
//!
//! # Usage
//!
//! ```ignore
//! let mut pipeline = AsyncPipeline::new(4); // 4 worker threads
//!
//! // Start processing (non-blocking)
//! pipeline.start(world_id, transitions, sampler, leaves, config);
//!
//! // Poll each frame
//! if let Some(ready_chunks) = pipeline.poll() {
//!     // Present chunks to renderer
//! }
//! ```

use std::collections::HashSet;
use std::sync::Arc;

use crate::octree::{OctreeConfig, OctreeNode, TransitionGroup};
use crate::pipeline::types::{ReadyChunk, VolumeSampler};
use crate::threading::{TaskExecutor, TaskId};
use crate::world::WorldId;

use super::process::process_transitions;

/// Non-blocking async pipeline processor.
///
/// Wraps `process_transitions` to run on background threads without blocking
/// the main thread. Works on native, emscripten, and falls back to synchronous
/// execution on wasm32-unknown-unknown.
pub struct AsyncPipeline {
    executor: Arc<TaskExecutor>,
    pending_task: Option<TaskId>,
}

impl AsyncPipeline {
    /// Create a new async pipeline with the specified number of worker threads.
    pub fn new(num_threads: usize) -> Self {
        Self {
            executor: Arc::new(TaskExecutor::new(num_threads)),
            pending_task: None,
        }
    }

    /// Create with default thread count (number of CPUs).
    pub fn default_threads() -> Self {
        Self {
            executor: Arc::new(TaskExecutor::default_threads()),
            pending_task: None,
        }
    }

    /// Create using a shared executor.
    ///
    /// Useful when you want multiple pipelines to share the same thread pool.
    pub fn with_executor(executor: Arc<TaskExecutor>) -> Self {
        Self {
            executor,
            pending_task: None,
        }
    }

    /// Check if a task is currently running.
    pub fn is_busy(&self) -> bool {
        self.pending_task
            .map(|id| self.executor.is_pending(id))
            .unwrap_or(false)
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

        // Spawn processing on background thread
        let task_id = self.executor.spawn(move || {
            process_transitions(world_id, &transition_groups, &sampler, &leaves, &config)
        });

        self.pending_task = Some(task_id);
        true
    }

    /// Poll for completion (non-blocking).
    ///
    /// Returns `Some(chunks)` when processing completes, `None` if still running
    /// or no task was started.
    pub fn poll(&mut self) -> Option<Vec<ReadyChunk>> {
        let task_id = self.pending_task?;

        if let Some(result) = self.executor.poll::<Vec<ReadyChunk>>(task_id) {
            self.pending_task = None;
            Some(result)
        } else {
            None
        }
    }

    /// Cancel any pending task.
    ///
    /// Note: The task will still run to completion on the worker thread,
    /// but results will be discarded.
    pub fn cancel(&mut self) {
        self.pending_task = None;
    }

    /// Get the number of worker threads.
    pub fn num_threads(&self) -> usize {
        self.executor.num_threads()
    }

    /// Get a reference to the underlying executor.
    pub fn executor(&self) -> &Arc<TaskExecutor> {
        &self.executor
    }
}

impl Default for AsyncPipeline {
    fn default() -> Self {
        Self::default_threads()
    }
}

// =============================================================================
// Batch Pipeline - Process multiple worlds/transitions concurrently
// =============================================================================

/// Batch identifier for tracking multiple concurrent tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BatchId(u64);

impl BatchId {
    fn next() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Result from a batch task.
pub struct BatchResult {
    pub batch_id: BatchId,
    pub world_id: WorldId,
    pub chunks: Vec<ReadyChunk>,
}

/// Pipeline that can process multiple batches concurrently.
///
/// Useful for multi-world scenarios or processing multiple transition groups
/// in parallel.
pub struct BatchPipeline {
    executor: Arc<TaskExecutor>,
    pending: Vec<(BatchId, WorldId, TaskId)>,
}

impl BatchPipeline {
    /// Create a new batch pipeline.
    pub fn new(num_threads: usize) -> Self {
        Self {
            executor: Arc::new(TaskExecutor::new(num_threads)),
            pending: Vec::new(),
        }
    }

    /// Create with default thread count.
    pub fn default_threads() -> Self {
        Self {
            executor: Arc::new(TaskExecutor::default_threads()),
            pending: Vec::new(),
        }
    }

    /// Create using a shared executor.
    pub fn with_executor(executor: Arc<TaskExecutor>) -> Self {
        Self {
            executor,
            pending: Vec::new(),
        }
    }

    /// Submit a batch for processing (non-blocking).
    pub fn submit<S: VolumeSampler + Clone + 'static>(
        &mut self,
        world_id: WorldId,
        transition_groups: Vec<TransitionGroup>,
        sampler: S,
        leaves: HashSet<OctreeNode>,
        config: OctreeConfig,
    ) -> BatchId {
        let batch_id = BatchId::next();

        let task_id = self.executor.spawn(move || {
            process_transitions(world_id, &transition_groups, &sampler, &leaves, &config)
        });

        self.pending.push((batch_id, world_id, task_id));
        batch_id
    }

    /// Poll for any completed batches (non-blocking).
    ///
    /// Returns completed results and removes them from pending.
    pub fn poll(&mut self) -> Vec<BatchResult> {
        let mut completed = Vec::new();
        let mut still_pending = Vec::new();

        for (batch_id, world_id, task_id) in self.pending.drain(..) {
            if let Some(chunks) = self.executor.poll::<Vec<ReadyChunk>>(task_id) {
                completed.push(BatchResult {
                    batch_id,
                    world_id,
                    chunks,
                });
            } else {
                still_pending.push((batch_id, world_id, task_id));
            }
        }

        self.pending = still_pending;
        completed
    }

    /// Get the number of pending batches.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Check if any batches are pending.
    pub fn is_busy(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Cancel all pending batches.
    pub fn cancel_all(&mut self) {
        self.pending.clear();
    }

    /// Get the number of worker threads.
    pub fn num_threads(&self) -> usize {
        self.executor.num_threads()
    }
}

impl Default for BatchPipeline {
    fn default() -> Self {
        Self::default_threads()
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
            _sample_start: [f64; 3],
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
        let mut pipeline = AsyncPipeline::new(2);

        assert!(!pipeline.is_busy());
        assert!(pipeline.poll().is_none());
    }

    #[test]
    fn test_async_pipeline_process() {
        let mut pipeline = AsyncPipeline::new(2);

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
            if let Some(r) = pipeline.poll() {
                result = Some(r);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(result.is_some());
        assert!(result.unwrap().is_empty()); // No transitions = no chunks
    }

    #[test]
    fn test_batch_pipeline() {
        let mut pipeline = BatchPipeline::new(4);

        let config = OctreeConfig::default();
        let sampler = TestSampler;

        // Submit multiple batches
        let id1 = pipeline.submit(
            WorldId::new(),
            vec![],
            sampler.clone(),
            HashSet::new(),
            config.clone(),
        );
        let id2 = pipeline.submit(
            WorldId::new(),
            vec![],
            sampler.clone(),
            HashSet::new(),
            config.clone(),
        );

        assert_eq!(pipeline.pending_count(), 2);

        // Poll until all complete
        let mut results = Vec::new();
        for _ in 0..1000 {
            results.extend(pipeline.poll());
            if results.len() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.batch_id == id1));
        assert!(results.iter().any(|r| r.batch_id == id2));
    }
}
