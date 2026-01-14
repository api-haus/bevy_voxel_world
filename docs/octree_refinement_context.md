# Octree Refinement Context for Rust Implementation

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Bounded world | **Implicit bounds** | 31 LOD levels cover enormous range; no explicit root needed |
| Quality control | **LOD exponent** | Scales distance thresholds (simple, effective) |
| Floating origin | **Future integration point** | Affects sampling origin, not octree structure |
| Coordinate system | **Node coords at own LOD** | Simpler than finest-LOD alignment approaches |

## LOD Convention

**LOD 0 = finest detail (smallest cells), higher LOD = coarser**

```
Cell Size = VOXELS_PER_CELL × voxel_size × 2^LOD
         = 28 × voxel_size × 2^LOD

LOD 0:  28 × voxel_size × 1  = 28 voxel_size units
LOD 1:  28 × voxel_size × 2  = 56 voxel_size units
LOD 10: 28 × voxel_size × 1024 = 28,672 voxel_size units
LOD 20: 28 × voxel_size × 1M   = ~29M voxel_size units
LOD 30: 28 × voxel_size × 1B   = ~30B voxel_size units
```

With `voxel_size = 1.0`, LOD 30 covers ~30 billion units per axis.

## Volume Constants (from C#)

```rust
/// Centralized constants for SDF volume layout.
/// 32³ samples with 28 interior cells + apron + displacement padding.
pub mod volume {
    // ═══════════════════════════════════════════════════════════════════
    // Sample Grid (must be 32 for bit shift operations)
    // ═══════════════════════════════════════════════════════════════════

    /// Number of samples per axis (32).
    pub const SAMPLE_SIZE: usize = 32;

    /// SAMPLE_SIZE squared (1024).
    pub const SAMPLE_SIZE_SQ: usize = SAMPLE_SIZE * SAMPLE_SIZE;

    /// SAMPLE_SIZE cubed (32768).
    pub const SAMPLE_SIZE_CUBED: usize = SAMPLE_SIZE_SQ * SAMPLE_SIZE;

    /// Maximum valid sample index (31).
    pub const MAX_SAMPLE_INDEX: usize = SAMPLE_SIZE - 1;

    // ═══════════════════════════════════════════════════════════════════
    // Bit Shifts for 3D Indexing: index = x << X_SHIFT | y << Y_SHIFT | z
    // ═══════════════════════════════════════════════════════════════════

    /// Bit shift for Y component: log2(32) = 5.
    pub const Y_SHIFT: usize = 5;

    /// Bit shift for X component: log2(32²) = 10.
    pub const X_SHIFT: usize = 10;

    /// Mask for extracting single axis index: 31 (0x1F).
    pub const INDEX_MASK: usize = SAMPLE_SIZE - 1;

    // ═══════════════════════════════════════════════════════════════════
    // Interior Cell Range (the actual mesh output)
    // ═══════════════════════════════════════════════════════════════════

    /// Number of interior cells per axis that produce mesh geometry (28).
    pub const INTERIOR_CELLS: usize = 28;

    /// First interior cell index (1).
    pub const FIRST_INTERIOR_CELL: usize = 1;

    /// Last interior cell index (28).
    pub const LAST_INTERIOR_CELL: usize = 28;

    // ═══════════════════════════════════════════════════════════════════
    // Apron and Padding
    // ═══════════════════════════════════════════════════════════════════

    /// Number of apron samples on negative boundary (1).
    pub const NEGATIVE_APRON: usize = 1;

    /// Number of padding samples for displacement on positive boundary (2).
    pub const DISPLACEMENT_PADDING: usize = 2;

    /// Last sample index used by interior cells (29).
    pub const LAST_INTERIOR_SAMPLE: usize = LAST_INTERIOR_CELL + 1;

    // ═══════════════════════════════════════════════════════════════════
    // World Size - THIS IS THE KEY CONSTANT FOR CELL SIZING
    // ═══════════════════════════════════════════════════════════════════

    /// Number of voxels per cell for world size calculations.
    /// Cell world size = voxel_size * VOXELS_PER_CELL * 2^LOD
    pub const VOXELS_PER_CELL: usize = INTERIOR_CELLS; // 28

    /// Convert 3D coords to flat index
    #[inline]
    pub const fn to_index(x: usize, y: usize, z: usize) -> usize {
        (x << X_SHIFT) | (y << Y_SHIFT) | z
    }

    /// Extract X from flat index
    #[inline]
    pub const fn from_index_x(index: usize) -> usize {
        (index >> X_SHIFT) & INDEX_MASK
    }

    /// Extract Y from flat index
    #[inline]
    pub const fn from_index_y(index: usize) -> usize {
        (index >> Y_SHIFT) & INDEX_MASK
    }

    /// Extract Z from flat index
    #[inline]
    pub const fn from_index_z(index: usize) -> usize {
        index & INDEX_MASK
    }
}
```

**Volume Layout Diagram:**
```
Sample index:  0     1     2    ...    27    28    29    30    31
               │     │                       │     │     │     │
               │     └───── 28 interior ─────┘     │     │     │
               │           cells (1-28)            │     │     │
               │                                   │     └─────┴───── displacement
               │                                   │                  padding
               └─ negative                         └─ last interior
                  apron                               sample (29)
```

## OctreeNode Structure

```rust
/// Octree node - immutable value type
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OctreeNode {
    /// Grid position at this node's LOD level
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Level of detail (0 = finest, higher = coarser)
    pub lod: i32,
}

impl OctreeNode {
    /// Get child node (finer detail: LOD - 1)
    /// Octant: 0-7 where bits represent +X, +Y, +Z offsets
    pub fn get_child(&self, octant: u8) -> Option<Self> {
        if self.lod <= 0 { return None; }
        let cx = (octant & 1) as i32;
        let cy = ((octant >> 1) & 1) as i32;
        let cz = ((octant >> 2) & 1) as i32;
        Some(Self {
            x: self.x * 2 + cx,
            y: self.y * 2 + cy,
            z: self.z * 2 + cz,
            lod: self.lod - 1,
        })
    }

    /// Get parent node (coarser: LOD + 1)
    pub fn get_parent(&self, max_lod: i32) -> Option<Self> {
        if self.lod >= max_lod { return None; }
        Some(Self {
            x: self.x / 2,
            y: self.y / 2,
            z: self.z / 2,
            lod: self.lod + 1,
        })
    }
}
```

## Implicit Bounds Design

With 31 LOD levels, the coordinate space is effectively unbounded:

| LOD | Cell Size (voxel_size=1) | Coverage per axis |
|-----|--------------------------|-------------------|
| 0 | 28 | 28 units |
| 10 | 28,672 | ~29K units |
| 20 | 29.4M | ~29M units |
| 30 | 30.1B | ~30B units |

**Design:** Leaves define the tree implicitly - no explicit root needed.

```rust
/// Implicit octree - leaves ARE the state
pub struct OctreeLeaves {
    leaves: HashSet<OctreeNode>,
}

impl OctreeLeaves {
    /// Initialize with a single node at given LOD
    pub fn new_with_initial(lod: i32) -> Self {
        let mut leaves = HashSet::new();
        leaves.insert(OctreeNode { x: 0, y: 0, z: 0, lod });
        Self { leaves }
    }

    /// Find the effective max LOD (coarsest node in leaves)
    pub fn effective_max_lod(&self) -> i32 {
        self.leaves.iter().map(|n| n.lod).max().unwrap_or(0)
    }
}
```

**Key insight:** Parent/child relationships are computed on-demand via coordinate math. No tree structure to maintain - just a set of leaf nodes.

**Initialization:** Start with a single node at an appropriate LOD for initial world view. Refinement will subdivide/merge as viewer moves.

## Transition Groups

A transition group represents an atomic octree state change:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransitionType {
    Subdivide, // 1 parent → 8 children
    Merge,     // 8 children → 1 parent
}

pub struct TransitionGroup {
    /// Type of transition
    pub transition_type: TransitionType,

    /// Key: the parent node (for both subdivide and merge)
    /// - Subdivide: parent being replaced by children
    /// - Merge: parent being created from children
    pub group_key: OctreeNode,

    /// Nodes to add to octree leaves (will need sampling/meshing)
    /// - Subdivide: 8 children
    /// - Merge: 1 parent
    pub nodes_to_add: SmallVec<[OctreeNode; 8]>,

    /// Nodes to remove from renderer
    /// - Subdivide: 1 parent
    /// - Merge: 8 children
    pub nodes_to_remove: SmallVec<[OctreeNode; 8]>,

    /// Neighbor LOD masks (parallel to nodes_to_add)
    /// Computed after refinement, before meshing
    pub neighbor_masks: SmallVec<[u32; 8]>,
}
```

### Transition Group Invariants

1. **Group key is always the parent node**
2. **Subdivide:** `nodes_to_add.len() == 8`, `nodes_to_remove.len() == 1`
3. **Merge:** `nodes_to_add.len() == 1`, `nodes_to_remove.len() == 8`
4. **Atomic:** All nodes in a group transition together

## Refinement Algorithm

```rust
pub struct RefinementInput {
    pub viewer_pos: DVec3,           // Double precision for huge worlds
    pub config: OctreeConfig,
    pub prev_leaves: HashSet<OctreeNode>,
    pub max_transitions_per_frame: usize,
}

pub struct RefinementOutput {
    pub next_leaves: HashSet<OctreeNode>,
    pub transition_groups: Vec<TransitionGroup>,
}

pub fn refine(input: RefinementInput) -> RefinementOutput {
    let mut next_leaves = input.prev_leaves.clone();
    let mut to_subdivide: Vec<OctreeNode> = Vec::new();
    let mut coarsen_candidates: HashSet<OctreeNode> = HashSet::new();

    // Phase 1: Identify candidates
    for node in &input.prev_leaves {
        // Check subdivision (LOD > MinLOD)
        if node.lod > input.config.min_lod {
            let center = node.get_center(&input.config);
            let dist = input.viewer_pos.distance(center);
            let threshold = input.config.get_threshold(node.lod);

            if dist < threshold {
                to_subdivide.push(*node);
                continue;
            }
        }

        // Check coarsening (LOD < MaxLOD)
        if node.lod < input.config.max_lod {
            if let Some(parent) = node.get_parent(input.config.max_lod) {
                let parent_center = parent.get_center(&input.config);
                let parent_dist = input.viewer_pos.distance(parent_center);
                let parent_threshold = input.config.get_threshold(parent.lod);

                if parent_dist >= parent_threshold {
                    coarsen_candidates.insert(parent);
                }
            }
        }
    }

    // Phase 2: Validate coarsening (all 8 children must be leaves)
    let valid_coarsen: Vec<_> = coarsen_candidates
        .into_iter()
        .filter(|parent| all_children_are_leaves(parent, &next_leaves))
        .collect();

    // Phase 3: Sort by priority
    // Subdivisions: closest first (highest priority)
    to_subdivide.sort_by(|a, b| {
        let da = input.viewer_pos.distance_squared(a.get_center(&input.config));
        let db = input.viewer_pos.distance_squared(b.get_center(&input.config));
        da.partial_cmp(&db).unwrap()
    });

    // Collapses: farthest first (shed distant load)
    let mut valid_coarsen = valid_coarsen;
    valid_coarsen.sort_by(|a, b| {
        let da = input.viewer_pos.distance_squared(a.get_center(&input.config));
        let db = input.viewer_pos.distance_squared(b.get_center(&input.config));
        db.partial_cmp(&da).unwrap() // Reversed!
    });

    let mut remaining_budget = input.max_transitions_per_frame;
    let mut transition_groups = Vec::new();

    // Phase 4: Apply collapses first (shed load)
    for parent in valid_coarsen.into_iter().take(remaining_budget) {
        apply_merge(&parent, &mut next_leaves, &mut transition_groups);
        remaining_budget -= 1;
    }

    // Phase 5: Apply subdivisions
    for node in to_subdivide.into_iter().take(remaining_budget) {
        apply_subdivide(&node, &mut next_leaves, &mut transition_groups);
    }

    // Sort transition groups by proximity (for presentation priority)
    transition_groups.sort_by(|a, b| {
        let da = input.viewer_pos.distance_squared(a.group_key.get_center(&input.config));
        let db = input.viewer_pos.distance_squared(b.group_key.get_center(&input.config));
        da.partial_cmp(&db).unwrap()
    });

    RefinementOutput { next_leaves, transition_groups }
}
```

## OctreeConfig

```rust
/// Configuration for octree refinement and world coordinate mapping.
#[derive(Clone, Debug)]
pub struct OctreeConfig {
    // ═══════════════════════════════════════════════════════════════════
    // Core Parameters
    // ═══════════════════════════════════════════════════════════════════

    /// Base voxel size in world units.
    /// All cell sizes derive from this: cell_size = voxel_size * VOXELS_PER_CELL * 2^LOD
    pub voxel_size: f64,

    /// World-space origin for coordinate calculations.
    /// Node world position = world_origin + grid_position * cell_size
    ///
    /// **Floating Origin Integration:** When implementing floating origin,
    /// this becomes the offset from true world origin. Sampling positions
    /// are computed relative to this origin.
    pub world_origin: DVec3,

    // ═══════════════════════════════════════════════════════════════════
    // LOD Bounds
    // ═══════════════════════════════════════════════════════════════════

    /// Finest LOD level (highest detail). Typically 0.
    pub min_lod: i32,

    /// Coarsest LOD level allowed. With 31 levels and voxel_size=1.0,
    /// LOD 30 covers ~30 billion units.
    pub max_lod: i32,

    // ═══════════════════════════════════════════════════════════════════
    // LOD Control
    // ═══════════════════════════════════════════════════════════════════

    /// LOD exponent: scales distance thresholds.
    /// Higher values = more aggressive LOD (coarser at same distance).
    /// Lower values = more detail at distance.
    ///
    /// threshold = cell_size * 2^lod_exponent
    ///
    /// Typical values: -1.0 (fine), 0.0 (default), 1.0 (coarse)
    pub lod_exponent: f64,
}

impl OctreeConfig {
    /// Calculate cell size at given LOD.
    /// cell_size = voxel_size * VOXELS_PER_CELL * 2^LOD
    #[inline]
    pub fn get_cell_size(&self, lod: i32) -> f64 {
        self.voxel_size * (volume::VOXELS_PER_CELL as f64) * (1u64 << lod) as f64
    }

    /// Calculate voxel size at given LOD (for sampling).
    /// voxel_at_lod = voxel_size * 2^LOD
    #[inline]
    pub fn get_voxel_size(&self, lod: i32) -> f64 {
        self.voxel_size * (1u64 << lod) as f64
    }

    /// Calculate refinement threshold for LOD.
    /// Node subdivides when viewer_distance < threshold.
    ///
    /// threshold = cell_size * lod_scale
    /// where lod_scale = 2^lod_exponent
    #[inline]
    pub fn get_threshold(&self, lod: i32) -> f64 {
        let cell_size = self.get_cell_size(lod);
        let lod_scale = 2.0_f64.powf(self.lod_exponent);
        cell_size * lod_scale
    }

    /// Get world-space minimum corner of a node.
    #[inline]
    pub fn get_node_min(&self, node: &OctreeNode) -> DVec3 {
        let cell_size = self.get_cell_size(node.lod);
        self.world_origin + DVec3::new(
            node.x as f64 * cell_size,
            node.y as f64 * cell_size,
            node.z as f64 * cell_size,
        )
    }

    /// Get world-space center of a node.
    #[inline]
    pub fn get_node_center(&self, node: &OctreeNode) -> DVec3 {
        let cell_size = self.get_cell_size(node.lod);
        self.get_node_min(node) + DVec3::splat(cell_size * 0.5)
    }
}

impl Default for OctreeConfig {
    fn default() -> Self {
        Self {
            voxel_size: 1.0,
            world_origin: DVec3::ZERO,
            min_lod: 0,
            max_lod: 30,
            lod_exponent: 0.0, // lod_scale = 1.0
        }
    }
}
```

### LOD Exponent Effect

| lod_exponent | lod_scale | Effect |
|--------------|-----------|--------|
| -2.0 | 0.25 | 4× more detail (fine) |
| -1.0 | 0.5 | 2× more detail |
| 0.0 | 1.0 | Default |
| 1.0 | 2.0 | 2× less detail (coarse) |
| 2.0 | 4.0 | 4× less detail |

### Floating Origin Integration Point

When integrating floating origin:

```rust
/// Update world origin when floating origin shifts
fn on_floating_origin_shift(&mut self, shift: DVec3) {
    self.world_origin -= shift;
}
```

The octree structure (node coordinates) remains unchanged. Only `world_origin` shifts, which affects:
1. `get_node_min()` / `get_node_center()` calculations
2. Sampling world positions
3. Distance calculations for LOD decisions

## Presentation Layer: Atomic Group Processing

### Problem: Render Voids

If meshes are added/removed individually, timing mismatches cause visible holes.

### Solution: Atomic 9-Node Groups

Process complete transition groups atomically in a single frame. No deferred operations, no phantom tracking.

```rust
/// A complete transition ready for presentation.
/// All meshes have been generated - apply atomically.
pub struct ReadyTransitionGroup {
    pub transition_type: TransitionType,

    /// Meshes to add (8 for subdivide, 1 for merge)
    /// Empty meshes included as markers (no geometry but still "present")
    pub meshes_to_add: SmallVec<[GeneratedMesh; 8]>,

    /// Nodes to remove (1 for subdivide, 8 for merge)
    /// Removal is no-op if node wasn't rendered - that's fine
    pub nodes_to_remove: SmallVec<[OctreeNode; 8]>,
}

impl PresentationLayer {
    /// Apply a complete transition group atomically.
    /// Called only when ALL meshes in the group are ready.
    pub fn apply_group(&mut self, group: ReadyTransitionGroup) {
        // Step 1: Add all new meshes
        for mesh in group.meshes_to_add {
            self.renderer.add(mesh);
        }

        // Step 2: Remove old nodes (no-op if not present)
        for node in group.nodes_to_remove {
            self.renderer.remove(&node); // Silent no-op if missing
        }
    }
}
```

### Key Invariants

1. **Never apply partial groups** - wait for all meshes to be generated
2. **Add before remove** - ensures coverage, no voids
3. **Removals are no-op safe** - missing node = already handled
4. **Single frame** - entire group processes atomically

### Pipeline Flow

```
TransitionGroup (from refinement)
    ↓
[Mesh generation for all nodes_to_add]
    ↓
ReadyTransitionGroup (all meshes complete)
    ↓
PresentationLayer.apply_group() (atomic, single frame)
```

The mesh generation stage buffers incomplete groups. Only when all 8 children (subdivide) or 1 parent (merge) have meshes ready does it emit a `ReadyTransitionGroup`.

## Edge Cases

### 1. Empty SDF Volumes
Pre-filter sampled volumes. If entirely air (all positive) or entirely solid (all negative), skip meshing:
- Still counts as "complete" for transition group
- Mark as `EmptyNotification` instead of `MeshJob`

### 2. Floating Origin
Store world positions in double precision. Apply origin offset at render time:
```rust
// In shader/renderer
world_pos = local_pos * voxel_size + chunk_world_min - floating_origin_offset
```

### 3. Neighbor LOD Tracking
When neighbors are at different LODs, boundary vertices need displacement:
```rust
/// 27-entry neighbor LOD info (3×3×3 cube)
pub struct NeighborInfo {
    /// LOD of each neighbor (-1,-1,-1) to (+1,+1,+1)
    /// Index: (1+x) + (1+y)*3 + (1+z)*9
    pub neighbor_lods: [u8; 27],
}
```

### 4. Cascading Merges
When a merge completes, siblings might have already been removed by a parent merge:
- Don't add to phantom set if node wasn't in renderer
- Check renderer.contains() before adding phantom

### 5. Initialization

```rust
/// Initialize octree for a given view
pub fn create_initial_octree(
    initial_lod: i32,
    config: OctreeConfig,
) -> OctreeLeaves {
    // Start with single node covering initial view
    OctreeLeaves::new_with_initial(initial_lod)
}
```

## Test Cases for TDD

```rust
#[cfg(test)]
mod tests {
    // ═══════════════════════════════════════════════════════════════════
    // Basic Node Operations
    // ═══════════════════════════════════════════════════════════════════

    #[test] fn test_get_child_returns_finer_lod() { ... }
    #[test] fn test_get_child_all_8_octants() { ... }
    #[test] fn test_get_child_at_lod_0_returns_none() { ... }
    #[test] fn test_get_parent_returns_coarser_lod() { ... }
    #[test] fn test_get_parent_at_max_lod_returns_none() { ... }
    #[test] fn test_child_parent_roundtrip() { ... }
    #[test] fn test_node_equality() { ... }
    #[test] fn test_node_hash_consistency() { ... }

    // ═══════════════════════════════════════════════════════════════════
    // Coordinate Math
    // ═══════════════════════════════════════════════════════════════════

    #[test] fn test_cell_size_at_lod_0() { ... }
    #[test] fn test_cell_size_doubles_per_lod() { ... }
    #[test] fn test_voxel_size_at_lod() { ... }
    #[test] fn test_node_min_at_origin() { ... }
    #[test] fn test_node_min_with_world_origin() { ... }
    #[test] fn test_node_center() { ... }
    #[test] fn test_lod_threshold_with_exponent() { ... }

    // ═══════════════════════════════════════════════════════════════════
    // Refinement Logic
    // ═══════════════════════════════════════════════════════════════════

    #[test] fn test_subdivide_produces_8_children() { ... }
    #[test] fn test_subdivide_removes_parent_from_leaves() { ... }
    #[test] fn test_merge_produces_1_parent() { ... }
    #[test] fn test_merge_removes_8_children_from_leaves() { ... }
    #[test] fn test_no_subdivide_at_min_lod() { ... }
    #[test] fn test_no_merge_at_max_lod() { ... }
    #[test] fn test_merge_requires_all_8_siblings() { ... }
    #[test] fn test_subdivide_priority_closest_first() { ... }
    #[test] fn test_merge_priority_farthest_first() { ... }
    #[test] fn test_max_transitions_budget_enforced() { ... }

    // ═══════════════════════════════════════════════════════════════════
    // Transition Groups
    // ═══════════════════════════════════════════════════════════════════

    #[test] fn test_transition_group_key_is_parent() { ... }
    #[test] fn test_subdivide_group_invariants() { ... }
    #[test] fn test_merge_group_invariants() { ... }
    #[test] fn test_transition_groups_sorted_by_distance() { ... }

    // ═══════════════════════════════════════════════════════════════════
    // Edge Cases
    // ═══════════════════════════════════════════════════════════════════

    #[test] fn test_viewer_very_far_collapses_to_coarse() { ... }
    #[test] fn test_viewer_at_node_center_subdivides() { ... }
    #[test] fn test_empty_leaves_no_transitions() { ... }
    #[test] fn test_single_leaf_at_min_lod_no_subdivide() { ... }
    #[test] fn test_lod_exponent_affects_threshold() { ... }

    // ═══════════════════════════════════════════════════════════════════
    // Volume Constants
    // ═══════════════════════════════════════════════════════════════════

    #[test] fn test_to_index_roundtrip() { ... }
    #[test] fn test_index_bounds() { ... }
    #[test] fn test_voxels_per_cell_is_28() { ... }
}
```
