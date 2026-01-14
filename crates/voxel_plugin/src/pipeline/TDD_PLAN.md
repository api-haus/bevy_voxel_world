# Voxel Pipeline Task Graph - TDD Test Plan

Generated: 2026-01-12

## Pipeline Architecture Overview

```
                         VOXEL PIPELINE TASK GRAPH
                         =========================

                    ┌─────────────────────────────────────────────────────────┐
                    │                    WorkSource                           │
                    │  ┌─────────────┐              ┌───────────────┐         │
                    │  │ REFINEMENT  │              │ INVALIDATION  │         │
                    │  │ (LOD change)│              │ (terrain edit)│         │
                    │  └──────┬──────┘              └───────┬───────┘         │
                    └─────────┼────────────────────────────┼──────────────────┘
                              │                            │
                              ▼                            │
┌──────────────────────────────────────────────────────────┼──────────────────┐
│ STAGE 1: REFINEMENT                                      │                  │
│ ────────────────────                                     │                  │
│ Input:  RefinementInput { viewer_pos, prev_leaves }      │                  │
│ Output: Vec<TransitionGroup>                             │                  │
│                                                          │                  │
│ TransitionGroup contains:                                │                  │
│   - group_key: OctreeNode (parent)                       │                  │
│   - transition_type: Subdivide | Merge                   │                  │
│   - nodes_to_add: SmallVec<[OctreeNode; 8]>              │                  │
│   - nodes_to_remove: SmallVec<[OctreeNode; 8]>           │                  │
│                                                          │                  │
│ Subdivide: 1 parent → 8 children                         │                  │
│ Merge:     8 children → 1 parent                         │                  │
└────────────────────────┬─────────────────────────────────┘                  │
                         │                                                     │
                         │ TransitionGroup[]                                   │
                         ▼                                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ STAGE 2: PREFILTER                                                          │
│ ──────────────────                                                          │
│ Input:  PrefilterInput { nodes: Vec<OctreeNode>, sdf_sampler }              │
│ Output: Vec<PrefilterOutput>                                                │
│                                                                             │
│ PrefilterOutput contains:                                                   │
│   - node: OctreeNode                                                        │
│   - result: PrefilterResult { Volume(Box<[i8; 32768]>) | Skip(SkipReason) } │
│                                                                             │
│ SkipReason:                                                                 │
│   - AllSolid   (corner_mask == 255)                                         │
│   - AllAir     (corner_mask == 0)                                           │
│                                                                             │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │ Homogeneity Check (8-corner sample):                                    │ │
│ │                                                                         │ │
│ │       4──────5           If all 8 corners have same sign:               │ │
│ │      /│     /│             - All negative → AllSolid                    │ │
│ │     6─┼────7 │             - All positive → AllAir                      │ │
│ │     │ 0────┼─1             - Otherwise → sample full 32³ volume         │ │
│ │     │/     │/                                                           │ │
│ │     2──────3                                                            │ │
│ │                                                                         │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────────────────────┘
                         │
                         │ PrefilterOutput[]
                         │ (only Volume variants proceed)
                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ STAGE 3: MESHING                                                            │
│ ────────────────                                                            │
│ Input:  Vec<MeshInput> { node, volume, materials, config }                  │
│ Output: Vec<MeshResult>                                                     │
│                                                                             │
│ MeshResult contains:                                                        │
│   - node: OctreeNode                                                        │
│   - output: MeshOutput { vertices, indices, bounds }                        │
│   - timing_us: u64                                                          │
│                                                                             │
│ Uses: surface_nets::generate() - pure function                              │
│ Parallel: rayon::par_iter over all inputs                                   │
│                                                                             │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │ MeshOutput structure:                                                   │ │
│ │   vertices: Vec<Vertex>          // position, normal, material_weights  │ │
│ │   indices: Vec<u32>              // triangle indices                    │ │
│ │   displaced_positions: Vec<[f32;3]>  // LOD seam adjusted positions     │ │
│ │   bounds: MinMaxAABB             // mesh bounding box                   │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────────────────────┘
                         │
                         │ MeshResult[]
                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ STAGE 4: COMPOSITION                                                        │
│ ────────────────────                                                        │
│ Input:  CompositionInput { mesh_results, transition_groups }                │
│ Output: Vec<GroupedMesh>                                                    │
│                                                                             │
│ GroupedMesh contains:                                                       │
│   - group_key: OctreeNode (parent node from TransitionGroup)                │
│   - meshes: SmallVec<[NodeMesh; 9]>  // 1 or 8 + optional parent            │
│   - transition_type: TransitionType                                         │
│                                                                             │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │ Grouping Logic:                                                         │ │
│ │                                                                         │ │
│ │ Subdivide (1→8):              Merge (8→1):                              │ │
│ │ ┌───────────────────┐         ┌───────────────────┐                     │ │
│ │ │ Group contains:   │         │ Group contains:   │                     │ │
│ │ │  - 8 child meshes │         │  - 1 parent mesh  │                     │ │
│ │ │  - parent ref     │         │  - 8 child refs   │                     │ │
│ │ │    (for fadeout)  │         │    (for fadeout)  │                     │ │
│ │ └───────────────────┘         └───────────────────┘                     │ │
│ │                                                                         │ │
│ │ INVALIDATION path: Skip composition, go directly to Presentation        │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                             │
│ NOTE: INVALIDATION work_source bypasses this stage!                         │
│       Invalidated meshes go straight to Presentation.                       │
└────────────────────────┬────────────────────────────────────────────────────┘
                         │
                         │ GroupedMesh[] | MeshResult[] (invalidation)
                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ STAGE 5: PRESENTATION                                                       │
│ ─────────────────────                                                       │
│ Input:  Vec<GroupedMesh> | Vec<MeshResult> (invalidation)                   │
│ Output: Vec<ReadyChunk>                                                     │
│                                                                             │
│ ReadyChunk contains:                                                        │
│   - node: OctreeNode                                                        │
│   - mesh_data: MeshData (vertices, indices as byte arrays)                  │
│   - presentation_hint: PresentationHint                                     │
│                                                                             │
│ PresentationHint:                                                           │
│   - Immediate           // Invalidation: swap mesh instantly                │
│   - FadeIn { group_key } // Subdivide: fade in new children                 │
│   - FadeOut { group_key } // Merge: fade out children, keep parent          │
│                                                                             │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │ Byte Array Format (for FFI to Unity/Godot):                             │ │
│ │   vertices: Vec<u8>   // Vertex struct as raw bytes                     │ │
│ │   indices: Vec<u8>    // u32 indices as raw bytes                       │ │
│ │   vertex_count: u32                                                     │ │
│ │   index_count: u32                                                      │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## I/O Type Definitions

### WorkSource Enum

```rust
/// Origin of pipeline work - determines routing through stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkSource {
    /// LOD refinement (viewer movement) - full pipeline, uses composition
    Refinement,
    /// Terrain invalidation (edit/brush) - bypass composition, immediate swap
    Invalidation,
}
```

### Stage 1: Refinement Types

```rust
// Already exists in octree/refinement.rs:
// - RefinementInput
// - RefinementOutput
// - TransitionGroup
// - TransitionType (Subdivide, Merge)
```

### Stage 2: Prefilter Types

```rust
/// Reason a chunk can skip meshing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// All samples are solid (inside surface)
    AllSolid,
    /// All samples are air (outside surface)
    AllAir,
}

/// Result of prefiltering a single node.
#[derive(Clone, Debug)]
pub enum PrefilterResult {
    /// Node has surface crossing - contains sampled volume
    Volume {
        volume: Box<[SdfSample; SAMPLE_SIZE_CB]>,
        materials: Box<[MaterialId; SAMPLE_SIZE_CB]>,
    },
    /// Node is homogeneous - skip meshing
    Skip(SkipReason),
}

/// Single prefilter output for a node.
pub struct PrefilterOutput {
    pub node: OctreeNode,
    pub result: PrefilterResult,
    /// Origin of this work (for routing)
    pub work_source: WorkSource,
}

/// Input to the prefilter stage.
pub struct PrefilterInput<S: SdfSampler> {
    pub nodes: Vec<(OctreeNode, WorkSource)>,
    pub sampler: S,
}

/// Trait for SDF sampling (allows mocking in tests).
pub trait SdfSampler: Send + Sync {
    /// Sample SDF at world position.
    fn sample(&self, world_pos: [f64; 3]) -> SdfSample;
    /// Sample material at world position.
    fn sample_material(&self, world_pos: [f64; 3]) -> MaterialId;
}
```

### Stage 3: Meshing Types

```rust
/// Input for meshing a single node.
pub struct MeshInput {
    pub node: OctreeNode,
    pub volume: Box<[SdfSample; SAMPLE_SIZE_CB]>,
    pub materials: Box<[MaterialId; SAMPLE_SIZE_CB]>,
    pub config: MeshConfig,
    pub work_source: WorkSource,
}

/// Result of meshing a single node.
pub struct MeshResult {
    pub node: OctreeNode,
    pub output: MeshOutput,
    pub timing_us: u64,
    pub work_source: WorkSource,
}
```

### Stage 4: Composition Types

```rust
/// Mesh data for a single node within a group.
pub struct NodeMesh {
    pub node: OctreeNode,
    pub output: MeshOutput,
}

/// Group of related meshes (from a TransitionGroup).
pub struct GroupedMesh {
    /// Parent node (group_key from TransitionGroup)
    pub group_key: OctreeNode,
    /// The meshes in this group (1 for merge, 8 for subdivide)
    pub meshes: SmallVec<[NodeMesh; 9]>,
    /// Type of transition (determines presentation behavior)
    pub transition_type: TransitionType,
}

/// Input to composition stage.
pub struct CompositionInput {
    /// Mesh results to group
    pub mesh_results: Vec<MeshResult>,
    /// Transition groups from refinement (for grouping)
    pub transition_groups: Vec<TransitionGroup>,
}
```

### Stage 5: Presentation Types

```rust
/// How a chunk should be presented to the renderer.
#[derive(Clone, Debug)]
pub enum PresentationHint {
    /// Swap mesh immediately (invalidation)
    Immediate,
    /// Fade in as part of a subdivide (group_key = parent)
    FadeIn { group_key: OctreeNode },
    /// Fade out as part of a merge (group_key = parent)
    FadeOut { group_key: OctreeNode },
}

/// Byte-level mesh data for FFI.
#[derive(Clone)]
pub struct MeshData {
    pub vertices: Vec<u8>,
    pub indices: Vec<u8>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub bounds: MinMaxAABB,
}

/// Final output ready for rendering.
pub struct ReadyChunk {
    pub node: OctreeNode,
    pub mesh_data: MeshData,
    pub hint: PresentationHint,
}
```

---

## Test Categories by Stage

### STAGE 1: Refinement (Existing - verify coverage)

**Location:** `octree/refinement_test.rs`

Tests already exist. Verify these behaviors are covered:

| Test                                        | Behavior                       | Status |
|---------------------------------------------|--------------------------------|--------|
| `test_subdivide_produces_8_children`        | 1 parent creates 8 children    | EXISTS |
| `test_subdivide_removes_parent_from_leaves` | Parent removed after subdivide | EXISTS |
| `test_merge_produces_1_parent`              | 8 children collapse to parent  | EXISTS |
| `test_merge_removes_8_children_from_leaves` | Children removed after merge   | EXISTS |
| `test_no_subdivide_at_min_lod`              | LOD 0 cannot subdivide         | EXISTS |
| `test_merge_requires_all_8_siblings`        | Need all 8 for merge           | EXISTS |
| `test_max_transitions_budget_enforced`      | Budget limit respected         | EXISTS |

**Additional tests needed:**

```rust
// pipeline/refinement_test.rs (new file for pipeline integration)

#[test]
fn test_transition_group_to_prefilter_nodes() {
    // Subdivide group should produce 8 nodes for prefilter
    // Merge group should produce 1 node for prefilter
}

#[test]
fn test_empty_refinement_produces_empty_transitions() {
    // No viewer movement = no transitions
}
```

---

### STAGE 2: Prefilter

**Location:** `pipeline/prefilter_test.rs`

#### 2.1 Homogeneity Detection

```rust
// =============================================================================
// Batch 1: Corner Sampling
// =============================================================================

#[test]
fn test_all_corners_negative_returns_all_solid() {
    // SDF: -10 at all 8 corners of chunk bounds
    // Expected: PrefilterResult::Skip(SkipReason::AllSolid)
}

#[test]
fn test_all_corners_positive_returns_all_air() {
    // SDF: +10 at all 8 corners of chunk bounds
    // Expected: PrefilterResult::Skip(SkipReason::AllAir)
}

#[test]
fn test_mixed_corners_returns_volume() {
    // SDF: 4 negative, 4 positive corners
    // Expected: PrefilterResult::Volume { ... }
}

#[test]
fn test_single_corner_difference_samples_volume() {
    // 7 negative + 1 positive corner
    // Expected: Volume (surface might exist)
}

// =============================================================================
// Batch 2: Corner Positions
// =============================================================================

#[test]
fn test_corner_positions_at_lod_0() {
    // LOD 0, node at (0,0,0), voxel_size=1.0
    // Corners should be at world positions:
    //   (0,0,0), (32,0,0), (0,32,0), (32,32,0),
    //   (0,0,32), (32,0,32), (0,32,32), (32,32,32)
}

#[test]
fn test_corner_positions_at_lod_3() {
    // LOD 3, node at (1,1,1), voxel_size=1.0
    // Scale = 2^3 = 8
    // Base = (1*32*8, 1*32*8, 1*32*8) = (256, 256, 256)
    // Corners span 32*8 = 256 units
}

// =============================================================================
// Batch 3: Volume Sampling
// =============================================================================

#[test]
fn test_volume_samples_32_cubed_points() {
    // When corners indicate surface, full volume sampled
    // Verify volume.len() == 32768
}

#[test]
fn test_volume_samples_correct_world_positions() {
    // Sample at volume[coord_to_index(x,y,z)] should query
    // world position: node_base + [x,y,z] * voxel_size * lod_scale
}

#[test]
fn test_materials_sampled_alongside_sdf() {
    // Materials array populated in same pass
}
```

#### 2.2 Edge Cases

```rust
// =============================================================================
// Batch 4: Edge Cases
// =============================================================================

#[test]
fn test_zero_sdf_at_corner_counts_as_surface() {
    // SDF exactly 0 (on surface) should NOT skip
    // 7 corners at -10, 1 corner at exactly 0
    // Expected: Volume (surface at that corner)
}

#[test]
fn test_very_small_negative_still_solid() {
    // SDF = -0.001 at all corners
    // Expected: AllSolid (numerical precision)
}

#[test]
fn test_parallel_prefilter_many_nodes() {
    // 100 nodes prefiltered in parallel
    // Some skip, some produce volumes
    // Verify correct routing by work_source
}

#[test]
fn test_prefilter_preserves_work_source() {
    // REFINEMENT input → output has REFINEMENT
    // INVALIDATION input → output has INVALIDATION
}
```

---

### STAGE 3: Meshing

**Location:** `pipeline/meshing_test.rs`

Note: `surface_nets::generate()` already has tests. Focus on stage wrapper.

#### 3.1 Stage Wrapper Behavior

```rust
// =============================================================================
// Batch 1: Pure Function Wrapping
// =============================================================================

#[test]
fn test_mesh_stage_calls_surface_nets_generate() {
    // Input: Valid volume with sphere SDF
    // Output: Non-empty MeshOutput
}

#[test]
fn test_mesh_stage_preserves_node_identity() {
    // Input.node == Output.node
}

#[test]
fn test_mesh_stage_records_timing() {
    // timing_us > 0 for non-trivial volume
}

#[test]
fn test_mesh_stage_preserves_work_source() {
    // INVALIDATION in → INVALIDATION out
}

// =============================================================================
// Batch 2: Empty/Degenerate Cases
// =============================================================================

#[test]
fn test_empty_volume_produces_empty_mesh() {
    // All positive SDF → no surface
    // vertices.is_empty() == true
}

#[test]
fn test_all_solid_volume_produces_empty_mesh() {
    // All negative SDF → no surface
    // vertices.is_empty() == true
}

#[test]
fn test_single_surface_cell_produces_vertices() {
    // One cell with surface crossing
    // At least some vertices produced
}

// =============================================================================
// Batch 3: Parallel Execution
// =============================================================================

#[test]
fn test_parallel_meshing_produces_correct_count() {
    // 8 inputs → 8 outputs
}

#[test]
fn test_parallel_meshing_output_order_matches_input() {
    // Optional: deterministic ordering
    // Or: output contains all expected node IDs
}
```

---

### STAGE 4: Composition

**Location:** `pipeline/composition_test.rs`

#### 4.1 Grouping by TransitionGroup

```rust
// =============================================================================
// Batch 1: Subdivide Grouping (1 parent → 8 children)
// =============================================================================

#[test]
fn test_subdivide_groups_8_meshes_by_parent() {
    // Input: TransitionGroup::new_subdivide(parent)
    //        8 MeshResults for each child
    // Output: 1 GroupedMesh with 8 meshes
}

#[test]
fn test_subdivide_group_key_is_parent() {
    // GroupedMesh.group_key == parent from TransitionGroup
}

#[test]
fn test_subdivide_transition_type_preserved() {
    // GroupedMesh.transition_type == Subdivide
}

// =============================================================================
// Batch 2: Merge Grouping (8 children → 1 parent)
// =============================================================================

#[test]
fn test_merge_groups_1_mesh_with_refs() {
    // Input: TransitionGroup::new_merge(parent, children)
    //        1 MeshResult for parent
    // Output: 1 GroupedMesh with 1 mesh
}

#[test]
fn test_merge_group_key_is_parent() {
    // Same parent identification
}

#[test]
fn test_merge_transition_type_preserved() {
    // GroupedMesh.transition_type == Merge
}

// =============================================================================
// Batch 3: Multiple Groups
// =============================================================================

#[test]
fn test_multiple_transition_groups_produce_multiple_grouped_meshes() {
    // 3 subdivide groups → 3 GroupedMesh outputs
}

#[test]
fn test_meshes_not_matching_groups_are_ungrouped() {
    // Orphan mesh (no matching TransitionGroup)
    // Should this error? Or create singleton group?
}

// =============================================================================
// Batch 4: Invalidation Bypass
// =============================================================================

#[test]
fn test_invalidation_work_source_bypasses_composition() {
    // MeshResult with work_source=INVALIDATION
    // Should NOT be grouped, passed through directly
}

#[test]
fn test_mixed_work_sources_routed_correctly() {
    // 3 REFINEMENT meshes (grouped)
    // 2 INVALIDATION meshes (passthrough)
    // Output: grouped + ungrouped correctly separated
}
```

#### 4.2 Edge Cases

```rust
// =============================================================================
// Batch 5: Edge Cases
// =============================================================================

#[test]
fn test_missing_mesh_for_group_member() {
    // TransitionGroup expects 8 children
    // Only 7 meshes provided (1 skipped by prefilter)
    // Expected: GroupedMesh with 7 meshes, marker for missing
}

#[test]
fn test_empty_mesh_in_group() {
    // Child mesh is empty (all air after full sample)
    // Still included in group with empty marker
}

#[test]
fn test_composition_with_no_transitions() {
    // Empty transition_groups, but has meshes (invalidation only)
    // All meshes pass through ungrouped
}
```

---

### STAGE 5: Presentation

**Location:** `pipeline/presentation_test.rs`

#### 5.1 Hint Generation

```rust
// =============================================================================
// Batch 1: PresentationHint from GroupedMesh
// =============================================================================

#[test]
fn test_subdivide_produces_fade_in_hints() {
    // GroupedMesh with transition_type=Subdivide
    // Each child mesh → ReadyChunk with FadeIn { group_key }
}

#[test]
fn test_merge_produces_fade_out_hints() {
    // GroupedMesh with transition_type=Merge
    // Parent mesh → ReadyChunk with FadeOut { group_key }
    // (Children already exist in scene, no new chunks for them)
}

#[test]
fn test_invalidation_produces_immediate_hints() {
    // Ungrouped mesh with work_source=INVALIDATION
    // ReadyChunk with Immediate hint
}

// =============================================================================
// Batch 2: MeshData Serialization
// =============================================================================

#[test]
fn test_mesh_data_byte_format_correct_size() {
    // vertex_count * size_of::<Vertex>() == vertices.len()
    // index_count * size_of::<u32>() == indices.len()
}

#[test]
fn test_mesh_data_preserves_vertex_data() {
    // Deserialize vertices, check position matches original
}

#[test]
fn test_mesh_data_preserves_bounds() {
    // AABB correctly transferred
}

#[test]
fn test_empty_mesh_produces_empty_mesh_data() {
    // Empty MeshOutput → MeshData with vertex_count=0
}
```

#### 5.2 Edge Cases

```rust
// =============================================================================
// Batch 3: Edge Cases
// =============================================================================

#[test]
fn test_group_key_correct_in_all_chunks_of_group() {
    // All 8 children of a subdivide have same group_key
}

#[test]
fn test_presentation_order_preserved() {
    // Chunks output in deterministic order for consistent rendering
}
```

---

## Integration Tests

**Location:** `pipeline/integration_test.rs`

```rust
// =============================================================================
// Full Pipeline Integration
// =============================================================================

#[test]
fn test_full_pipeline_refinement_subdivide() {
    // 1. Create single node at LOD 5
    // 2. Viewer at node center triggers subdivide
    // 3. Prefilter samples all 8 children
    // 4. Meshing generates 8 meshes
    // 5. Composition groups into 1 GroupedMesh
    // 6. Presentation outputs 8 ReadyChunks with FadeIn
}

#[test]
fn test_full_pipeline_refinement_merge() {
    // 1. Create 8 children at LOD 4
    // 2. Viewer very far triggers merge
    // 3. Prefilter samples parent
    // 4. Meshing generates 1 mesh
    // 5. Composition groups into 1 GroupedMesh
    // 6. Presentation outputs 1 ReadyChunk with FadeOut
}

#[test]
fn test_full_pipeline_invalidation_bypass() {
    // 1. Create invalidation request for node
    // 2. Prefilter samples node
    // 3. Meshing generates mesh
    // 4. Composition BYPASSED (work_source check)
    // 5. Presentation outputs 1 ReadyChunk with Immediate
}

#[test]
fn test_mixed_refinement_and_invalidation() {
    // Both work sources in same frame
    // Verify correct routing and output
}

// =============================================================================
// Skip Propagation
// =============================================================================

#[test]
fn test_skipped_node_produces_no_ready_chunk() {
    // Prefilter returns Skip(AllAir)
    // No MeshResult, no GroupedMesh, no ReadyChunk
}

#[test]
fn test_partial_group_when_some_children_skip() {
    // Subdivide: 6 children have surface, 2 are all-air
    // GroupedMesh contains 6 meshes
    // Presentation outputs 6 ReadyChunks
}

// =============================================================================
// Performance Characteristics
// =============================================================================

#[test]
fn test_parallel_meshing_faster_than_sequential() {
    // Benchmark: 8 meshes parallel vs sequential
    // Parallel should be significantly faster on multi-core
}

#[test]
fn test_prefilter_avoids_unnecessary_sampling() {
    // Counting sampler: track sample() calls
    // All-air node should only sample 8 corners
    // Surface node should sample 32768 points
}
```

---

## Test Utilities

**Location:** `pipeline/test_utils.rs`

```rust
// =============================================================================
// Mock SDF Samplers
// =============================================================================

/// Sphere SDF centered at origin.
pub struct SphereSampler {
    pub center: DVec3,
    pub radius: f64,
}

impl SdfSampler for SphereSampler {
    fn sample(&self, pos: [f64; 3]) -> SdfSample {
        let p = DVec3::from_array(pos);
        let dist = (p - self.center).length() - self.radius;
        sdf_conversion::to_storage(dist as f32)
    }

    fn sample_material(&self, _pos: [f64; 3]) -> MaterialId {
        0 // Uniform material
    }
}

/// Constant SDF (all solid or all air).
pub struct ConstantSampler(pub SdfSample);

/// Checkerboard pattern for testing grouping.
pub struct CheckerboardSampler {
    pub cell_size: f64,
}

/// Counting sampler for verifying optimization.
pub struct CountingSampler<S: SdfSampler> {
    pub inner: S,
    pub sample_count: AtomicUsize,
}

// =============================================================================
// Test Fixtures
// =============================================================================

/// Create a TransitionGroup for subdivide testing.
pub fn subdivide_fixture(lod: i32) -> TransitionGroup {
    let parent = OctreeNode::new(0, 0, 0, lod);
    TransitionGroup::new_subdivide(parent).unwrap()
}

/// Create 8 mock MeshResults for children of a parent.
pub fn child_mesh_results(parent: &OctreeNode) -> Vec<MeshResult> {
    (0..8u8)
        .map(|octant| {
            let child = parent.get_child(octant).unwrap();
            MeshResult {
                node: child,
                output: make_sphere_mesh(),
                timing_us: 100,
                work_source: WorkSource::Refinement,
            }
        })
        .collect()
}

/// Create a sphere mesh output for testing.
pub fn make_sphere_mesh() -> MeshOutput {
    // Use existing make_sphere_volume() pattern
    let (volume, materials) = make_sphere_volume();
    surface_nets::generate(&volume, &materials, &MeshConfig::default())
}
```

---

## Test Organization

```
crates/voxel_plugin/src/
├── pipeline/
│   ├── mod.rs                    # Module declarations, stage traits
│   ├── types.rs                  # I/O type definitions
│   ├── prefilter.rs              # Stage 2 implementation
│   ├── prefilter_test.rs         # Stage 2 tests
│   ├── meshing.rs                # Stage 3 wrapper (uses surface_nets)
│   ├── meshing_test.rs           # Stage 3 tests
│   ├── composition.rs            # Stage 4 implementation
│   ├── composition_test.rs       # Stage 4 tests
│   ├── presentation.rs           # Stage 5 implementation
│   ├── presentation_test.rs      # Stage 5 tests
│   ├── integration_test.rs       # Full pipeline tests
│   └── test_utils.rs             # Shared test utilities
```

---

## Implementation Order (TDD Red-Green-Refactor)

1. **Types first** (`types.rs`)

- Define all I/O structs
- No logic, just data structures
- No tests needed (data-only)

2. **Test utilities** (`test_utils.rs`)

- Mock samplers
- Fixture generators
- Run to verify utilities compile

3. **Stage 2: Prefilter** (most logic)

- Write tests in `prefilter_test.rs`
- Implement `prefilter.rs` until tests pass

4. **Stage 3: Meshing** (thin wrapper)

- Write tests in `meshing_test.rs`
- Implement `meshing.rs` wrapper

5. **Stage 4: Composition** (grouping logic)

- Write tests in `composition_test.rs`
- Implement `composition.rs`

6. **Stage 5: Presentation** (serialization)

- Write tests in `presentation_test.rs`
- Implement `presentation.rs`

7. **Integration tests** (full pipeline)

- Write tests in `integration_test.rs`
- Wire up module exports in `mod.rs`

---

## Estimated Test Count

| Stage        | Tests   | Complexity              |
|--------------|---------|-------------------------|
| Prefilter    | ~12     | Medium (sampling logic) |
| Meshing      | ~8      | Low (wrapper)           |
| Composition  | ~12     | Medium (grouping)       |
| Presentation | ~10     | Low (serialization)     |
| Integration  | ~8      | High (full pipeline)    |
| **Total**    | **~50** |                         |

---

## Open Questions

1. **Missing mesh in group**: Should composition fail, or mark slot as empty?

- Recommendation: Mark as empty, renderer handles missing chunks

2. **Orphan meshes**: MeshResult with no matching TransitionGroup?

- Recommendation: Error for REFINEMENT, allowed for INVALIDATION

3. **Output ordering**: Should ReadyChunks be sorted by priority?

- Recommendation: Yes, by distance from viewer (closest first)

4. **Timing aggregation**: Per-stage timing or per-chunk?

- Recommendation: Both - stage-level for profiling, chunk-level for debugging