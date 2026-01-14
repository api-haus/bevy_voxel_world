//! Pipeline I/O types for the voxel task graph.
//!
//! ```text
//!                          VOXEL PIPELINE TASK GRAPH
//!                          =========================
//!
//!                     ┌─────────────────────────────────────────────────────────┐
//!                     │                    WorkSource                           │
//!                     │  ┌─────────────┐              ┌───────────────┐         │
//!                     │  │ REFINEMENT  │              │ INVALIDATION  │         │
//!                     │  │ (LOD change)│              │ (terrain edit)│         │
//!                     │  └──────┬──────┘              └───────┬───────┘         │
//!                     └─────────┼────────────────────────────┼──────────────────┘
//!                               │                            │
//!                               ▼                            │
//! ┌──────────────────────────────────────────────────────────┼──────────────────┐
//! │ STAGE 1: REFINEMENT                                      │                  │
//! │ Input:  RefinementInput { viewer_pos, prev_leaves }      │                  │
//! │ Output: Vec<TransitionGroup>                             │                  │
//! │                                                          │                  │
//! │ Subdivide: 1 parent → 8 children                         │                  │
//! │ Merge:     8 children → 1 parent                         │                  │
//! └────────────────────────┬─────────────────────────────────┘                  │
//!                          │                                                     │
//!                          │ TransitionGroup[]                                   │
//!                          ▼                                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │ STAGE 2: PREFILTER                                                          │
//! │ Input:  Vec<(OctreeNode, WorkSource)> + VolumeSampler                       │
//! │ Output: Vec<PrefilterOutput>                                                │
//! │                                                                             │
//! │ ┌─────────────────────────────────────────────────────────────────────────┐ │
//! │ │ Homogeneity Check (8-corner sample):                                    │ │
//! │ │       4──────5           If all 8 corners have same sign:               │ │
//! │ │      /│     /│             - All negative → AllSolid → Skip             │ │
//! │ │     6─┼────7 │             - All positive → AllAir → Skip               │ │
//! │ │     │ 0────┼─1             - Otherwise → sample full 32³ volume         │ │
//! │ │     │/     │/                                                           │ │
//! │ │     2──────3                                                            │ │
//! │ └─────────────────────────────────────────────────────────────────────────┘ │
//! └────────────────────────┬────────────────────────────────────────────────────┘
//!                          │ PrefilterOutput[] (only Volume variants proceed)
//!                          ▼
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │ STAGE 3: MESHING                                                            │
//! │ Input:  Vec<MeshInput> { node, volume, materials, config }                  │
//! │ Output: Vec<MeshResult>                                                     │
//! │                                                                             │
//! │ Uses: surface_nets::generate() - pure function, rayon parallel              │
//! └────────────────────────┬────────────────────────────────────────────────────┘
//!                          │ MeshResult[]
//!                          ▼
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │ STAGE 4: COMPOSITION                                                        │
//! │ Input:  CompositionInput { mesh_results, transition_groups }                │
//! │ Output: Vec<GroupedMesh>                                                    │
//! │                                                                             │
//! │ Subdivide: Groups 8 child meshes by parent                                  │
//! │ Merge:     Groups 1 parent mesh by parent                                   │
//! │                                                                             │
//! │ NOTE: INVALIDATION work_source bypasses this stage!                         │
//! └────────────────────────┬────────────────────────────────────────────────────┘
//!                          │ GroupedMesh[] | MeshResult[] (invalidation bypass)
//!                          ▼
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │ STAGE 5: PRESENTATION                                                       │
//! │ Input:  Vec<GroupedMesh> | Vec<MeshResult>                                  │
//! │ Output: Vec<ReadyChunk>                                                     │
//! │                                                                             │
//! │ PresentationHint: Immediate | FadeIn | FadeOut                              │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```

use smallvec::SmallVec;

use crate::constants::SAMPLE_SIZE_CB;
use crate::octree::{OctreeNode, TransitionType};
use crate::types::{MaterialId, MeshConfig, MeshOutput, MinMaxAABB, SdfSample};
use crate::world::WorldId;

// =============================================================================
// WorkSource - Determines pipeline routing
// =============================================================================

/// Origin of pipeline work - determines routing through stages.
///
/// - `Refinement`: Full pipeline with composition (groups of 9)
/// - `Invalidation`: Bypasses composition, goes directly to presentation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkSource {
  /// LOD refinement triggered by viewer movement.
  /// Goes through full pipeline including composition.
  Refinement,

  /// Terrain invalidation triggered by edit/brush.
  /// Bypasses composition, produces Immediate presentation hint.
  Invalidation,
}

// =============================================================================
// Stage 2: Presample Types
// =============================================================================

/// Sampled volume data from presampling.
#[derive(Clone)]
pub struct SampledVolume {
  pub volume: Box<[SdfSample; SAMPLE_SIZE_CB]>,
  pub materials: Box<[MaterialId; SAMPLE_SIZE_CB]>,
}

impl std::fmt::Debug for SampledVolume {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "SampledVolume([32³ samples])")
  }
}

/// Output from presampling a single node.
pub struct PresampleOutput {
  /// The node that was presampled.
  pub node: OctreeNode,

  /// Sampled volume if surface exists, None if homogeneous.
  pub volume: Option<SampledVolume>,

  /// Origin of this work (for routing in later stages).
  pub work_source: WorkSource,
}

/// Volume-based sampler trait for SIMD-efficient batch sampling.
///
/// Always samples 32x32x32 volumes in one call, enabling SIMD optimization.
/// Inspired by FastNoise2's `fnGenUniformGrid3D` API with simplified parameters.
pub trait VolumeSampler: Send + Sync {
  /// Sample a 32x32x32 SDF volume.
  ///
  /// # Parameters
  /// - `sample_start`: World-space origin of the volume (position of first sample)
  /// - `voxel_size`: Distance between adjacent samples in world units
  /// - `volume`: Output buffer for SDF values (32³ = 32,768 i8 values)
  /// - `materials`: Output buffer for material IDs (32³ = 32,768 u8 values)
  ///
  /// # Memory Layout
  /// Uses X-slowest, Z-fastest indexing: `index = x * 32² + y * 32 + z`
  ///
  /// # World Position
  /// Sample at grid position (x, y, z) corresponds to world position:
  /// `world_pos = sample_start + [x, y, z] * voxel_size`
  fn sample_volume(
    &self,
    sample_start: [f64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  );
}

/// Blanket impl for boxed trait objects.
impl VolumeSampler for Box<dyn VolumeSampler> {
  fn sample_volume(
    &self,
    sample_start: [f64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    (**self).sample_volume(sample_start, voxel_size, volume, materials)
  }
}

// =============================================================================
// Stage 3: Meshing Types
// =============================================================================

/// Input for meshing a single node.
pub struct MeshInput {
  /// The node being meshed.
  pub node: OctreeNode,

  /// Sampled SDF volume (32³).
  pub volume: Box<[SdfSample; SAMPLE_SIZE_CB]>,

  /// Material IDs per sample.
  pub materials: Box<[MaterialId; SAMPLE_SIZE_CB]>,

  /// Meshing configuration (voxel size, neighbor mask, etc.).
  pub config: MeshConfig,

  /// Origin of this work (preserved through pipeline).
  pub work_source: WorkSource,
}

/// Result of meshing a single node.
pub struct MeshResult {
  /// The node that was meshed.
  pub node: OctreeNode,

  /// Generated mesh output (may be empty if no surface).
  pub output: MeshOutput,

  /// Time taken to mesh in microseconds.
  pub timing_us: u64,

  /// Origin of this work (preserved through pipeline).
  pub work_source: WorkSource,
}

// =============================================================================
// Stage 4: Composition Types
// =============================================================================

/// Mesh data for a single node within a group.
pub struct NodeMesh {
  /// The node this mesh belongs to.
  pub node: OctreeNode,

  /// The generated mesh output.
  pub output: MeshOutput,
}

/// Group of related meshes from a single TransitionGroup.
///
/// For subdivide: contains up to 8 child meshes.
/// For merge: contains 1 parent mesh.
pub struct GroupedMesh {
  /// Parent node (group_key from TransitionGroup).
  pub group_key: OctreeNode,

  /// The meshes in this group.
  /// - Subdivide: up to 8 child meshes (some may be skipped)
  /// - Merge: 1 parent mesh
  pub meshes: SmallVec<[NodeMesh; 9]>,

  /// Type of transition (determines presentation behavior).
  pub transition_type: TransitionType,
}

// =============================================================================
// Stage 5: Presentation Types
// =============================================================================

/// How a chunk should be presented to the renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationHint {
  /// Swap mesh immediately (used for invalidation).
  Immediate,

  /// Fade in as part of a subdivide transition.
  /// The group_key identifies the parent whose children are appearing.
  FadeIn { group_key: OctreeNode },

  /// Fade out as part of a merge transition.
  /// The group_key identifies the parent that will remain.
  FadeOut { group_key: OctreeNode },
}

/// Byte-level mesh data ready for FFI to game engines.
#[derive(Clone)]
pub struct MeshData {
  /// Vertex data as raw bytes (Vertex struct layout).
  pub vertices: Vec<u8>,

  /// Index data as raw bytes (u32 layout).
  pub indices: Vec<u8>,

  /// Number of vertices.
  pub vertex_count: u32,

  /// Number of indices.
  pub index_count: u32,

  /// Mesh bounding box.
  pub bounds: MinMaxAABB,
}

impl std::fmt::Debug for MeshData {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("MeshData")
      .field("vertex_count", &self.vertex_count)
      .field("index_count", &self.index_count)
      .field("bounds", &self.bounds)
      .finish()
  }
}

/// Final output ready for rendering.
#[derive(Debug)]
pub struct ReadyChunk {
  /// The world this chunk belongs to.
  pub world_id: WorldId,

  /// The node this chunk represents.
  pub node: OctreeNode,

  /// Serialized mesh data.
  pub mesh_data: MeshData,

  /// Presentation hint for the renderer.
  pub hint: PresentationHint,
}

// =============================================================================
// Pipeline Epoch (for stale work detection)
// =============================================================================

/// Epoch counter for detecting stale invalidation work.
///
/// When refinement changes the octree structure (subdivide/merge),
/// any in-flight invalidation work for affected nodes becomes stale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch(pub u64);

impl Epoch {
  /// Create a new epoch starting at 0.
  pub fn new() -> Self {
    Self(0)
  }

  /// Increment the epoch (called after refinement changes structure).
  pub fn increment(&mut self) {
    self.0 += 1;
  }
}
