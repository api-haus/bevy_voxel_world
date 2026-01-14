//! Cross-platform threading abstraction using rayon.
//!
//! Uses `rayon::spawn` as the unified threading solution:
//! - Native: rayon's thread pool (std::thread based)
//! - wasm32-unknown-unknown: wasm-bindgen-rayon (Web Workers)
//! - wasm32-unknown-emscripten: rayon via pthreads (Web Workers)
//!
//! # Usage
//!
//! ```ignore
//! let executor = TaskExecutor::new();
//!
//! // Queue work (non-blocking)
//! let task_id = executor.spawn(move || expensive_computation());
//!
//! // Poll for results each frame
//! if let Some(result) = executor.poll::<MyResult>(task_id) {
//!     // Use result
//! }
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Unique identifier for a spawned task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
  fn next() -> Self {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    Self(COUNTER.fetch_add(1, Ordering::Relaxed))
  }
}

/// Type-erased result container.
struct TaskResult {
  data: Box<dyn std::any::Any + Send>,
}

/// Cross-platform task executor using rayon.
///
/// Uses `rayon::spawn` for fire-and-forget work submission, with channels
/// for result retrieval. This works across all platforms where rayon is
/// supported (native, wasm-bindgen-rayon, emscripten with pthreads).
pub struct TaskExecutor {
  /// Completed results waiting to be polled.
  results: Arc<Mutex<HashMap<TaskId, TaskResult>>>,
  /// Currently pending task IDs.
  pending: Arc<Mutex<std::collections::HashSet<TaskId>>>,
}

impl TaskExecutor {
  /// Create a new executor.
  ///
  /// Note: The `num_threads` parameter is ignored - rayon manages its own
  /// thread pool. Use `rayon::ThreadPoolBuilder` if you need to configure
  /// the pool size before creating the executor.
  pub fn new(_num_threads: usize) -> Self {
    Self {
      results: Arc::new(Mutex::new(HashMap::new())),
      pending: Arc::new(Mutex::new(std::collections::HashSet::new())),
    }
  }

  /// Create executor (rayon manages thread count automatically).
  pub fn default_threads() -> Self {
    Self::new(0)
  }

  /// Spawn a task on rayon's thread pool (non-blocking).
  ///
  /// Returns a TaskId that can be used to poll for the result.
  pub fn spawn<F, T>(&self, work: F) -> TaskId
  where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
  {
    let task_id = TaskId::next();

    // Add to pending set
    {
      let mut pending = self.pending.lock().unwrap();
      pending.insert(task_id);
    }

    // Clone Arcs for the closure
    let results = Arc::clone(&self.results);
    let pending = Arc::clone(&self.pending);

    // Spawn on rayon's thread pool
    rayon::spawn(move || {
      // Execute work
      let result = work();

      // Store result
      {
        let mut results = results.lock().unwrap();
        results.insert(
          task_id,
          TaskResult {
            data: Box::new(result),
          },
        );
      }

      // Remove from pending
      {
        let mut pending = pending.lock().unwrap();
        pending.remove(&task_id);
      }
    });

    task_id
  }

  /// Poll for a task's result (non-blocking).
  ///
  /// Returns `Some(result)` if the task completed, `None` if still running.
  /// Returns `None` if the task ID is invalid or already consumed.
  pub fn poll<T: 'static>(&self, task_id: TaskId) -> Option<T> {
    let mut results = self.results.lock().unwrap();
    if let Some(result) = results.remove(&task_id) {
      result.data.downcast::<T>().ok().map(|b| *b)
    } else {
      None
    }
  }

  /// Check if a task is still pending.
  pub fn is_pending(&self, task_id: TaskId) -> bool {
    let pending = self.pending.lock().unwrap();
    pending.contains(&task_id)
  }

  /// Get the number of worker threads in rayon's pool.
  pub fn num_threads(&self) -> usize {
    rayon::current_num_threads()
  }

  /// Get the number of tasks currently queued or running.
  pub fn pending_count(&self) -> usize {
    let pending = self.pending.lock().unwrap();
    pending.len()
  }
}

impl Default for TaskExecutor {
  fn default() -> Self {
    Self::default_threads()
  }
}

impl Clone for TaskExecutor {
  fn clone(&self) -> Self {
    Self {
      results: Arc::clone(&self.results),
      pending: Arc::clone(&self.pending),
    }
  }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_spawn_and_poll() {
    let executor = TaskExecutor::new(2);

    let task_id = executor.spawn(|| 42i32);

    // Poll until complete
    let mut result = None;
    for _ in 0..1000 {
      if let Some(r) = executor.poll::<i32>(task_id) {
        result = Some(r);
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert_eq!(result, Some(42));
  }

  #[test]
  fn test_multiple_tasks() {
    let executor = TaskExecutor::new(4);

    let ids: Vec<_> = (0..10).map(|i| executor.spawn(move || i * 2)).collect();

    // Collect all results
    let mut results = vec![None; 10];
    for _ in 0..1000 {
      for (idx, &task_id) in ids.iter().enumerate() {
        if results[idx].is_none() {
          results[idx] = executor.poll::<i32>(task_id);
        }
      }
      if results.iter().all(|r| r.is_some()) {
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }

    let results: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();
    assert_eq!(results, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
  }

  #[test]
  fn test_heavy_computation() {
    let executor = TaskExecutor::new(4);

    // Spawn CPU-bound work
    let task_id = executor.spawn(|| {
      let mut sum = 0u64;
      for i in 0..1_000_000 {
        sum = sum.wrapping_add(i);
      }
      sum
    });

    // Poll until complete
    let mut result = None;
    for _ in 0..5000 {
      if let Some(r) = executor.poll::<u64>(task_id) {
        result = Some(r);
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert!(result.is_some());
  }

  #[test]
  fn test_default_threads() {
    let executor = TaskExecutor::default_threads();
    assert!(executor.num_threads() >= 1);
  }
}
