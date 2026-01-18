//! Gradient/normal computation using glam SIMD.
//!
//! Computes the gradient of the SDF at a point, which gives us the surface
//! normal. Uses 2x2x2 stencil (8 corner samples per cell).
//!
//! Also provides geometry-based normal calculation from triangle faces.

use glam::Vec3A;

use crate::types::MeshOutput;

/// Compute gradient normal from 8 corner samples using SIMD.
///
/// Corner layout:
/// ```text
/// 0: (0,0,0)  4: (0,0,1)
/// 1: (1,0,0)  5: (1,0,1)
/// 2: (0,1,0)  6: (0,1,1)
/// 3: (1,1,0)  7: (1,1,1)
/// ```
#[inline]
pub fn compute(samples: &[f32; 8]) -> [f32; 3] {
  // X gradient: sum of right face - sum of left face
  let gx = (samples[1] + samples[3] + samples[5] + samples[7])
    - (samples[0] + samples[2] + samples[4] + samples[6]);

  // Y gradient: sum of top face - sum of bottom face
  let gy = (samples[2] + samples[3] + samples[6] + samples[7])
    - (samples[0] + samples[1] + samples[4] + samples[5]);

  // Z gradient: sum of front face - sum of back face
  let gz = (samples[4] + samples[5] + samples[6] + samples[7])
    - (samples[0] + samples[1] + samples[2] + samples[3]);

  // Normalize using glam SIMD
  let gradient = Vec3A::new(gx, gy, gz);
  let len_sq = gradient.length_squared();

  if len_sq < 1e-8 {
    return [0.0, 1.0, 0.0]; // Fallback to up
  }

  let normalized = gradient * len_sq.sqrt().recip();
  [normalized.x, normalized.y, normalized.z]
}

/// Compute interpolated gradient normal using vertex position within cell.
///
/// Instead of computing a single gradient for the entire cell, this computes
/// gradients at each of the 8 corners using one-sided differences, then
/// trilinearly interpolates to the vertex position. This eliminates stepping
/// artifacts caused by cell-level gradients.
///
/// # Arguments
/// * `samples` - 8 corner SDF samples (same layout as `compute`)
/// * `frac` - Vertex position within cell as [0,1] fractions for x,y,z
///
/// # Corner gradient computation
/// Each corner uses differences to its neighbors within the cell:
/// - Corner 0 (0,0,0): forward differences (s1-s0, s2-s0, s4-s0)
/// - Corner 7 (1,1,1): backward differences (s7-s6, s7-s5, s7-s3)
/// - Mixed corners: appropriate one-sided differences
#[inline]
pub fn compute_interpolated(samples: &[f32; 8], frac: [f32; 3]) -> [f32; 3] {
  // Compute gradient at each corner using one-sided differences
  // Corner layout: bit 0 = X, bit 1 = Y, bit 2 = Z
  let corner_gradients: [Vec3A; 8] = [
    // Corner 0 (0,0,0): all forward
    Vec3A::new(
      samples[1] - samples[0],
      samples[2] - samples[0],
      samples[4] - samples[0],
    ),
    // Corner 1 (1,0,0): X backward, Y/Z forward
    Vec3A::new(
      samples[1] - samples[0],
      samples[3] - samples[1],
      samples[5] - samples[1],
    ),
    // Corner 2 (0,1,0): X forward, Y backward, Z forward
    Vec3A::new(
      samples[3] - samples[2],
      samples[2] - samples[0],
      samples[6] - samples[2],
    ),
    // Corner 3 (1,1,0): X/Y backward, Z forward
    Vec3A::new(
      samples[3] - samples[2],
      samples[3] - samples[1],
      samples[7] - samples[3],
    ),
    // Corner 4 (0,0,1): X/Y forward, Z backward
    Vec3A::new(
      samples[5] - samples[4],
      samples[6] - samples[4],
      samples[4] - samples[0],
    ),
    // Corner 5 (1,0,1): X/Z backward, Y forward
    Vec3A::new(
      samples[5] - samples[4],
      samples[7] - samples[5],
      samples[5] - samples[1],
    ),
    // Corner 6 (0,1,1): X forward, Y/Z backward
    Vec3A::new(
      samples[7] - samples[6],
      samples[6] - samples[4],
      samples[6] - samples[2],
    ),
    // Corner 7 (1,1,1): all backward
    Vec3A::new(
      samples[7] - samples[6],
      samples[7] - samples[5],
      samples[7] - samples[3],
    ),
  ];

  // Trilinear interpolation weights
  let [fx, fy, fz] = frac;
  let fx1 = 1.0 - fx;
  let fy1 = 1.0 - fy;
  let fz1 = 1.0 - fz;

  // Interpolate along X edges (4 pairs)
  let g00 = corner_gradients[0] * fx1 + corner_gradients[1] * fx;
  let g01 = corner_gradients[4] * fx1 + corner_gradients[5] * fx;
  let g10 = corner_gradients[2] * fx1 + corner_gradients[3] * fx;
  let g11 = corner_gradients[6] * fx1 + corner_gradients[7] * fx;

  // Interpolate along Y (2 pairs)
  let g0 = g00 * fy1 + g10 * fy;
  let g1 = g01 * fy1 + g11 * fy;

  // Interpolate along Z (final)
  let gradient = g0 * fz1 + g1 * fz;

  // Normalize
  let len_sq = gradient.length_squared();
  if len_sq < 1e-8 {
    return [0.0, 1.0, 0.0]; // Fallback to up
  }

  let normalized = gradient * len_sq.sqrt().recip();
  [normalized.x, normalized.y, normalized.z]
}

// =============================================================================
// Geometry-based normal recalculation
// =============================================================================

/// Recalculate normals from triangle geometry using angle-weighted averaging.
///
/// Uses Thürmer & Wüthrich's "Mean Weighted by Angle" (MWA) algorithm:
/// Each face's contribution to a vertex normal is weighted by the interior
/// angle of the triangle at that vertex. This produces smoother results than
/// area-weighting for meshes with varying triangle sizes.
///
/// Reference: Thürmer, G. & Wüthrich, C.A. (1998). Computing Vertex Normals
/// from Polygonal Facets. Journal of Graphics Tools, 3(1), 43-46.
#[cfg_attr(
  feature = "tracing",
  tracing::instrument(skip_all, name = "gradient::recalculate_from_geometry")
)]
pub fn recalculate_from_geometry(output: &mut MeshOutput) {
  // Reset all normals
  for vertex in &mut output.vertices {
    vertex.normal = [0.0, 0.0, 0.0];
  }

  // Process triangles (3 indices each)
  let indices = &output.indices;
  let vertices = &mut output.vertices;

  for tri in indices.chunks_exact(3) {
    let i0 = tri[0] as usize;
    let i1 = tri[1] as usize;
    let i2 = tri[2] as usize;

    let p0 = Vec3A::from_array(vertices[i0].position);
    let p1 = Vec3A::from_array(vertices[i1].position);
    let p2 = Vec3A::from_array(vertices[i2].position);

    // Edge vectors from each vertex
    let e01 = p1 - p0;
    let e02 = p2 - p0;
    let e12 = p2 - p1;

    // Face normal (normalized for angle weighting)
    let face_normal = e01.cross(e02);
    let face_len_sq = face_normal.length_squared();

    // Skip degenerate triangles
    if face_len_sq < 1e-12 {
      continue;
    }

    let face_normal_unit = face_normal * face_len_sq.sqrt().recip();

    // Compute angle at each vertex and accumulate weighted normal
    // Angle at v0: between edges e01 and e02
    let angle0 = vertex_angle(e01, e02);
    let weighted0 = face_normal_unit * angle0;
    add_to_normal(&mut vertices[i0].normal, &weighted0.to_array());

    // Angle at v1: between edges -e01 and e12
    let angle1 = vertex_angle(-e01, e12);
    let weighted1 = face_normal_unit * angle1;
    add_to_normal(&mut vertices[i1].normal, &weighted1.to_array());

    // Angle at v2: between edges -e02 and -e12
    let angle2 = vertex_angle(-e02, -e12);
    let weighted2 = face_normal_unit * angle2;
    add_to_normal(&mut vertices[i2].normal, &weighted2.to_array());
  }

  // Normalize all normals
  for vertex in &mut output.vertices {
    let n = Vec3A::from_array(vertex.normal);
    let len_sq = n.length_squared();
    if len_sq < 1e-12 {
      vertex.normal = [0.0, 1.0, 0.0]; // Fallback to up
    } else {
      let normalized = n * len_sq.sqrt().recip();
      vertex.normal = normalized.to_array();
    }
  }
}

/// Compute the angle between two edge vectors at a vertex.
///
/// Returns the angle in radians using the formula: acos(dot(a, b) / (|a| *
/// |b|))
#[inline]
fn vertex_angle(e1: Vec3A, e2: Vec3A) -> f32 {
  let len1_sq = e1.length_squared();
  let len2_sq = e2.length_squared();

  if len1_sq < 1e-12 || len2_sq < 1e-12 {
    return 0.0;
  }

  let dot = e1.dot(e2);
  let cos_angle = dot / (len1_sq.sqrt() * len2_sq.sqrt());

  // Clamp to [-1, 1] to handle floating point errors
  cos_angle.clamp(-1.0, 1.0).acos()
}

#[inline(always)]
fn add_to_normal(normal: &mut [f32; 3], add: &[f32; 3]) {
  normal[0] += add[0];
  normal[1] += add[1];
  normal[2] += add[2];
}

#[cfg(test)]
#[path = "gradient_test.rs"]
mod gradient_test;
