# Voxel Bridge Framework Design

Reference document for engine integration bridges. All bridges must implement this contract consistently.

## Core Principle

**The bridge is a thin adapter layer.** It translates between `voxel_plugin` abstractions and engine-specific APIs. It does NOT add logic, filtering, or state beyond what's needed for translation.

---

## Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│                    Consuming Game                       │
│              (shaders, materials, gameplay)             │
├─────────────────────────────────────────────────────────┤
│                    Engine Bridge                        │
│         voxel_bevy (native) / voxel_unity (FFI)         │
├─────────────────────────────────────────────────────────┤
│                    voxel_plugin                         │
│     (refinement, meshing, pipeline - engine-agnostic)   │
└─────────────────────────────────────────────────────────┘
```

---

## Bridge Contract

Every bridge MUST implement these responsibilities:

### 1. World Lifecycle

| Operation | Description |
|-----------|-------------|
| `create_world(config, sampler)` | Initialize world with config and volume sampler |
| `destroy_world(world_id)` | Cleanup all state for a world |
| `get_initial_leaves(config)` | Compute starting leaves from world bounds |

### 2. Refinement Entry Point

The bridge uses `VoxelWorld::refine()` for LOD updates:

```rust
// Rust (native)
let output = world.refine(viewer_pos: DVec3) -> RefinementOutput

// FFI
voxel_world_refine(world_id, viewer_x, viewer_y, viewer_z, &out_transitions) -> status
```

**Consumer perspective:**
```
Each frame:
  1. Get viewer position in local space
  2. Call world.refine(viewer_pos)
  3. Generate meshes for nodes in transition_groups.nodes_to_add
  4. Apply spawn/despawn operations
```

The bridge handles mesh generation and entity management.

### PresentationBatch (Standardized)

All bridges return the same logical structure:

```rust
struct PresentationBatch {
    /// Chunks to remove first (despawn before spawn for atomicity)
    pub to_despawn: Vec<ChunkKey>,

    /// Chunks to spawn after despawn
    pub to_spawn: Vec<ChunkMesh>,
}

struct ChunkKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub lod: i32,
}

struct ChunkMesh {
    pub key: ChunkKey,
    pub position: [f64; 3],  // Local-space position (node_min)
    pub scale: f64,          // Uniform scale (voxel_size at lod)
    pub vertices: &[Vertex], // Vertex data
    pub indices: &[u16],     // Index data
}
```

**Notes:**
- `to_despawn` processed before `to_spawn` (atomic transitions)
- Position/scale pre-calculated by bridge (consumer just uses them)
- Vertex format is engine-agnostic (position, normal, material weights)
- FFI bridges use pointers; native bridges use slices

### 3. Refinement Pipeline

The refinement and mesh generation flow:

```
┌─────────┐    ┌───────────┐    ┌──────┐    ┌─────────┐
│ Refine  │───▶│ Presample │───▶│ Mesh │───▶│ Present │
└─────────┘    └───────────┘    └──────┘    └─────────┘
                     │                            │
                     └── parallel via rayon ──────┘
```

**Refinement is synchronous.** The `refine()` call computes LOD transitions based on viewer position and budget. Mesh generation can be parallelized via rayon.

**Internal flow:**
```rust
fn refine(&mut self, viewer_pos: DVec3) -> RefinementOutput {
    let input = RefinementInput {
        viewer_pos,
        config: self.config.clone(),
        prev_leaves: self.leaves.as_set().clone(),
        budget: self.budget,
    };

    let output = octree::refine(input);

    // Update leaves to match refinement output
    self.leaves = OctreeLeaves::from(output.next_leaves.clone());

    output
}
```

The bridge then generates meshes for `output.transition_groups` nodes in parallel.

### 4. Entity/Chunk Presentation

| Operation | Description |
|-----------|-------------|
| `spawn_chunk(node, mesh, transform)` | Create renderable from MeshOutput |
| `despawn_chunk(node)` | Destroy renderable |
| `apply_transition_group(group)` | Atomic: despawn old + spawn new in same frame |

**Transform calculation (MUST be consistent):**
```rust
position = config.get_node_min(&node)  // World-space corner
scale    = config.get_voxel_size(node.lod)  // Uniform scale
```

### 5. Chunk Registry

Track `(WorldId, OctreeNode) → EngineEntity` mapping for:
- O(1) lookup during despawn
- World cleanup on destroy
- Debug visualization

---

## Coordinate Spaces

The voxel system operates entirely in **local space**. The engine bridge handles space translation.

```
┌─────────────────────────────────────────────────────────┐
│                    Engine World Space                    │
│     (viewer at world position, voxel root transformed)  │
├─────────────────────────────────────────────────────────┤
│                    Bridge Translation                    │
│         viewer_local = root.inverse_transform(viewer)   │
├─────────────────────────────────────────────────────────┤
│                    Voxel Local Space                     │
│     (refinement, sampling, meshing all happen here)     │
└─────────────────────────────────────────────────────────┘
```

### Rules

1. **Voxel world is local-space only.** All octree coordinates, sampling, and mesh vertices are in local space relative to `world_origin` (typically zero).

2. **Engine applies affine transform.** The voxel root entity can be translated, rotated, and scaled in the engine. Chunk entities inherit this transform.

3. **Viewer position must be localized.** Before calling `refine()`, the bridge MUST transform the viewer's world position into voxel local space:

```rust
// Bevy example
let viewer_world = viewer_transform.translation();
let root_transform = root_global_transform;
let viewer_local = root_transform.affine().inverse().transform_point3(viewer_world);
```

```csharp
// Unity example (C#)
Vector3 viewerWorld = viewer.position;
Vector3 viewerLocal = voxelRoot.InverseTransformPoint(viewerWorld);
```

4. **Mesh vertices stay local.** The mesher outputs vertices in `[0, ~31]` local coords. The engine transform (on root or chunk entities) places them in world space. Do NOT bake world positions into vertex data.

### Why Local Space?

- **Double precision:** LOD thresholds use `DVec3` for huge worlds without precision loss
- **Movable worlds:** Entire voxel terrain can be repositioned without regenerating meshes
- **Multi-world:** Multiple voxel worlds with different transforms coexist cleanly

---

## State Ownership

| State | Owner | Notes |
|-------|-------|-------|
| `OctreeConfig` | Bridge | Immutable after creation |
| `leaves: HashSet<OctreeNode>` | Bridge | Updated by refine() output |
| `sampler: impl VolumeSampler` | Bridge | Passed to pipeline |
| Mesh data | Transient | Consumed during spawn, then discarded |
| Entity references | Bridge | Engine-specific handles |
| Root transform | Engine | Affine transform for world placement |

---

## Reference Implementation: voxel_bevy

### Module Structure

```
voxel_bevy/
├── world.rs          # VoxelWorldRoot, WorldChunkMap, sync_world_transforms
├── entity_queue.rs   # Atomic transition batching
├── systems/
│   └── entities.rs   # spawn_chunk_entity(), mesh_output_to_bevy()
├── components.rs     # VoxelChunk, VoxelViewer markers
├── resources.rs      # VoxelMetricsResource
└── debug_ui.rs       # Debug overlay (feature-gated)
```

### Key Patterns

**1. VoxelWorldRoot wraps VoxelWorld:**
```rust
#[derive(Component)]
pub struct VoxelWorldRoot {
    pub world: VoxelWorld<Box<dyn VolumeSampler>>,
}
```

**2. EntityQueue for atomic transitions:**
```rust
queue.queue_transitions(result.transitions);
queue.process_frame(|transition| {
    // Despawn + spawn in same frame
});
```

**3. WorldChunkMap for O(1) lookup:**
```rust
worlds: HashMap<WorldId, HashMap<OctreeNode, Entity>>
```

---

## FFI Bridge Requirements (voxel_unity)

FFI bridges have additional constraints:

### Synchronous Update Model

voxel_unity uses **synchronous refinement** with **parallel mesh generation** via rayon:

```rust
fn update(&mut self, viewer_pos: DVec3) -> bool {
    // 1. Run sync refine()
    let output = refine(RefinementInput { viewer_pos, ... });

    // 2. Generate meshes in parallel (rayon)
    let meshes = nodes_to_mesh.par_iter()
        .filter_map(|node| sample_and_mesh(node))
        .collect();

    // 3. Build FFI batch immediately
    self.build_ffi_batch(output, meshes);

    true // Events always ready after update
}
```

This mirrors the Bevy approach: sync refinement decisions, parallel mesh generation.

### Memory Safety
- Mesh data must be retained until engine copies it
- Use `RetainedTransitionGroup` to hold data across FFI boundary
- Pointers are valid until the next `voxel_world_update()` call

### Batch Structure
```c
struct FfiPresentationBatch {
    groups: *const FfiTransitionGroup,
    groups_count: u32,
}

struct FfiTransitionGroup {
    group_key: FfiChunkKey,
    is_collapse: u8,  // 1 = merge, 0 = subdivide
    to_remove: *const FfiChunkKey,
    to_remove_count: u32,
    to_add: *const FfiChunkPresentation,  // Includes mesh data
    to_add_count: u32,
}
```

### Update Protocol
```
1. Engine calls voxel_world_update(world_id, viewer_pos, &out_batch)
2. Returns: 0 = no transitions needed, 1 = batch ready, <0 = error
3. Engine processes batch (create/destroy GameObjects)
4. Next update() invalidates previous batch pointers
```

**Note:** Unlike async models, the update call blocks until refinement and mesh generation complete. For very large transitions, this may cause a frame spike - use `RefinementBudget` to limit work per call.

---

## Invariants (MUST NOT violate)

1. **Leaves == Truth:** The `leaves` set is the authoritative state. Entity count must match.

2. **Atomic Transitions:** Never show partial transition (e.g., parent gone but children not yet spawned).

3. **No Logic in Bridge:** Bridge translates, doesn't decide. All LOD decisions come from `refine()`.

4. **Consistent Transforms:** All bridges use same formula: `position = node_min`, `scale = voxel_size_at_lod`.

5. **Sampler Purity:** Same (node, sampler, config) → same mesh. No bridge-specific sampling.

---

## Debugging Checklist

When a bridge misbehaves:

| Symptom | Check |
|---------|-------|
| Chunks don't spawn | Is leaves set populated? Is spawn_chunk called? |
| Chunks don't despawn | Is transition.nodes_to_remove processed? Entity lookup working? |
| Visual pops | Are transitions applied atomically? (despawn+spawn same frame) |
| Never merges | Is `all_children_are_leaves()` passing? Are merge candidates generated? |
| Wrong positions | Is transform using `get_node_min()` and `get_voxel_size()`? |

### Instrumentation Points

Add logging at:
1. `refine()` output: transition count, subdivide vs collapse
2. `leaves` mutations: before/after size
3. Entity operations: spawn/despawn with node coordinates
4. FFI boundary: batch contents before return

---

## Metrics Contract

Core metrics from voxel_plugin, accessed via `voxel_world_get_metrics()`.

### Overview

The voxel plugin provides engine-agnostic metrics collection through the `metrics` feature.
Timing is collected in 128-sample rolling windows, and operation counts are cumulative.

```rust
// Rust (native) - access via world.metrics field
let snapshot = world.metrics.snapshot();
println!("Refine avg: {}µs", snapshot.refine.avg_us);

// FFI - call voxel_world_get_metrics()
FfiMetricsSnapshot snapshot;
voxel_world_get_metrics(world_id, &snapshot);
printf("Mesh avg: %lluµs\n", snapshot.mesh.avg_us);
```

### Timing Stats

Each timing category provides histogram stats from a 128-sample rolling window:

| Field | Type | Description |
|-------|------|-------------|
| `last_us` | u64 | Most recent timing sample |
| `avg_us` | u64 | Rolling average (~2s at 60fps) |
| `min_us` | u64 | Minimum in window |
| `max_us` | u64 | Maximum in window |
| `sample_count` | u32 | Samples collected (up to 128) |

### Timing Categories

| Category | Description | What's Timed |
|----------|-------------|--------------|
| `refine` | Octree refinement | LOD decision, transition group computation |
| `mesh` | Mesh generation batch | Presample + surface nets + composition |
| `sample` | Volume sampling | Reserved for per-sample timing |

### Operation Counts (Cumulative)

| Counter | Description |
|---------|-------------|
| `total_refine_calls` | Number of `refine()` invocations |
| `total_chunks_meshed` | Chunks processed through pipeline |
| `total_transitions` | Transition groups processed |

### FFI Types

```c
// C/C# struct (matches Rust repr(C))
typedef struct {
    uint64_t last_us;
    uint64_t avg_us;
    uint64_t min_us;
    uint64_t max_us;
    uint32_t sample_count;
    uint32_t _pad;
} FfiTimingStats;

typedef struct {
    FfiTimingStats refine;
    FfiTimingStats mesh;
    FfiTimingStats sample;
    uint64_t total_refine_calls;
    uint64_t total_chunks_meshed;
    uint64_t total_transitions;
} FfiMetricsSnapshot;
```

### Usage Example (Unity C#)

```csharp
[DllImport("voxel_unity")]
private static extern int voxel_world_get_metrics(int worldId, ref FfiMetricsSnapshot snapshot);

// In your update/debug loop:
FfiMetricsSnapshot metrics = default;
if (voxel_world_get_metrics(worldId, ref metrics) == 0)
{
    Debug.Log($"Refine: {metrics.refine.last_us}µs (avg {metrics.refine.avg_us}), " +
              $"Mesh: {metrics.mesh.last_us}µs (avg {metrics.mesh.avg_us})");
}
```

### Feature Gating

Metrics collection is controlled by the `metrics` feature flag:

- **voxel_plugin**: `cargo build --features metrics`
- **voxel_unity**: Enabled by default (`default = ["metrics"]`)

When disabled, `voxel_world_get_metrics()` returns -4 (feature not enabled).
