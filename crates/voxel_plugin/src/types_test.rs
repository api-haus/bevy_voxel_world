use sdf_conversion::*;

use super::*;

// Use voxel_size=1.0 for tests (SDF in world units = SDF in voxel units)
const TEST_VOXEL_SIZE: f32 = 1.0;

// SDF conversion tests
#[test]
fn test_roundtrip_zero() {
  assert_eq!(to_float(to_storage(0.0, TEST_VOXEL_SIZE), TEST_VOXEL_SIZE), 0.0);
}

#[test]
fn test_roundtrip_positive() {
  // Use value within range (±RANGE_VOXELS = ±1.0)
  let sdf = 0.5;
  let stored = to_storage(sdf, TEST_VOXEL_SIZE);
  let recovered = to_float(stored, TEST_VOXEL_SIZE);
  // Should be within one quantization level
  let inv_scale = RANGE_VOXELS / 127.0;
  assert!((sdf - recovered).abs() < inv_scale * 1.5);
}

#[test]
fn test_roundtrip_negative() {
  // Use value within range (±RANGE_VOXELS = ±1.0)
  let sdf = -0.5;
  let stored = to_storage(sdf, TEST_VOXEL_SIZE);
  let recovered = to_float(stored, TEST_VOXEL_SIZE);
  let inv_scale = RANGE_VOXELS / 127.0;
  assert!((sdf - recovered).abs() < inv_scale * 1.5);
}

#[test]
fn test_clamping() {
  // Values beyond ±RANGE_VOXELS should clamp to ±127
  assert_eq!(to_storage(100.0, TEST_VOXEL_SIZE), 127);
  assert_eq!(to_storage(-100.0, TEST_VOXEL_SIZE), -127);
}

#[test]
fn test_scale_factor() {
  // Scale should map ±RANGE_VOXELS (±1.0) to ±127
  // BASE_SCALE = 127.0 / RANGE_VOXELS = 127.0 / 1.0 = 127.0
  assert!((BASE_SCALE - 127.0).abs() < 0.01);
  assert_eq!(to_storage(RANGE_VOXELS, TEST_VOXEL_SIZE), 127);
  assert_eq!(to_storage(-RANGE_VOXELS, TEST_VOXEL_SIZE), -127);
}

#[test]
fn test_voxel_size_scaling() {
  // With voxel_size=2.0, an SDF of 2.0 world units = 1.0 voxel units
  // Should produce same quantized value as sdf=1.0 with voxel_size=1.0
  let stored_small = to_storage(1.0, 1.0);
  let stored_large = to_storage(2.0, 2.0);
  assert_eq!(stored_small, stored_large);

  // And recover correctly
  let recovered = to_float(stored_large, 2.0);
  assert!((recovered - 2.0).abs() < 0.2); // Allow for quantization error
}

// General types tests
#[test]
fn test_aabb_encapsulate() {
  let mut aabb = MinMaxAABB::empty();
  aabb.encapsulate([1.0, 2.0, 3.0]);
  aabb.encapsulate([-1.0, -2.0, -3.0]);

  assert_eq!(aabb.min, [-1.0, -2.0, -3.0]);
  assert_eq!(aabb.max, [1.0, 2.0, 3.0]);
  assert!(aabb.is_valid());
}

#[test]
fn test_mesh_output_clear() {
  let mut output = MeshOutput::new();
  output.vertices.push(Vertex::default());
  output.indices.push(0);
  output.clear();

  assert!(output.is_empty());
  assert_eq!(output.triangle_count(), 0);
}

#[test]
fn test_mesh_config_builder() {
  let config = MeshConfig::new()
    .with_voxel_size(2.0)
    .with_neighbor_mask(0xFF)
    .with_normal_mode(NormalMode::Geometry);

  assert_eq!(config.voxel_size, 2.0);
  assert_eq!(config.neighbor_mask, 0xFF);
  assert_eq!(config.normal_mode, NormalMode::Geometry);
}

#[test]
fn test_normal_mode_variants() {
  // Test all NormalMode variants can be set
  let config_grad = MeshConfig::new().with_normal_mode(NormalMode::Gradient);
  assert_eq!(config_grad.normal_mode, NormalMode::Gradient);

  let config_geom = MeshConfig::new().with_normal_mode(NormalMode::Geometry);
  assert_eq!(config_geom.normal_mode, NormalMode::Geometry);

  let config_blend = MeshConfig::new().with_normal_mode(NormalMode::Blended {
    blend_distance: 3.0,
  });
  match config_blend.normal_mode {
    NormalMode::Blended { blend_distance } => {
      assert_eq!(blend_distance, 3.0);
    }
    _ => panic!("Expected Blended mode"),
  }
}
