//! Vertex position calculation for Surface Nets.
//!
//! Optimized implementation using direct edge iteration and SIMD vectors.

pub use glam::Vec3A;

/// Precomputed corner positions within unit cube.
/// Layout matches corner index bits: corner i = (x=bit0, y=bit1, z=bit2)
pub const CORNER_POSITIONS: [Vec3A; 8] = [
  Vec3A::new(0.0, 0.0, 0.0), // 0b000
  Vec3A::new(1.0, 0.0, 0.0), // 0b001
  Vec3A::new(0.0, 1.0, 0.0), // 0b010
  Vec3A::new(1.0, 1.0, 0.0), // 0b011
  Vec3A::new(0.0, 0.0, 1.0), // 0b100
  Vec3A::new(1.0, 0.0, 1.0), // 0b101
  Vec3A::new(0.0, 1.0, 1.0), // 0b110
  Vec3A::new(1.0, 1.0, 1.0), // 0b111
];

/// Edge definitions: pairs of corner indices.
/// 12 edges of a cube, ordered for consistency with edge_table.
pub const CUBE_EDGES: [[usize; 2]; 12] = [
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

/// Compute vertex position as centroid of edge crossing points.
///
/// Uses direct edge iteration - no edge_mask lookup needed.
/// Checks sign changes directly on samples for each edge.
/// Returns Vec3A for SIMD efficiency - caller converts to array when storing.
#[inline]
pub fn compute_position_direct(samples: &[f32; 8]) -> Vec3A {
  let mut sum = Vec3A::ZERO;
  let mut count = 0u32;

  for &[c0, c1] in &CUBE_EDGES {
    let s0 = samples[c0];
    let s1 = samples[c1];

    // Check if edge crosses surface (signs differ)
    if (s0 < 0.0) != (s1 < 0.0) {
      // Interpolation factor for zero-crossing
      let t = s0 / (s0 - s1);

      // Lerp between corner positions
      let p0 = CORNER_POSITIONS[c0];
      let p1 = CORNER_POSITIONS[c1];
      sum += p0 + t * (p1 - p0);
      count += 1;
    }
  }

  if count == 0 {
    return Vec3A::splat(0.5); // Fallback to center
  }

  sum / count as f32
}

#[cfg(test)]
#[path = "vertex_calc_test.rs"]
mod vertex_calc_test;
