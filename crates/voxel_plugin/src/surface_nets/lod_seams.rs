//! LOD seam resolution for Surface Nets.
//!
//! Handles boundary vertex detection and displacement for seamless transitions
//! between chunks with different levels of detail.
//!
//! # The Seam Problem
//!
//! When voxel chunks at different resolutions meet, their mesh vertices don't
//! align:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    THE SEAM PROBLEM                             │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   FINE CHUNK (LOD 0)              COARSE CHUNK (LOD 1)         │
//! │                                                                 │
//! │   Fine vertices: *                Coarse vertices: o            │
//! │                                                                 │
//! │   * --- * --- * --- * --- *          o ------------- o          │
//! │   |     |     |     |     |          |               |          │
//! │   * --- * --- * --- * --- *          |               |          │
//! │   |     |     |     |     |          |               |          │
//! │   * --- * --- * --- * --- *    GAP   |               |          │
//! │   |     |     |     |     |  <---->  |               |          │
//! │   * --- * --- * --- * --- *          |               |          │
//! │   |     |     |     |     |          |               |          │
//! │   * --- * --- * --- * --- *          o ------------- o          │
//! │                                                                 │
//! │   Fine chunk has 5 vertices       Coarse chunk has 2 vertices   │
//! │   along boundary edge             along same edge               │
//! │                                                                 │
//! │   Surface positions DON'T MATCH -> visible gaps on slopes       │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Displaced Vertices Solution
//!
//! Recalculate boundary vertices using the coarser LOD's sampling positions:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   DISPLACED VERTICES CONCEPT                    │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   BEFORE DISPLACEMENT:                                          │
//! │                                                                 │
//! │   Fine chunk boundary       Coarse chunk boundary               │
//! │   (LOD 0)                   (LOD 1)                             │
//! │                                                                 │
//! │   *--*--*--*--*                    o-------------o              │
//! │   A  B  C  D  E                    A'            E'             │
//! │                                                                 │
//! │   B, C, D have no matching vertices on coarse side              │
//! │                                                                 │
//! │   AFTER DISPLACEMENT:                                           │
//! │                                                                 │
//! │   *-------------*                  o-------------o              │
//! │   A      >      E                  A'            E'             │
//! │        B,C,D                                                    │
//! │        snapped                                                  │
//! │                                                                 │
//! │   Boundary vertices B, C, D are RECALCULATED using              │
//! │   the same SDF sampling as the coarser LOD would use            │
//! │                                                                 │
//! │   Result: Fine boundary EXACTLY matches coarse boundary         │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Influence Zone
//!
//! Vertices within `NEIGHBOR_STEP` of a boundary with a coarser neighbor
//! are displaced:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                  VERTEX LOD DETERMINATION                       │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   NeighborStep = 2^(NeighborLOD - MyLOD)                        │
//! │                                                                 │
//! │   LOD diff = 1 -> NeighborStep = 2                              │
//! │   LOD diff = 2 -> NeighborStep = 4                              │
//! │   LOD diff = 3 -> NeighborStep = 8                              │
//! │                                                                 │
//! │   INFLUENCE ZONE (1D example, +X boundary):                     │
//! │                                                                 │
//! │   NeighborStep = 2                                              │
//! │   Interior cells = 28 (cells 1-28)                              │
//! │                                                                 │
//! │   Cell index:  1  2  3  ...  26  27  28                         │
//! │                                  +========+                     │
//! │                                  |INFLUENCE|                    │
//! │                                  |  ZONE   |                    │
//! │                                  +=========+                    │
//! │                                  ^                              │
//! │                        Cells within NeighborStep                │
//! │                        of boundary need displacement            │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Neighbor Mask Bits
//!
//! The 27-bit neighbor mask encodes which neighbors have coarser LOD:
//! - Bit 0: `ALL_SAME_LOD` flag (skip displacement if set)
//! - Bits 1-6: Face neighbors (6 faces)
//! - Bits 7-14: Corner neighbors (8 corners)
//! - Bits 15-26: Edge neighbors (12 edges)

use crate::constants::*;
use crate::edge_table::EDGE_TABLE;
use crate::types::{sdf_conversion, SdfSample};

// Neighbor mask bit positions (must match Unity NeighborMask)

/// Bit 0: ALL_SAME_LOD flag (skip displacement if set)
pub const ALL_SAME_LOD: u32 = 1 << 0;

// Bits 1-6: Face transitions
pub const FACE_POS_X: u32 = 1 << 1;
pub const FACE_NEG_X: u32 = 1 << 2;
pub const FACE_POS_Y: u32 = 1 << 3;
pub const FACE_NEG_Y: u32 = 1 << 4;
pub const FACE_POS_Z: u32 = 1 << 5;
pub const FACE_NEG_Z: u32 = 1 << 6;

// Bits 7-14: Vertex/corner transitions
pub const VERTEX_NNN: u32 = 1 << 7; // (-1,-1,-1)
pub const VERTEX_PNN: u32 = 1 << 8; // (+1,-1,-1)
pub const VERTEX_NPN: u32 = 1 << 9; // (-1,+1,-1)
pub const VERTEX_PPN: u32 = 1 << 10; // (+1,+1,-1)
pub const VERTEX_NNP: u32 = 1 << 11; // (-1,-1,+1)
pub const VERTEX_PNP: u32 = 1 << 12; // (+1,-1,+1)
pub const VERTEX_NPP: u32 = 1 << 13; // (-1,+1,+1)
pub const VERTEX_PPP: u32 = 1 << 14; // (+1,+1,+1)

// Bits 15-26: Edge transitions
pub const EDGE_X_NN: u32 = 1 << 15; // X-axis edge at Y-,Z-
pub const EDGE_X_PN: u32 = 1 << 16; // X-axis edge at Y+,Z-
pub const EDGE_X_NP: u32 = 1 << 17; // X-axis edge at Y-,Z+
pub const EDGE_X_PP: u32 = 1 << 18; // X-axis edge at Y+,Z+
pub const EDGE_Y_NN: u32 = 1 << 19; // Y-axis edge at X-,Z-
pub const EDGE_Y_PN: u32 = 1 << 20; // Y-axis edge at X+,Z-
pub const EDGE_Y_NP: u32 = 1 << 21; // Y-axis edge at X-,Z+
pub const EDGE_Y_PP: u32 = 1 << 22; // Y-axis edge at X+,Z+
pub const EDGE_Z_NN: u32 = 1 << 23; // Z-axis edge at X-,Y-
pub const EDGE_Z_PN: u32 = 1 << 24; // Z-axis edge at X+,Y-
pub const EDGE_Z_NP: u32 = 1 << 25; // Z-axis edge at X-,Y+
pub const EDGE_Z_PP: u32 = 1 << 26; // Z-axis edge at X+,Y+

// Combined masks
pub const ALL_FACE_BITS: u32 =
  FACE_POS_X | FACE_NEG_X | FACE_POS_Y | FACE_NEG_Y | FACE_POS_Z | FACE_NEG_Z;

pub const ALL_VERTEX_BITS: u32 = VERTEX_NNN
  | VERTEX_PNN
  | VERTEX_NPN
  | VERTEX_PPN
  | VERTEX_NNP
  | VERTEX_PNP
  | VERTEX_NPP
  | VERTEX_PPP;

pub const ALL_EDGE_BITS: u32 = EDGE_X_NN
  | EDGE_X_PN
  | EDGE_X_NP
  | EDGE_X_PP
  | EDGE_Y_NN
  | EDGE_Y_PN
  | EDGE_Y_NP
  | EDGE_Y_PP
  | EDGE_Z_NN
  | EDGE_Z_PN
  | EDGE_Z_NP
  | EDGE_Z_PP;

pub const ALL_TRANSITION_BITS: u32 = ALL_FACE_BITS | ALL_VERTEX_BITS | ALL_EDGE_BITS;

/// Neighbor step for LOD difference of 1
const NEIGHBOR_STEP: i32 = 2;

/// Interior bounds for quick rejection
const INTERIOR_MIN: i32 = FIRST_INTERIOR_CELL as i32 + NEIGHBOR_STEP;
const INTERIOR_MAX: i32 = LAST_INTERIOR_CELL as i32 - NEIGHBOR_STEP;

/// Helper type for neighbor mask configuration
pub struct NeighborMask;

impl NeighborMask {
  pub const ALL_SAME_LOD: u32 = ALL_SAME_LOD;
  pub const FACE_POS_X: u32 = FACE_POS_X;
  pub const FACE_NEG_X: u32 = FACE_NEG_X;
  pub const FACE_POS_Y: u32 = FACE_POS_Y;
  pub const FACE_NEG_Y: u32 = FACE_NEG_Y;
  pub const FACE_POS_Z: u32 = FACE_POS_Z;
  pub const FACE_NEG_Z: u32 = FACE_NEG_Z;
}

/// Check if a vertex at the given cell position is within the influence zone
/// of any direction that has a coarser LOD neighbor.
///
/// Uses compound proximity checks:
/// - Face: 1 axis near boundary
/// - Edge: 2 axes near boundary
/// - Corner: 3 axes near boundary
pub fn is_boundary_vertex(cell_pos: [i32; 3], mask: u32) -> bool {
  let [x, y, z] = cell_pos;

  // Quick interior check
  if (INTERIOR_MIN..=INTERIOR_MAX).contains(&x)
    && (INTERIOR_MIN..=INTERIOR_MAX).contains(&y)
    && (INTERIOR_MIN..=INTERIOR_MAX).contains(&z)
  {
    return false;
  }

  // Precompute boundary proximity flags
  let max_cell = LAST_INTERIOR_CELL as i32;
  let min_cell = FIRST_INTERIOR_CELL as i32;

  let near_pos_x = x > max_cell - NEIGHBOR_STEP;
  let near_neg_x = x < min_cell + NEIGHBOR_STEP;
  let near_pos_y = y > max_cell - NEIGHBOR_STEP;
  let near_neg_y = y < min_cell + NEIGHBOR_STEP;
  let near_pos_z = z > max_cell - NEIGHBOR_STEP;
  let near_neg_z = z < min_cell + NEIGHBOR_STEP;

  // Face checks (1 axis)
  if (mask & FACE_POS_X) != 0 && near_pos_x {
    return true;
  }
  if (mask & FACE_NEG_X) != 0 && near_neg_x {
    return true;
  }
  if (mask & FACE_POS_Y) != 0 && near_pos_y {
    return true;
  }
  if (mask & FACE_NEG_Y) != 0 && near_neg_y {
    return true;
  }
  if (mask & FACE_POS_Z) != 0 && near_pos_z {
    return true;
  }
  if (mask & FACE_NEG_Z) != 0 && near_neg_z {
    return true;
  }

  // Edge checks (2 axes)
  if (mask & EDGE_X_NN) != 0 && near_neg_y && near_neg_z {
    return true;
  }
  if (mask & EDGE_X_PN) != 0 && near_pos_y && near_neg_z {
    return true;
  }
  if (mask & EDGE_X_NP) != 0 && near_neg_y && near_pos_z {
    return true;
  }
  if (mask & EDGE_X_PP) != 0 && near_pos_y && near_pos_z {
    return true;
  }
  if (mask & EDGE_Y_NN) != 0 && near_neg_x && near_neg_z {
    return true;
  }
  if (mask & EDGE_Y_PN) != 0 && near_pos_x && near_neg_z {
    return true;
  }
  if (mask & EDGE_Y_NP) != 0 && near_neg_x && near_pos_z {
    return true;
  }
  if (mask & EDGE_Y_PP) != 0 && near_pos_x && near_pos_z {
    return true;
  }
  if (mask & EDGE_Z_NN) != 0 && near_neg_x && near_neg_y {
    return true;
  }
  if (mask & EDGE_Z_PN) != 0 && near_pos_x && near_neg_y {
    return true;
  }
  if (mask & EDGE_Z_NP) != 0 && near_neg_x && near_pos_y {
    return true;
  }
  if (mask & EDGE_Z_PP) != 0 && near_pos_x && near_pos_y {
    return true;
  }

  // Corner checks (3 axes)
  if (mask & VERTEX_NNN) != 0 && near_neg_x && near_neg_y && near_neg_z {
    return true;
  }
  if (mask & VERTEX_PNN) != 0 && near_pos_x && near_neg_y && near_neg_z {
    return true;
  }
  if (mask & VERTEX_NPN) != 0 && near_neg_x && near_pos_y && near_neg_z {
    return true;
  }
  if (mask & VERTEX_PPN) != 0 && near_pos_x && near_pos_y && near_neg_z {
    return true;
  }
  if (mask & VERTEX_NNP) != 0 && near_neg_x && near_neg_y && near_pos_z {
    return true;
  }
  if (mask & VERTEX_PNP) != 0 && near_pos_x && near_neg_y && near_pos_z {
    return true;
  }
  if (mask & VERTEX_NPP) != 0 && near_neg_x && near_pos_y && near_pos_z {
    return true;
  }
  if (mask & VERTEX_PPP) != 0 && near_pos_x && near_pos_y && near_pos_z {
    return true;
  }

  false
}

/// Compute displaced position matching coarser LOD sampling.
///
/// Resamples the SDF at stride-2 resolution (parent cell) and computes
/// the vertex position as the coarser LOD would.
pub fn compute_displaced_position(
  volume: &[SdfSample; SAMPLE_SIZE_CB],
  cell_pos: [i32; 3],
  original_position: [f32; 3],
) -> [f32; 3] {
  const PARENT_STEP: i32 = 2;
  let max_idx = MAX_SAMPLE_INDEX as i32;

  // Align to parent cell grid
  let parent_x = (cell_pos[0] / PARENT_STEP) * PARENT_STEP;
  let parent_y = (cell_pos[1] / PARENT_STEP) * PARENT_STEP;
  let parent_z = (cell_pos[2] / PARENT_STEP) * PARENT_STEP;

  // Sample 8 corners at stride-2 spacing
  let mut samples = [0.0f32; 8];
  let mut corner_mask = 0u8;

  for corner in 0..8 {
    let dx = ((corner & 1) as i32) * PARENT_STEP;
    let dy = (((corner >> 1) & 1) as i32) * PARENT_STEP;
    let dz = (((corner >> 2) & 1) as i32) * PARENT_STEP;

    // Clamp sample positions to valid bounds
    let sample_x = (parent_x + dx).min(max_idx).max(0) as usize;
    let sample_y = (parent_y + dy).min(max_idx).max(0) as usize;
    let sample_z = (parent_z + dz).min(max_idx).max(0) as usize;

    let idx = coord_to_index(sample_x, sample_y, sample_z);
    let sdf = volume[idx];
    // Use proper SDF scaling for smooth interpolation
    samples[corner] = sdf_conversion::to_float(sdf);

    if sdf < 0 {
      corner_mask |= 1 << corner;
    }
  }

  // No surface crossing at coarser resolution - return original
  if corner_mask == 0 || corner_mask == 255 {
    return original_position;
  }

  // Compute vertex position using same algorithm
  let edge_mask = EDGE_TABLE[corner_mask as usize];
  let local_pos = compute_vertex_position(&samples, edge_mask);

  // Scale back to chunk coordinates
  [
    parent_x as f32 + local_pos[0] * PARENT_STEP as f32,
    parent_y as f32 + local_pos[1] * PARENT_STEP as f32,
    parent_z as f32 + local_pos[2] * PARENT_STEP as f32,
  ]
}

/// Compute vertex position (simplified version for displacement)
fn compute_vertex_position(samples: &[f32; 8], edge_mask: u16) -> [f32; 3] {
  use crate::edge_table::EDGE_CORNERS;

  let mut position = [0.0f32; 3];
  let mut count = 0;

  for edge in 0..12 {
    if (edge_mask & (1 << edge)) == 0 {
      continue;
    }

    let [c0, c1] = EDGE_CORNERS[edge];
    let s0 = samples[c0 as usize];
    let s1 = samples[c1 as usize];

    let diff = s0 - s1;
    let t = if diff.abs() < 1e-6 { 0.5 } else { s0 / diff };

    let p0 = corner_pos(c0);
    let p1 = corner_pos(c1);

    position[0] += p0[0] + t * (p1[0] - p0[0]);
    position[1] += p0[1] + t * (p1[1] - p0[1]);
    position[2] += p0[2] + t * (p1[2] - p0[2]);
    count += 1;
  }

  if count == 0 {
    return [0.5, 0.5, 0.5];
  }

  let inv = 1.0 / count as f32;
  [position[0] * inv, position[1] * inv, position[2] * inv]
}

fn corner_pos(corner: u8) -> [f32; 3] {
  [
    (corner & 1) as f32,
    ((corner >> 1) & 1) as f32,
    ((corner >> 2) & 1) as f32,
  ]
}

// =============================================================================
// Boundary blend factor for normal interpolation
// =============================================================================

/// Compute blend factor for transitioning between interior and boundary
/// normals.
///
/// Returns a value in [0.0, 1.0]:
/// - 1.0 = interior (use geometry normals)
/// - 0.0 = at boundary (use gradient normals)
/// - (0.0, 1.0) = blend zone
///
/// # Arguments
///
/// * `cell_pos` - Cell position [x, y, z]
/// * `blend_distance` - Number of cells from boundary where blending starts
///
/// # Example
///
/// ```text
/// blend_distance = 3.0
///
/// Cell index:   1    2    3    4    5    ...   24   25   26   27   28
/// Blend factor: 0.0  0.33 0.67 1.0  1.0  ...   1.0  1.0  0.67 0.33 0.0
///               └─────────┘                        └─────────┘
///               blend zone                         blend zone
/// ```
pub fn compute_boundary_blend_factor(cell_pos: [i32; 3], blend_distance: f32) -> f32 {
  let min_cell = FIRST_INTERIOR_CELL as f32;
  let max_cell = LAST_INTERIOR_CELL as f32;

  // Distance from each boundary
  let dist_neg_x = cell_pos[0] as f32 - min_cell;
  let dist_pos_x = max_cell - cell_pos[0] as f32;
  let dist_neg_y = cell_pos[1] as f32 - min_cell;
  let dist_pos_y = max_cell - cell_pos[1] as f32;
  let dist_neg_z = cell_pos[2] as f32 - min_cell;
  let dist_pos_z = max_cell - cell_pos[2] as f32;

  // Minimum distance to any boundary
  let min_dist = dist_neg_x
    .min(dist_pos_x)
    .min(dist_neg_y)
    .min(dist_pos_y)
    .min(dist_neg_z)
    .min(dist_pos_z);

  // Convert to blend factor: 0 at boundary, 1 at blend_distance from boundary
  if blend_distance <= 0.0 {
    return 1.0; // No blending
  }

  (min_dist / blend_distance).clamp(0.0, 1.0)
}

/// Check if a vertex needs boundary blending.
///
/// More efficient than computing the full blend factor when you just need
/// to know if blending is required.
#[inline]
pub fn needs_boundary_blend(cell_pos: [i32; 3], blend_distance: f32) -> bool {
  let min_cell = FIRST_INTERIOR_CELL as i32;
  let max_cell = LAST_INTERIOR_CELL as i32;
  let dist = blend_distance as i32;

  cell_pos[0] < min_cell + dist
    || cell_pos[0] > max_cell - dist
    || cell_pos[1] < min_cell + dist
    || cell_pos[1] > max_cell - dist
    || cell_pos[2] < min_cell + dist
    || cell_pos[2] > max_cell - dist
}

#[cfg(test)]
#[path = "lod_seams_test.rs"]
mod lod_seams_test;
