//! Precomputed edge crossing table for Surface Nets.
//!
//! Maps 8-bit corner masks to 12-bit edge masks indicating which edges
//! have surface crossings.
//!
//! # Cube Topology
//!
//! ```text
//!       6──────7         Corners (binary ZYX):
//!      /│     /│           0=(0,0,0)  1=(1,0,0)  2=(0,1,0)  3=(1,1,0)
//!     4─┼────5 │           4=(0,0,1)  5=(1,0,1)  6=(0,1,1)  7=(1,1,1)
//!     │ 2────┼─3
//!     │/     │/          +Y
//!     0──────1            │  +Z
//!                         │ /
//!                         └───+X
//! ```
//!
//! # Edge Layout
//!
//! ```text
//! 12 edges total (4 per axis):
//!
//! X-axis edges (parallel to X):
//!   Edge 0:  [0,1] at Y=0, Z=0 (bottom-back)
//!   Edge 5:  [2,3] at Y=1, Z=0 (top-back)
//!   Edge 8:  [4,5] at Y=0, Z=1 (bottom-front)
//!   Edge 11: [6,7] at Y=1, Z=1 (top-front)
//!
//! Y-axis edges (parallel to Y):
//!   Edge 1:  [0,2] at X=0, Z=0 (left-back)
//!   Edge 3:  [1,3] at X=1, Z=0 (right-back)
//!   Edge 9:  [4,6] at X=0, Z=1 (left-front)
//!   Edge 10: [5,7] at X=1, Z=1 (right-front)
//!
//! Z-axis edges (parallel to Z):
//!   Edge 2:  [0,4] at X=0, Y=0 (bottom-left)
//!   Edge 4:  [1,5] at X=1, Y=0 (bottom-right)
//!   Edge 6:  [2,6] at X=0, Y=1 (top-left)
//!   Edge 7:  [3,7] at X=1, Y=1 (top-right)
//! ```
//!
//! # Edge Table Usage
//!
//! Given a corner mask (8 bits indicating which corners are solid),
//! look up `EDGE_TABLE[corner_mask]` to get a 12-bit edge mask.
//! Each set bit indicates an edge with a surface crossing.
//!
//! ```text
//! Corner mask: 0b00000001 (only corner 0 is solid)
//! Edge mask:   0b00000111 (edges 0, 1, 2 have crossings)
//!                    ^^^
//!                    ||└─ Edge 0: [0,1] crosses (0 solid, 1 air)
//!                    |└── Edge 1: [0,2] crosses (0 solid, 2 air)
//!                    └─── Edge 2: [0,4] crosses (0 solid, 4 air)
//! ```

/// Edge endpoint corner indices.
/// Each edge connects two corners of the 2×2×2 cube.
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
pub const EDGE_CORNERS: [[u8; 2]; 12] = [
  [0, 1], // Edge 0:  X axis at Y=0, Z=0
  [0, 2], // Edge 1:  Y axis at X=0, Z=0
  [0, 4], // Edge 2:  Z axis at X=0, Y=0
  [1, 3], // Edge 3:  Y axis at X=1, Z=0
  [1, 5], // Edge 4:  Z axis at X=1, Y=0
  [2, 3], // Edge 5:  X axis at Y=1, Z=0
  [2, 6], // Edge 6:  Z axis at X=0, Y=1
  [3, 7], // Edge 7:  Z axis at X=1, Y=1
  [4, 5], // Edge 8:  X axis at Y=0, Z=1
  [4, 6], // Edge 9:  Y axis at X=0, Z=1
  [5, 7], // Edge 10: Y axis at X=1, Z=1
  [6, 7], // Edge 11: X axis at Y=1, Z=1
];

/// Precomputed edge table.
/// Index: 8-bit corner mask (which corners are solid)
/// Value: 12-bit edge mask (which edges have crossings)
///
/// An edge has a crossing if exactly one of its endpoint corners is solid.
pub const EDGE_TABLE: [u16; 256] = generate_edge_table();

/// Generate the edge table at compile time.
const fn generate_edge_table() -> [u16; 256] {
  let mut table = [0u16; 256];
  let mut corner_mask = 0usize;

  while corner_mask < 256 {
    let mut edge_mask = 0u16;
    let mut edge = 0;

    while edge < 12 {
      let c0 = EDGE_CORNERS[edge][0] as usize;
      let c1 = EDGE_CORNERS[edge][1] as usize;

      let solid0 = (corner_mask >> c0) & 1;
      let solid1 = (corner_mask >> c1) & 1;

      // Edge has crossing if corners have different signs
      if solid0 != solid1 {
        edge_mask |= 1 << edge;
      }

      edge += 1;
    }

    table[corner_mask] = edge_mask;
    corner_mask += 1;
  }

  table
}

/// Get corner position within unit cube.
#[inline(always)]
pub const fn corner_position(corner: u8) -> [f32; 3] {
  [
    (corner & 1) as f32,
    ((corner >> 1) & 1) as f32,
    ((corner >> 2) & 1) as f32,
  ]
}

#[cfg(test)]
#[path = "edge_table_test.rs"]
mod edge_table_test;
