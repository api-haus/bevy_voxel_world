# Volume Layout Design Document

## C# Reference Implementation

### Core Constants (from VolumeConstants.cs)

```
SAMPLE_SIZE = 32           # Total samples per axis
INTERIOR_CELLS = 28        # Cells that produce mesh geometry
VOXELS_PER_CELL = 28       # Used for cell_size calculation
FIRST_INTERIOR_CELL = 1    # First cell that emits triangles
LAST_INTERIOR_CELL = 28    # Last cell that emits triangles
```

### Volume Layout (Single Axis)

```
Sample Index:  0     1     2    ...    27    28    29    30    31
               │     │                       │     │     │     │
               │     └───── 28 interior ─────┘     │     │     │
               │           cells (1-28)            │     └─────┴── displacement
               │                                   │                padding
               └─ negative                         └─ last interior
                  apron                               sample (29)
```

### Key Relationships

1. **Cell Size** = `voxel_size * VOXELS_PER_CELL * 2^LOD` = `voxel_size * 28 * 2^LOD`
2. **Node Min** = `RootMin + (node_coords * cell_size)`
3. **Sample Position** = `node_min + sample_index * voxel_size`
4. **Transform Position** = `node_min` (NO offset!)
5. **Transform Scale** = `voxel_size`

### World Position Mapping

For chunk at node (0,0,0) with `voxel_size=1`, `cell_size=28`:

```
Sample 0:  world position = node_min + 0  = 0
Sample 1:  world position = node_min + 1  = 1
...
Sample 28: world position = node_min + 28 = 28
Sample 29: world position = node_min + 29 = 29
Sample 30: world position = node_min + 30 = 30
Sample 31: world position = node_min + 31 = 31
```

For adjacent chunk at node (1,0,0):

```
node_min = 0 + 28 = 28

Sample 0:  world position = 28 + 0  = 28  ← SAME as chunk 0, sample 28!
Sample 1:  world position = 28 + 1  = 29  ← SAME as chunk 0, sample 29!
...
```

### Overlap Region

Adjacent chunks INTENTIONALLY overlap:

```
Chunk 0 samples:        0   1   ...  27  28  29  30  31
Chunk 1 samples:                              0   1   2   3   4  ...

World positions:        0   1   ...  27  28  29  30  31  32  33  ...
                                         ↑───────↑
                                       OVERLAP REGION
                                       (samples 28-31 of chunk 0
                                        = samples 0-3 of chunk 1)
```

### Triangle Emission Rules

**The critical rule**: Only interior cells (1-28) should emit triangles.

- **Cell 0**: Creates vertex (for neighbor's triangles), but does NOT emit its own triangles
- **Cells 1-28**: Create vertices AND emit triangles (interior)
- **Cells 29-30**: Create vertices (for LOD displacement), but do NOT emit triangles

The boundary check in surface nets:

```rust
// CURRENT (WRONG - only checks negative boundary)
if pos[u] == 0 || pos[v] == 0 {
  continue;  // Skip negative apron
}

// CORRECT (checks both boundaries)
if x < FIRST_INTERIOR_CELL || x > LAST_INTERIOR_CELL
   || y < FIRST_INTERIOR_CELL || y > LAST_INTERIOR_CELL
   || z < FIRST_INTERIOR_CELL || z > LAST_INTERIOR_CELL {
  return;  // Cell is not interior - skip ALL triangle emission
}
```

### Why Current Implementation Shows Overlap

With `voxel_size=16`, `cell_size=448`:

```
Chunk 0: world_min = -500
         mesh vertices span: -500 + 0*16 to -500 + 31*16 = [-500, -4]

Chunk 1: world_min = -52 (= -500 + 448)
         mesh vertices span: -52 + 0*16 to -52 + 31*16 = [-52, 444]

OVERLAP: [-52, -4] = 48 world units = 3 voxels
```

This happens because:
1. Mesh vertices span 32 samples (0-31)
2. Cell size is only 28 voxels
3. Triangles are emitted for cells 0-30, not just 1-28

### The Fix

1. Vertices should still be created for ALL cells with surface crossings (0-30)
2. Triangle emission should be RESTRICTED to interior cells (1-28 on ALL axes)
3. Cells 0, 29, 30 create vertices but NO triangles

This ensures:
- Chunk boundaries meet seamlessly (vertices at boundary match)
- No duplicate triangles in overlap region
- No Z-fighting or visual overlap
