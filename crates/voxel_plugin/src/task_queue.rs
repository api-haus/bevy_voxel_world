//! Task queue for parallel meshing operations.
//!
//! Following the IStage pattern: Enqueue → Tick → Completions
//!
//! Uses rayon for parallel execution on all platforms.
//! On WASM, requires wasm-bindgen-rayon initialization before use.

use rayon::prelude::*;
use web_time::Instant;

use crate::{surface_nets, MaterialId, MeshConfig, MeshOutput, SdfSample, SAMPLE_SIZE_CB};

/// Request to generate a mesh from an SDF volume.
#[derive(Clone)]
pub struct MeshRequest {
  /// Unique identifier for this request
  pub id: u64,
  /// SDF volume data (32³ samples)
  pub volume: Box<[SdfSample; SAMPLE_SIZE_CB]>,
  /// Material IDs per sample
  pub materials: Box<[MaterialId; SAMPLE_SIZE_CB]>,
  /// Meshing configuration
  pub config: MeshConfig,
}

/// Completed mesh result.
pub struct MeshCompletion {
  /// Request ID this completion corresponds to
  pub id: u64,
  /// Generated mesh output
  pub output: MeshOutput,
  /// Raw meshing time in microseconds
  pub mesh_time_us: u64,
}

/// Meshing stage that processes requests in parallel.
pub struct MeshingStage {
  /// Pending requests waiting to be processed
  pending: Vec<MeshRequest>,
  /// Completed results ready to be collected
  completed: Vec<MeshCompletion>,
  /// Next request ID
  next_id: u64,
}

impl Default for MeshingStage {
  fn default() -> Self {
    Self::new()
  }
}

impl MeshingStage {
  /// Create a new meshing stage.
  pub fn new() -> Self {
    Self {
      pending: Vec::new(),
      completed: Vec::new(),
      next_id: 0,
    }
  }

  /// Enqueue a mesh request, returning the assigned ID.
  pub fn enqueue(
    &mut self,
    volume: Box<[SdfSample; SAMPLE_SIZE_CB]>,
    materials: Box<[MaterialId; SAMPLE_SIZE_CB]>,
    config: MeshConfig,
  ) -> u64 {
    let id = self.next_id;
    self.next_id += 1;

    self.pending.push(MeshRequest {
      id,
      volume,
      materials,
      config,
    });

    id
  }

  /// Process pending requests in parallel and move completions to output.
  /// Returns the number of tasks processed this tick.
  pub fn tick(&mut self) -> usize {
    if self.pending.is_empty() {
      return 0;
    }

    let requests = std::mem::take(&mut self.pending);
    let count = requests.len();

    let completions: Vec<MeshCompletion> = requests
      .into_par_iter()
      .map(|req| {
        let start = Instant::now();
        let output = surface_nets::generate(&req.volume, &req.materials, &req.config);
        let mesh_time_us = start.elapsed().as_micros() as u64;
        MeshCompletion {
          id: req.id,
          output,
          mesh_time_us,
        }
      })
      .collect();

    self.completed.extend(completions);
    count
  }

  /// Take all completed meshes.
  pub fn drain_completions(&mut self) -> Vec<MeshCompletion> {
    std::mem::take(&mut self.completed)
  }

  /// Number of pending requests.
  pub fn pending_count(&self) -> usize {
    self.pending.len()
  }

  /// Number of completed results waiting to be drained.
  pub fn completed_count(&self) -> usize {
    self.completed.len()
  }

  /// True when no work remains.
  pub fn is_idle(&self) -> bool {
    self.pending.is_empty() && self.completed.is_empty()
  }
}

#[cfg(test)]
#[path = "task_queue_test.rs"]
mod task_queue_test;
