//! Stage 3: Meshing
//!
//! Thin wrapper around `surface_nets::generate()` that:
//! - Processes inputs in parallel via rayon
//! - Tracks timing per mesh
//! - Preserves work_source for routing
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │ Meshing Stage                                                           │
//! │                                                                         │
//! │  MeshInput { node, volume, materials, config, work_source }             │
//! │           │                                                             │
//! │           ▼                                                             │
//! │  ┌─────────────────────────────────────────────────────────┐            │
//! │  │ surface_nets::generate(&volume, &materials, &config)    │            │
//! │  │ → MeshOutput { vertices, indices, bounds }              │            │
//! │  └─────────────────────────────────────────────────────────┘            │
//! │           │                                                             │
//! │           ▼                                                             │
//! │  MeshResult { node, output, timing_us, work_source }                    │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```

use web_time::Instant;

use rayon::prelude::*;

use super::types::{MeshInput, MeshResult};
use crate::surface_nets;

/// Mesh a single node using surface nets algorithm.
///
/// This is a pure function that wraps `surface_nets::generate()` with timing.
pub fn mesh_node(input: MeshInput) -> MeshResult {
  let start = Instant::now();
  let output = surface_nets::generate(&input.volume, &input.materials, &input.config);
  let timing_us = start.elapsed().as_micros() as u64;

  MeshResult {
    node: input.node,
    output,
    timing_us,
    work_source: input.work_source,
  }
}

/// Mesh multiple nodes in parallel using rayon.
///
/// Results maintain the same order as inputs for deterministic output.
pub fn mesh_batch(inputs: Vec<MeshInput>) -> Vec<MeshResult> {
  if inputs.is_empty() {
    return Vec::new();
  }

  inputs.into_par_iter().map(mesh_node).collect()
}

#[cfg(test)]
#[path = "meshing_test.rs"]
mod meshing_test;
