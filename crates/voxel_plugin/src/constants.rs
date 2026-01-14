//! Volume layout constants for 32³ voxel chunks.
//!
//! These constants match the C# reference implementation and are optimized for
//! bit-shift operations (requiring exactly 32 samples per axis).
//!
//! # SDF Volume Layout
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         SDF VOLUME LAYOUT                               │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  Sample index:  0     1     2    ...    27    28    29    30    31      │
//! │                 │     │                       │     │     │     │       │
//! │                 │     └───── 28 interior ─────┘     │     │     │       │
//! │                 │           cells (1-28)            │     │     │       │
//! │                 │                                   │     └─────┴─────  │
//! │                 │                                   │     displacement  │
//! │                 └─ negative                         └─ last interior    │
//! │                    apron                               sample (29)      │
//! │                                                                         │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  Region Breakdown:                                                      │
//! │                                                                         │
//! │  [0]        Negative apron - for normal calculation at cell 1           │
//! │  [1-28]     Interior cell origins (28 cells that produce geometry)      │
//! │  [29]       Last sample needed by cell 28 (+1 corner)                   │
//! │  [30-31]    Displacement padding - stride-2 sampling for LOD seams      │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Why 28 Interior Cells?
//!
//! - Bit shifts require 32 samples (2^5 for efficient indexing)
//! - 1 sample for negative apron (normals at boundary)
//! - 2 samples for displacement padding (stride-2 at positive boundary)
//! - Remaining: 32 - 1 - 2 - 1 = 28 interior cells
//!
//! Cell 28 stride-2 displacement needs samples 28 and 30:
//! - `parent_x = (28 / 2) * 2 = 28`
//! - corners at 28, 28+2=30 ← both within bounds
//!
//! # Memory Layout
//!
//! ```text
//! Volume memory layout (row-major, Z innermost):
//!
//! Address:  0    1    2   ...  31   32   33  ...  1023  1024 ...
//! Content: [0,0,0][0,0,1]...[0,0,31][0,1,0]...[0,31,31][1,0,0]...
//!          └─────── Z ───────┘└─────── Z ───────┘
//!
//! Optimal access: Sequential Z, then Y, then X
//! Cache-friendly: Process Z-columns together (32 bytes = 1 cache line)
//! ```
//!
//! # 3D Indexing
//!
//! ```text
//! index = x << 10 | y << 5 | z
//!       = x * 1024 + y * 32 + z
//! ```
//!
//! # Coordinate System
//!
//! ```text
//!         +Y
//!          │
//!          │
//!          │
//!          └───────── +X
//!         /
//!        /
//!       +Z
//!
//! Cell corner indices (binary: ZYX):
//!   0 = (0,0,0)    4 = (0,0,1)
//!   1 = (1,0,0)    5 = (1,0,1)
//!   2 = (0,1,0)    6 = (0,1,1)
//!   3 = (1,1,0)    7 = (1,1,1)
//! ```

/// Number of samples per axis (must be 32 for bit-shift optimizations)
pub const SAMPLE_SIZE: usize = 32;

/// Samples squared (32² = 1024)
pub const SAMPLE_SIZE_SQ: usize = SAMPLE_SIZE * SAMPLE_SIZE;

/// Total samples in a chunk (32³ = 32768)
pub const SAMPLE_SIZE_CB: usize = SAMPLE_SIZE * SAMPLE_SIZE * SAMPLE_SIZE;

/// Maximum valid sample index (31)
pub const MAX_SAMPLE_INDEX: usize = SAMPLE_SIZE - 1;

/// Bit shift for Y coordinate indexing (log2(32) = 5)
pub const Y_SHIFT: u32 = 5;

/// Bit shift for X coordinate indexing (log2(1024) = 10)
pub const X_SHIFT: u32 = 10;

/// Mask for extracting single axis from index (0x1F = 31)
pub const INDEX_MASK: usize = 0x1F;

/// Number of interior cells per axis that produce geometry
pub const INTERIOR_CELLS: usize = 28;

/// First interior cell index (after negative apron)
pub const FIRST_INTERIOR_CELL: usize = 1;

/// Last interior cell index
pub const LAST_INTERIOR_CELL: usize = 28;

/// Padding for negative boundary normal calculation
pub const NEGATIVE_APRON: usize = 1;

/// Padding for stride-2 LOD seam displacement sampling
pub const DISPLACEMENT_PADDING: usize = 2;

/// Last interior sample index (LAST_INTERIOR_CELL + 1)
pub const LAST_INTERIOR_SAMPLE: usize = LAST_INTERIOR_CELL + 1;

/// Convert 3D coordinates to linear index using bit shifts.
///
/// Layout: X is major axis (stride 1024), Y is middle (stride 32), Z is minor
/// (stride 1)
#[inline(always)]
pub const fn coord_to_index(x: usize, y: usize, z: usize) -> usize {
  (x << X_SHIFT) | (y << Y_SHIFT) | z
}

/// Convert linear index to 3D coordinates.
#[inline(always)]
pub const fn index_to_coord(idx: usize) -> (usize, usize, usize) {
  let x = idx >> X_SHIFT;
  let y = (idx >> Y_SHIFT) & INDEX_MASK;
  let z = idx & INDEX_MASK;
  (x, y, z)
}

/// Volume index offsets for 8 cube corners relative to base position.
///
/// Corner layout (binary: ZYX):
/// - 0 = (0,0,0)
/// - 1 = (1,0,0)
/// - 2 = (0,1,0)
/// - 3 = (1,1,0)
/// - 4 = (0,0,1)
/// - 5 = (1,0,1)
/// - 6 = (0,1,1)
/// - 7 = (1,1,1)
pub const CORNER_OFFSETS: [usize; 8] = [
  0,                                   // (0,0,0)
  1 << X_SHIFT,                        // (1,0,0)
  1 << Y_SHIFT,                        // (0,1,0)
  (1 << X_SHIFT) | (1 << Y_SHIFT),     // (1,1,0)
  1,                                   // (0,0,1)
  (1 << X_SHIFT) | 1,                  // (1,0,1)
  (1 << Y_SHIFT) | 1,                  // (0,1,1)
  (1 << X_SHIFT) | (1 << Y_SHIFT) | 1, // (1,1,1)
];

#[cfg(test)]
#[path = "constants_test.rs"]
mod constants_test;
