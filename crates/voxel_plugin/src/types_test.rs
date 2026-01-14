use sdf_conversion::*;

use super::*;

// SDF conversion tests
#[test]
fn test_roundtrip_zero() {
  assert_eq!(to_float(to_storage(0.0)), 0.0);
}

#[test]
fn test_roundtrip_positive() {
  let sdf = 5.0;
  let stored = to_storage(sdf);
  let recovered = to_float(stored);
  // Should be within one quantization level
  assert!((sdf - recovered).abs() < INV_SCALE * 1.5);
}

#[test]
fn test_roundtrip_negative() {
  let sdf = -3.5;
  let stored = to_storage(sdf);
  let recovered = to_float(stored);
  assert!((sdf - recovered).abs() < INV_SCALE * 1.5);
}

#[test]
fn test_clamping() {
  // Values beyond ±10 should clamp to ±127
  assert_eq!(to_storage(100.0), 127);
  assert_eq!(to_storage(-100.0), -127);
}

#[test]
fn test_scale_factor() {
  // Scale should map ±10 to ±127
  assert!((SCALE - 12.7).abs() < 0.01);
  assert_eq!(to_storage(10.0), 127);
  assert_eq!(to_storage(-10.0), -127);
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
