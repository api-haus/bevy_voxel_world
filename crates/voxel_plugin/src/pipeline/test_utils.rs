//! Test utilities for pipeline tests.
//!
//! Provides mock volume samplers and fixture generators for testing
//! each pipeline stage in isolation.

use std::sync::atomic::{AtomicUsize, Ordering};

use glam::DVec3;

use super::types::{MeshResult, VolumeSampler, WorkSource};
use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::octree::{OctreeConfig, OctreeNode, TransitionGroup};
use crate::surface_nets;
use crate::types::{sdf_conversion, MaterialId, MeshConfig, MeshOutput, SdfSample};

// =============================================================================
// Mock Volume Samplers
// =============================================================================

/// Sphere SDF centered at a point.
///
/// Returns negative inside the sphere, positive outside.
pub struct SphereSampler {
  pub center: DVec3,
  pub radius: f64,
}

impl SphereSampler {
  pub fn new(center: DVec3, radius: f64) -> Self {
    Self { center, radius }
  }

  /// Create a sphere at origin with given radius.
  pub fn at_origin(radius: f64) -> Self {
    Self::new(DVec3::ZERO, radius)
  }

  /// Sample single point (used internally).
  fn sample_point(&self, world_pos: DVec3) -> SdfSample {
    let dist = (world_pos - self.center).length() - self.radius;
    sdf_conversion::to_storage(dist as f32)
  }
}

impl VolumeSampler for SphereSampler {
  fn sample_volume(
    &self,
    sample_start: [f64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    let start = DVec3::from_array(sample_start);
    for x in 0..SAMPLE_SIZE {
      for y in 0..SAMPLE_SIZE {
        for z in 0..SAMPLE_SIZE {
          let idx = x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;
          let world_pos = start + DVec3::new(x as f64, y as f64, z as f64) * voxel_size;
          volume[idx] = self.sample_point(world_pos);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Constant SDF sampler - returns same value everywhere.
///
/// Use positive for "all air", negative for "all solid".
pub struct ConstantSampler {
  pub value: SdfSample,
  pub material: MaterialId,
}

impl ConstantSampler {
  /// All air (positive SDF).
  pub fn all_air() -> Self {
    Self {
      value: 127,
      material: 0,
    }
  }

  /// All solid (negative SDF).
  pub fn all_solid() -> Self {
    Self {
      value: -127,
      material: 0,
    }
  }

  /// Specific value.
  pub fn with_value(value: SdfSample) -> Self {
    Self { value, material: 0 }
  }
}

impl VolumeSampler for ConstantSampler {
  fn sample_volume(
    &self,
    _sample_start: [f64; 3],
    _voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    volume.fill(self.value);
    materials.fill(self.material);
  }
}

/// Plane SDF - divides space by a plane.
///
/// Returns negative below the plane, positive above.
pub struct PlaneSampler {
  /// Point on the plane.
  pub point: DVec3,
  /// Normal direction (points toward positive/air side).
  pub normal: DVec3,
}

impl PlaneSampler {
  /// Horizontal plane at given Y height.
  pub fn horizontal(y: f64) -> Self {
    Self {
      point: DVec3::new(0.0, y, 0.0),
      normal: DVec3::Y,
    }
  }

  /// Sample single point (used internally).
  fn sample_point(&self, world_pos: DVec3) -> SdfSample {
    let dist = (world_pos - self.point).dot(self.normal);
    sdf_conversion::to_storage(dist as f32)
  }
}

impl VolumeSampler for PlaneSampler {
  fn sample_volume(
    &self,
    sample_start: [f64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    let start = DVec3::from_array(sample_start);
    for x in 0..SAMPLE_SIZE {
      for y in 0..SAMPLE_SIZE {
        for z in 0..SAMPLE_SIZE {
          let idx = x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;
          let world_pos = start + DVec3::new(x as f64, y as f64, z as f64) * voxel_size;
          volume[idx] = self.sample_point(world_pos);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Corner-controlled sampler for testing specific corner configurations.
///
/// Samples 8 corner values, interpolates linearly between them.
pub struct CornerSampler {
  /// SDF values at corners [0..8] in standard corner order.
  /// Corner order: (0,0,0), (1,0,0), (0,1,0), (1,1,0), (0,0,1), (1,0,1),
  /// (0,1,1), (1,1,1)
  pub corners: [SdfSample; 8],
  /// Chunk bounds for interpolation.
  pub min: DVec3,
  pub max: DVec3,
}

impl CornerSampler {
  /// All corners negative (all solid).
  pub fn all_negative() -> Self {
    Self {
      corners: [-100; 8],
      min: DVec3::ZERO,
      max: DVec3::splat(32.0),
    }
  }

  /// All corners positive (all air).
  pub fn all_positive() -> Self {
    Self {
      corners: [100; 8],
      min: DVec3::ZERO,
      max: DVec3::splat(32.0),
    }
  }

  /// Mixed corners for surface crossing.
  pub fn mixed() -> Self {
    Self {
      corners: [-100, 100, -100, 100, -100, 100, -100, 100],
      min: DVec3::ZERO,
      max: DVec3::splat(32.0),
    }
  }

  /// 7 negative, 1 positive (surface at one corner).
  pub fn mostly_solid() -> Self {
    Self {
      corners: [-100, -100, -100, -100, -100, -100, -100, 100],
      min: DVec3::ZERO,
      max: DVec3::splat(32.0),
    }
  }

  /// 7 negative, 1 exactly zero (on surface).
  pub fn with_zero_corner() -> Self {
    Self {
      corners: [-100, -100, -100, -100, -100, -100, -100, 0],
      min: DVec3::ZERO,
      max: DVec3::splat(32.0),
    }
  }

  /// Set bounds for a specific node.
  pub fn for_node(mut self, node: &OctreeNode, config: &OctreeConfig) -> Self {
    let min = config.get_node_min(node);
    let cell_size = config.get_cell_size(node.lod);
    let max = min + DVec3::splat(cell_size);
    self.min = min;
    self.max = max;
    self
  }

  /// Sample single point with trilinear interpolation.
  fn sample_point(&self, world_pos: DVec3) -> SdfSample {
    // Normalize position to [0, 1] within chunk bounds
    let size = self.max - self.min;
    let t = (world_pos - self.min) / size;
    let t = t.clamp(DVec3::ZERO, DVec3::ONE);

    // Trilinear interpolation
    let c = &self.corners;
    let x0 = (1.0 - t.x) * c[0] as f64 + t.x * c[1] as f64;
    let x1 = (1.0 - t.x) * c[2] as f64 + t.x * c[3] as f64;
    let x2 = (1.0 - t.x) * c[4] as f64 + t.x * c[5] as f64;
    let x3 = (1.0 - t.x) * c[6] as f64 + t.x * c[7] as f64;

    let y0 = (1.0 - t.y) * x0 + t.y * x1;
    let y1 = (1.0 - t.y) * x2 + t.y * x3;

    let z = (1.0 - t.z) * y0 + t.z * y1;

    z.clamp(-127.0, 127.0) as SdfSample
  }
}

impl VolumeSampler for CornerSampler {
  fn sample_volume(
    &self,
    sample_start: [f64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    let start = DVec3::from_array(sample_start);
    for x in 0..SAMPLE_SIZE {
      for y in 0..SAMPLE_SIZE {
        for z in 0..SAMPLE_SIZE {
          let idx = x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;
          let world_pos = start + DVec3::new(x as f64, y as f64, z as f64) * voxel_size;
          volume[idx] = self.sample_point(world_pos);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Counting sampler wrapper - tracks number of sample_volume() calls.
///
/// Useful for verifying optimization (corner-only vs full-volume sampling).
pub struct CountingSampler<S: VolumeSampler> {
  pub inner: S,
  pub sample_count: AtomicUsize,
}

impl<S: VolumeSampler> CountingSampler<S> {
  pub fn new(inner: S) -> Self {
    Self {
      inner,
      sample_count: AtomicUsize::new(0),
    }
  }

  pub fn count(&self) -> usize {
    self.sample_count.load(Ordering::SeqCst)
  }

  pub fn reset(&self) {
    self.sample_count.store(0, Ordering::SeqCst);
  }
}

impl<S: VolumeSampler> VolumeSampler for CountingSampler<S> {
  fn sample_volume(
    &self,
    sample_start: [f64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    self.sample_count.fetch_add(1, Ordering::SeqCst);
    self
      .inner
      .sample_volume(sample_start, voxel_size, volume, materials)
  }
}

// =============================================================================
// Test Fixtures
// =============================================================================

/// Create a default OctreeConfig for testing.
pub fn test_config() -> OctreeConfig {
  OctreeConfig {
    voxel_size: 1.0,
    min_lod: 0,
    max_lod: 5,
    ..OctreeConfig::default()
  }
}

/// Create a TransitionGroup for subdivide testing.
pub fn subdivide_fixture(lod: i32) -> TransitionGroup {
  let parent = OctreeNode::new(0, 0, 0, lod);
  TransitionGroup::new_subdivide(parent).expect("Should create subdivide group")
}

/// Create a TransitionGroup for merge testing.
pub fn merge_fixture(lod: i32) -> TransitionGroup {
  let parent = OctreeNode::new(0, 0, 0, lod);
  let children = (0..8u8)
    .filter_map(|octant| parent.get_child(octant))
    .collect();
  TransitionGroup::new_merge(parent, children).expect("Should create merge group")
}

/// Generate a sphere volume for testing meshing.
pub fn make_sphere_volume(
  radius: f32,
) -> (
  Box<[SdfSample; SAMPLE_SIZE_CB]>,
  Box<[MaterialId; SAMPLE_SIZE_CB]>,
) {
  let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
  let mut materials = Box::new([0u8; SAMPLE_SIZE_CB]);

  let center = 16.0f32;
  for z in 0..32 {
    for y in 0..32 {
      for x in 0..32 {
        let idx = x + y * 32 + z * 32 * 32;
        let dx = x as f32 - center;
        let dy = y as f32 - center;
        let dz = z as f32 - center;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt() - radius;
        volume[idx] = sdf_conversion::to_storage(dist);
        materials[idx] = 0;
      }
    }
  }

  (volume, materials)
}

/// Generate a sphere mesh output for testing.
pub fn make_sphere_mesh() -> MeshOutput {
  let (volume, materials) = make_sphere_volume(12.0);
  surface_nets::generate(&volume, &materials, &MeshConfig::default())
}

/// Create mock MeshResults for all children of a parent.
pub fn child_mesh_results(parent: &OctreeNode) -> Vec<MeshResult> {
  (0..8u8)
    .filter_map(|octant| {
      let child = parent.get_child(octant)?;
      Some(MeshResult {
        node: child,
        output: make_sphere_mesh(),
        timing_us: 100,
        work_source: WorkSource::Refinement,
      })
    })
    .collect()
}

/// Create a single mock MeshResult for a node.
pub fn mock_mesh_result(node: OctreeNode, work_source: WorkSource) -> MeshResult {
  MeshResult {
    node,
    output: make_sphere_mesh(),
    timing_us: 100,
    work_source,
  }
}
