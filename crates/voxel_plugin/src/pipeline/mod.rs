//! Voxel Pipeline Task Graph
//!
//! A directed task graph for voxel processing with parallel execution via
//! rayon.
//!
//! ```text
//! ┌────────────┐     ┌───────────┐     ┌─────────┐     ┌─────────────┐     ┌──────────────┐
//! │ Refinement ├────►│ Presample ├────►│ Meshing ├────►│ Composition ├────►│ Presentation │
//! └────────────┘     └───────────┘     └─────────┘     └─────────────┘     └──────────────┘
//!      │                    │                │                 │                    │
//! TransitionGroup    PresampleOut      MeshResult       GroupedMesh          ReadyChunk
//!   (1→8 or 8→1)     (volume|skip)     (vertices)      (groups of 9)          (final)
//!
//!                     ┌─────────────┐
//!                     │ Invalidation│──► Presample ──► Meshing ──► Presentation (bypass composition)
//!                     └─────────────┘
//! ```
//!
//! # Pipeline Stages
//!
//! 1. **Refinement**: Determines which nodes to subdivide/merge based on viewer
//!    distance
//! 2. **Presample**: Samples full 32³ volume, skips homogeneous chunks
//! 3. **Meshing**: Generates surface nets mesh from SDF volume (parallel via
//!    rayon)
//! 4. **Composition**: Groups meshes by TransitionGroup (9 nodes per group)
//! 5. **Presentation**: Serializes mesh data with presentation hints
//!    (FadeIn/FadeOut/Immediate)
//!
//! # Work Sources
//!
//! - `Refinement`: LOD changes from viewer movement → full pipeline
//! - `Invalidation`: Terrain edits/brushes → bypasses composition for immediate
//!   update

pub mod types;

// Stage implementations
pub mod composition;
pub mod meshing;
pub mod presample;
pub mod presentation;
pub mod process;
pub mod async_process;

// Test utilities
#[cfg(test)]
pub mod test_utils;

// Consistency tests
#[cfg(test)]
#[path = "consistency_test.rs"]
mod consistency_test;

// Re-exports
pub use types::{
  Epoch, GroupedMesh, MeshData, MeshInput, MeshResult, NodeMesh, PresampleOutput, PresentationHint,
  ReadyChunk, SampledVolume, VolumeSampler, WorkSource,
};

// Synchronous entry point
pub use process::{process_transitions, process_transitions_timed, ProcessingStats};

// Async entry points (non-blocking, cross-platform)
pub use async_process::{AsyncPipeline, BatchId, BatchPipeline, BatchResult};
