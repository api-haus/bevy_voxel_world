# Octree Module TDD Implementation Plan

## Overview

This document organizes the 35 test stubs from `octree_refinement_context.md` into a dependency-ordered implementation sequence for incremental TDD development.

## Module Structure

```
voxel_plugin/src/
  octree/
    mod.rs              # Module exports
    volume.rs           # Volume constants (can reuse/extend constants.rs)
    node.rs             # OctreeNode struct
    config.rs           # OctreeConfig with coordinate math
    leaves.rs           # OctreeLeaves implicit bounds
    transition.rs       # TransitionType, TransitionGroup
    refinement.rs       # Refinement algorithm
    tests/
      mod.rs
      node_tests.rs
      config_tests.rs
      refinement_tests.rs
      transition_tests.rs
```

**Alternative:** Since `constants.rs` already exists with identical constants, the octree module can import from there rather than duplicating. Only add octree-specific constants (VOXELS_PER_CELL alias) if needed.

## Dependencies

Required crates (check Cargo.toml):
- `glam` for `DVec3` (double precision vectors)
- `smallvec` for `SmallVec<[T; 8]>`

## Implementation Batches

---

### Batch 1: Foundation - Volume Constants

**Purpose:** Verify existing constants align with octree needs; add any missing ones.

**Files:** `voxel_plugin/src/constants.rs` (extend existing)

**Tests:**

| # | Test Name | Description |
|---|-----------|-------------|
| 1 | `test_voxels_per_cell_is_28` | Verify INTERIOR_CELLS (or alias) equals 28 |
| 2 | `test_to_index_roundtrip` | Already exists, ensure octree compatibility |
| 3 | `test_index_bounds` | Verify all edge indices are valid |

**Implementation Notes:**
- `constants.rs` already has `INTERIOR_CELLS = 28`, `coord_to_index`, `index_to_coord`
- Add `pub const VOXELS_PER_CELL: usize = INTERIOR_CELLS;` if explicit alias desired
- These tests largely exist; this batch is about verification

**Exit Criteria:** All volume constant tests pass, imports work for octree module.

---

### Batch 2: OctreeNode - Core Structure

**Purpose:** Implement immutable node value type with parent/child navigation.

**Files:** `voxel_plugin/src/octree/node.rs`

**Tests:**

| # | Test Name | Description | Dependencies |
|---|-----------|-------------|--------------|
| 4 | `test_node_equality` | Two nodes with same x,y,z,lod are equal | None |
| 5 | `test_node_hash_consistency` | Equal nodes produce equal hashes | test_node_equality |
| 6 | `test_get_child_returns_finer_lod` | Child has lod - 1 | None |
| 7 | `test_get_child_all_8_octants` | All octants (0-7) produce correct children | test_get_child_returns_finer_lod |
| 8 | `test_get_child_at_lod_0_returns_none` | Cannot subdivide at finest LOD | test_get_child_returns_finer_lod |
| 9 | `test_get_parent_returns_coarser_lod` | Parent has lod + 1 | None |
| 10 | `test_get_parent_at_max_lod_returns_none` | Cannot go coarser than max | test_get_parent_returns_coarser_lod |
| 11 | `test_child_parent_roundtrip` | parent(child(node)) may not equal node (asymmetric), but relationship is consistent | All above |

**Implementation Order:**
1. Define `OctreeNode` struct with derives (Clone, Copy, PartialEq, Eq, Hash, Debug)
2. Implement `new()` constructor
3. Implement `get_child(octant: u8) -> Option<Self>`
4. Implement `get_parent(max_lod: i32) -> Option<Self>`

**Exit Criteria:** 8 node tests pass.

---

### Batch 3: OctreeConfig - Coordinate Math

**Purpose:** World coordinate calculations, LOD thresholds.

**Files:** `voxel_plugin/src/octree/config.rs`

**Dependencies:** Batch 2 (uses OctreeNode), requires `glam::DVec3`

**Tests:**

| # | Test Name | Description | Dependencies |
|---|-----------|-------------|--------------|
| 12 | `test_cell_size_at_lod_0` | cell_size = voxel_size * 28 * 1 = 28 | None |
| 13 | `test_cell_size_doubles_per_lod` | cell_size(lod+1) = 2 * cell_size(lod) | test_cell_size_at_lod_0 |
| 14 | `test_voxel_size_at_lod` | voxel_at_lod = voxel_size * 2^lod | None |
| 15 | `test_node_min_at_origin` | Node (0,0,0) at LOD 0 with origin (0,0,0) has min (0,0,0) | test_cell_size_at_lod_0 |
| 16 | `test_node_min_with_world_origin` | Node position offsets by world_origin | test_node_min_at_origin |
| 17 | `test_node_center` | Center = min + cell_size * 0.5 | test_node_min_at_origin |
| 18 | `test_lod_threshold_with_exponent` | threshold = cell_size * 2^lod_exponent | test_cell_size_at_lod_0 |
| 19 | `test_lod_exponent_affects_threshold` | Different exponents produce different thresholds | test_lod_threshold_with_exponent |

**Implementation Order:**
1. Define `OctreeConfig` struct with fields (voxel_size, world_origin, min_lod, max_lod, lod_exponent)
2. Implement `Default` trait
3. Implement `get_cell_size(lod: i32) -> f64`
4. Implement `get_voxel_size(lod: i32) -> f64`
5. Implement `get_node_min(&self, node: &OctreeNode) -> DVec3`
6. Implement `get_node_center(&self, node: &OctreeNode) -> DVec3`
7. Implement `get_threshold(lod: i32) -> f64`

**Exit Criteria:** 8 config tests pass.

---

### Batch 4: OctreeLeaves - Implicit Bounds

**Purpose:** HashSet-based leaf storage with implicit tree structure.

**Files:** `voxel_plugin/src/octree/leaves.rs`

**Dependencies:** Batch 2 (OctreeNode)

**Tests:**

| # | Test Name | Description | Dependencies |
|---|-----------|-------------|--------------|
| 20 | `test_empty_leaves_no_transitions` | Empty leaves produce no transitions | None |
| 21 | `test_single_leaf_at_min_lod_no_subdivide` | Leaf at min_lod cannot subdivide | Batch 2 |

**Implementation Order:**
1. Define `OctreeLeaves` struct with `leaves: HashSet<OctreeNode>`
2. Implement `new_with_initial(lod: i32) -> Self`
3. Implement `effective_max_lod() -> i32`
4. Implement iterator/accessor methods

**Exit Criteria:** 2 leaves tests pass.

---

### Batch 5: TransitionGroup - Atomic State Changes

**Purpose:** Define transition types and group structure.

**Files:** `voxel_plugin/src/octree/transition.rs`

**Dependencies:** Batch 2 (OctreeNode)

**Tests:**

| # | Test Name | Description | Dependencies |
|---|-----------|-------------|--------------|
| 22 | `test_transition_group_key_is_parent` | group_key field is always the parent node | None |
| 23 | `test_subdivide_group_invariants` | Subdivide: 8 to_add, 1 to_remove | None |
| 24 | `test_merge_group_invariants` | Merge: 1 to_add, 8 to_remove | None |

**Implementation Order:**
1. Define `TransitionType` enum (Subdivide, Merge)
2. Define `TransitionGroup` struct
3. Implement factory methods: `new_subdivide()`, `new_merge()`

**Exit Criteria:** 3 transition tests pass.

---

### Batch 6: Refinement Core - Subdivide/Merge Operations

**Purpose:** Apply atomic operations to leaf set.

**Files:** `voxel_plugin/src/octree/refinement.rs`

**Dependencies:** Batches 2-5

**Tests:**

| # | Test Name | Description | Dependencies |
|---|-----------|-------------|--------------|
| 25 | `test_subdivide_produces_8_children` | Subdivide creates all 8 children | Batch 5 |
| 26 | `test_subdivide_removes_parent_from_leaves` | Parent no longer in leaves after subdivide | test 25 |
| 27 | `test_merge_produces_1_parent` | Merge creates parent node | Batch 5 |
| 28 | `test_merge_removes_8_children_from_leaves` | All children removed after merge | test 27 |
| 29 | `test_no_subdivide_at_min_lod` | Cannot subdivide when lod == min_lod | test 25 |
| 30 | `test_no_merge_at_max_lod` | Cannot merge when parent.lod would exceed max_lod | test 27 |
| 31 | `test_merge_requires_all_8_siblings` | Merge only valid if all 8 children exist as leaves | test 27 |

**Implementation Order:**
1. Define `RefinementInput` struct
2. Define `RefinementOutput` struct
3. Implement `apply_subdivide()` helper
4. Implement `apply_merge()` helper
5. Implement `all_children_are_leaves()` helper

**Exit Criteria:** 7 core refinement tests pass.

---

### Batch 7: Refinement Algorithm - Priority and Budget

**Purpose:** Full refinement loop with sorting and transition budget.

**Files:** `voxel_plugin/src/octree/refinement.rs` (extend)

**Dependencies:** Batch 6

**Tests:**

| # | Test Name | Description | Dependencies |
|---|-----------|-------------|--------------|
| 32 | `test_subdivide_priority_closest_first` | Nodes closer to viewer subdivide first | Batch 3, Batch 6 |
| 33 | `test_merge_priority_farthest_first` | Nodes farthest from viewer merge first | Batch 3, Batch 6 |
| 34 | `test_max_transitions_budget_enforced` | Transition count <= max_transitions_per_frame | test 32, test 33 |
| 35 | `test_transition_groups_sorted_by_distance` | Output groups sorted by proximity | test 32, test 33 |

**Implementation Order:**
1. Implement Phase 1: Identify candidates (subdivide/coarsen)
2. Implement Phase 2: Validate coarsening
3. Implement Phase 3: Sort by priority
4. Implement Phase 4: Apply collapses
5. Implement Phase 5: Apply subdivisions
6. Implement output sorting

**Exit Criteria:** 4 algorithm tests pass.

---

### Batch 8: Edge Cases and Viewer Behavior

**Purpose:** Ensure correct behavior at boundaries and special conditions.

**Files:** `voxel_plugin/src/octree/refinement.rs` (tests only)

**Dependencies:** Batches 6-7

**Tests:**

| # | Test Name | Description | Dependencies |
|---|-----------|-------------|--------------|
| 36 | `test_viewer_very_far_collapses_to_coarse` | Distant viewer triggers merges | All above |
| 37 | `test_viewer_at_node_center_subdivides` | Viewer inside threshold triggers subdivide | All above |

**Implementation Notes:**
- These are integration tests using full `refine()` function
- Verify the algorithm converges correctly

**Exit Criteria:** 2 edge case tests pass.

---

## Summary: Test Execution Order

| Batch | Tests | Cumulative | Files |
|-------|-------|------------|-------|
| 1 | 3 | 3 | constants.rs |
| 2 | 8 | 11 | octree/node.rs |
| 3 | 8 | 19 | octree/config.rs |
| 4 | 2 | 21 | octree/leaves.rs |
| 5 | 3 | 24 | octree/transition.rs |
| 6 | 7 | 31 | octree/refinement.rs |
| 7 | 4 | 35 | octree/refinement.rs |
| 8 | 2 | 37 | octree/refinement.rs (tests) |

**Note:** The context document lists 35 stubs, but tests 36-37 are edge cases that could be counted as 2 additional integration tests.

---

## Cargo.toml Changes Required

```toml
[dependencies]
rayon = { workspace = true }
glam = { version = "0.29", features = ["std"] }  # For DVec3
smallvec = "1.13"  # For SmallVec<[OctreeNode; 8]>
```

---

## Module Wiring

After all batches complete, add to `lib.rs`:

```rust
pub mod octree;
pub use octree::{
    OctreeNode, OctreeConfig, OctreeLeaves,
    TransitionType, TransitionGroup,
    RefinementInput, RefinementOutput, refine,
};
```

---

## Implementation Notes

### Critical Invariants to Test

1. **Child coordinates:** `child.x = parent.x * 2 + (octant & 1)`
2. **Parent coordinates:** `parent.x = child.x / 2` (integer division)
3. **LOD direction:** Lower LOD = finer detail (opposite of some conventions)
4. **Cell size formula:** `voxel_size * VOXELS_PER_CELL * 2^lod`
5. **Threshold formula:** `cell_size * 2^lod_exponent`

### Known Edge Cases

1. **Negative coordinates:** Node coords can be negative
2. **Large LODs:** LOD 30 with i32 coords can overflow - consider i64 or bounds checking
3. **Asymmetric roundtrip:** `parent(child(node, octant))` equals `node`, but `child(parent(node), octant)` may not equal `node` (depends on octant)

### Float Precision

- Use `f64` (DVec3) for world coordinates to support huge worlds
- `f32` only for final mesh output
- Threshold comparisons use strict `<` not `<=`

---

## Test Data Fixtures

Consider creating test helpers:

```rust
#[cfg(test)]
mod test_fixtures {
    use super::*;

    /// Standard test config with voxel_size=1.0
    pub fn default_config() -> OctreeConfig {
        OctreeConfig::default()
    }

    /// Node at origin with given LOD
    pub fn origin_node(lod: i32) -> OctreeNode {
        OctreeNode { x: 0, y: 0, z: 0, lod }
    }

    /// Viewer at world origin
    pub fn origin_viewer() -> DVec3 {
        DVec3::ZERO
    }
}
```

---

## Next Steps

After this plan is reviewed:
1. Add glam/smallvec dependencies
2. Create `voxel_plugin/src/octree/mod.rs`
3. Implement Batch 1 (verify constants)
4. Proceed through batches sequentially

Each batch follows red-green-refactor:
1. **Red:** Write tests that fail
2. **Green:** Minimal implementation to pass
3. **Refactor:** Clean up without breaking tests