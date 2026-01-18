//! Simple SDF samplers for testing and debugging.
//!
//! These samplers implement deterministic mathematical SDFs that are
//! easy to verify visually. Use them to test chunk tiling coherency
//! without noise generation complexity.

use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::pipeline::VolumeSampler;
use crate::types::{sdf_conversion, MaterialId, SdfSample};

/// Tilted plane SDF sampler.
///
/// Creates a plane tilted at 45° through the origin.
/// Useful for testing chunk boundary alignment since the surface
/// crosses many chunk boundaries at a predictable angle.
///
/// SDF: `(y - height) * cos(angle) - x * sin(angle)`
/// Default: plane tilted 45° passing through y=0
#[derive(Clone)]
pub struct TiltedPlaneSampler {
  /// Height offset of the plane (default: 0.0)
  pub height: f64,
  /// Tilt angle in radians (default: π/4 = 45°)
  pub angle: f64,
}

impl Default for TiltedPlaneSampler {
  fn default() -> Self {
    Self {
      height: 0.0,
      angle: std::f64::consts::FRAC_PI_4, // 45 degrees
    }
  }
}

impl TiltedPlaneSampler {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_height(mut self, height: f64) -> Self {
    self.height = height;
    self
  }

  pub fn with_angle_degrees(mut self, degrees: f64) -> Self {
    self.angle = degrees.to_radians();
    self
  }
}

impl VolumeSampler for TiltedPlaneSampler {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    let cos_a = self.angle.cos();
    let sin_a = self.angle.sin();

    for xi in 0..SAMPLE_SIZE {
      for yi in 0..SAMPLE_SIZE {
        for zi in 0..SAMPLE_SIZE {
          // World position = (grid_offset + sample_index) * voxel_size
          let wx = (grid_offset[0] + xi as i64) as f64 * voxel_size;
          let wy = (grid_offset[1] + yi as i64) as f64 * voxel_size;
          let _wz = (grid_offset[2] + zi as i64) as f64 * voxel_size;

          // Tilted plane SDF: distance to plane tilted around Z axis
          // Plane normal = (sin(angle), cos(angle), 0)
          let sdf = (wy - self.height) * cos_a - wx * sin_a;

          let idx = xi * SAMPLE_SIZE * SAMPLE_SIZE + yi * SAMPLE_SIZE + zi;
          volume[idx] = sdf_conversion::to_storage(sdf as f32, voxel_size as f32);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Sphere SDF sampler.
///
/// Creates a sphere centered at origin with given radius.
/// Simple test case with radial symmetry.
#[derive(Clone)]
pub struct SphereSampler {
  /// Center of the sphere in world coordinates
  pub center: [f64; 3],
  /// Radius of the sphere
  pub radius: f64,
}

impl Default for SphereSampler {
  fn default() -> Self {
    Self {
      center: [0.0, 0.0, 0.0],
      radius: 20.0,
    }
  }
}

impl SphereSampler {
  pub fn new(radius: f64) -> Self {
    Self {
      center: [0.0, 0.0, 0.0],
      radius,
    }
  }

  pub fn with_center(mut self, center: [f64; 3]) -> Self {
    self.center = center;
    self
  }
}

impl VolumeSampler for SphereSampler {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    for xi in 0..SAMPLE_SIZE {
      for yi in 0..SAMPLE_SIZE {
        for zi in 0..SAMPLE_SIZE {
          // World position = (grid_offset + sample_index) * voxel_size
          let wx = (grid_offset[0] + xi as i64) as f64 * voxel_size;
          let wy = (grid_offset[1] + yi as i64) as f64 * voxel_size;
          let wz = (grid_offset[2] + zi as i64) as f64 * voxel_size;

          // Sphere SDF: distance to surface = |p - center| - radius
          let dx = wx - self.center[0];
          let dy = wy - self.center[1];
          let dz = wz - self.center[2];
          let dist = (dx * dx + dy * dy + dz * dz).sqrt();
          let sdf = dist - self.radius;

          let idx = xi * SAMPLE_SIZE * SAMPLE_SIZE + yi * SAMPLE_SIZE + zi;
          volume[idx] = sdf_conversion::to_storage(sdf as f32, voxel_size as f32);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Horizontal plane sampler (ground plane).
///
/// Simple flat plane at a given height. Good baseline test.
#[derive(Clone)]
pub struct GroundPlaneSampler {
  /// Height of the ground plane
  pub height: f64,
}

impl Default for GroundPlaneSampler {
  fn default() -> Self {
    Self { height: 0.0 }
  }
}

impl GroundPlaneSampler {
  pub fn new(height: f64) -> Self {
    Self { height }
  }
}

impl VolumeSampler for GroundPlaneSampler {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    for xi in 0..SAMPLE_SIZE {
      for yi in 0..SAMPLE_SIZE {
        for zi in 0..SAMPLE_SIZE {
          // World position Y
          let wy = (grid_offset[1] + yi as i64) as f64 * voxel_size;

          // Ground plane SDF: positive above, negative below
          let sdf = wy - self.height;

          let idx = xi * SAMPLE_SIZE * SAMPLE_SIZE + yi * SAMPLE_SIZE + zi;
          volume[idx] = sdf_conversion::to_storage(sdf as f32, voxel_size as f32);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Box SDF sampler.
///
/// Axis-aligned box centered at origin.
#[derive(Clone)]
pub struct BoxSampler {
  /// Center of the box
  pub center: [f64; 3],
  /// Half-extents (half-size in each dimension)
  pub half_extents: [f64; 3],
}

impl Default for BoxSampler {
  fn default() -> Self {
    Self {
      center: [0.0, 0.0, 0.0],
      half_extents: [10.0, 10.0, 10.0],
    }
  }
}

impl BoxSampler {
  pub fn new(half_extents: [f64; 3]) -> Self {
    Self {
      center: [0.0, 0.0, 0.0],
      half_extents,
    }
  }

  pub fn with_center(mut self, center: [f64; 3]) -> Self {
    self.center = center;
    self
  }
}

impl VolumeSampler for BoxSampler {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    for xi in 0..SAMPLE_SIZE {
      for yi in 0..SAMPLE_SIZE {
        for zi in 0..SAMPLE_SIZE {
          let wx = (grid_offset[0] + xi as i64) as f64 * voxel_size;
          let wy = (grid_offset[1] + yi as i64) as f64 * voxel_size;
          let wz = (grid_offset[2] + zi as i64) as f64 * voxel_size;

          // Box SDF
          let dx = (wx - self.center[0]).abs() - self.half_extents[0];
          let dy = (wy - self.center[1]).abs() - self.half_extents[1];
          let dz = (wz - self.center[2]).abs() - self.half_extents[2];

          let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2) + dz.max(0.0).powi(2)).sqrt();
          let inside = dx.max(dy).max(dz).min(0.0);
          let sdf = outside + inside;

          let idx = xi * SAMPLE_SIZE * SAMPLE_SIZE + yi * SAMPLE_SIZE + zi;
          volume[idx] = sdf_conversion::to_storage(sdf as f32, voxel_size as f32);
          materials[idx] = 0;
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tilted_plane_crosses_origin() {
    let sampler = TiltedPlaneSampler::default();
    let mut volume = [0i8; SAMPLE_SIZE_CB];
    let mut materials = [0u8; SAMPLE_SIZE_CB];

    // Sample around origin
    sampler.sample_volume([0, 0, 0], 1.0, &mut volume, &mut materials);

    // Should have both positive and negative values (surface crosses volume)
    let has_positive = volume.iter().any(|&v| v > 0);
    let has_negative = volume.iter().any(|&v| v < 0);
    assert!(has_positive && has_negative, "Tilted plane should cross the volume");
  }

  #[test]
  fn sphere_surface_exists() {
    let sampler = SphereSampler::new(10.0);
    let mut volume = [0i8; SAMPLE_SIZE_CB];
    let mut materials = [0u8; SAMPLE_SIZE_CB];

    // Sample around origin where sphere surface should be
    sampler.sample_volume([-16, -16, -16], 1.0, &mut volume, &mut materials);

    // Should have both inside (negative) and outside (positive)
    let has_positive = volume.iter().any(|&v| v > 0);
    let has_negative = volume.iter().any(|&v| v < 0);
    assert!(has_positive && has_negative, "Sphere surface should cross the volume");
  }

  #[test]
  fn ground_plane_splits_volume() {
    let sampler = GroundPlaneSampler::new(16.0);
    let mut volume = [0i8; SAMPLE_SIZE_CB];
    let mut materials = [0u8; SAMPLE_SIZE_CB];

    sampler.sample_volume([0, 0, 0], 1.0, &mut volume, &mut materials);

    // Should have values above (positive) and below (negative) the plane
    let has_positive = volume.iter().any(|&v| v > 0);
    let has_negative = volume.iter().any(|&v| v < 0);
    assert!(has_positive && has_negative, "Ground plane should split the volume");
  }
}
