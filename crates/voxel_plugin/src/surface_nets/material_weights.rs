//! Material weight calculation for Surface Nets vertices.

use crate::constants::*;
use crate::types::MaterialId;

/// Compute material blend weights from solid corners.
///
/// Uses the DreamCat Games algorithm: only corners with negative SDF (solid)
/// contribute their material weight. The resulting weights are normalized to
/// sum to 1.0.
///
/// # Arguments
///
/// * `materials` - Material ID array for the volume
/// * `corner_mask` - 8-bit mask of solid corners
/// * `base_idx` - Base index in volume for corner 0
///
/// # Returns
///
/// 4-element weight array (one per material slot), summing to 1.0.
pub fn compute(
  materials: &[MaterialId; SAMPLE_SIZE_CB],
  corner_mask: u8,
  base_idx: usize,
) -> [f32; 4] {
  let mut weights = [0.0f32; 4];

  for corner in 0..8 {
    // Skip non-solid corners (air)
    if (corner_mask & (1 << corner)) == 0 {
      continue;
    }

    // Get material ID for this solid corner
    let mat_id = materials[base_idx + CORNER_OFFSETS[corner]] as usize;

    // Clamp to valid range (0-3) and accumulate weight
    weights[mat_id.min(3)] += 1.0;
  }

  // Normalize weights to sum to 1.0
  let sum = weights[0] + weights[1] + weights[2] + weights[3];
  if sum > 0.0001 {
    let inv_sum = 1.0 / sum;
    weights[0] *= inv_sum;
    weights[1] *= inv_sum;
    weights[2] *= inv_sum;
    weights[3] *= inv_sum;
  } else {
    // Fallback: all weight on material 0
    weights[0] = 1.0;
  }

  weights
}

#[cfg(test)]
#[path = "material_weights_test.rs"]
mod material_weights_test;
