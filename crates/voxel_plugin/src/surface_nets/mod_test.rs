use super::*;
use crate::types::sdf_conversion;

fn create_sphere_sdf(radius: f32, center: [f32; 3]) -> [SdfSample; SAMPLE_SIZE_CB] {
  let mut volume = [0i8; SAMPLE_SIZE_CB];
  for x in 0..SAMPLE_SIZE {
    for y in 0..SAMPLE_SIZE {
      for z in 0..SAMPLE_SIZE {
        let dx = x as f32 - center[0];
        let dy = y as f32 - center[1];
        let dz = z as f32 - center[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let sdf = dist - radius;
        // Use proper SDF scaling for storage
        volume[coord_to_index(x, y, z)] = sdf_conversion::to_storage(sdf);
      }
    }
  }
  volume
}

#[test]
fn test_empty_volume_produces_no_mesh() {
  let volume = [127i8; SAMPLE_SIZE_CB]; // All positive = air
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::default();

  let output = generate(&volume, &materials, &config);

  assert!(output.is_empty());
  assert_eq!(output.triangle_count(), 0);
}

#[test]
fn test_solid_volume_produces_no_mesh() {
  let volume = [-127i8; SAMPLE_SIZE_CB]; // All negative = solid
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::default();

  let output = generate(&volume, &materials, &config);

  assert!(output.is_empty());
  assert_eq!(output.triangle_count(), 0);
}

#[test]
fn test_sphere_produces_mesh() {
  let volume = create_sphere_sdf(10.0, [16.0, 16.0, 16.0]);
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::default();

  let output = generate(&volume, &materials, &config);

  assert!(!output.is_empty());
  assert!(
    output.vertices.len() > 100,
    "Expected many vertices, got {}",
    output.vertices.len()
  );
  assert!(
    output.triangle_count() > 100,
    "Expected many triangles, got {}",
    output.triangle_count()
  );
  assert!(output.bounds.is_valid());
}

#[test]
fn test_indices_are_valid() {
  let volume = create_sphere_sdf(8.0, [16.0, 16.0, 16.0]);
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::default();

  let output = generate(&volume, &materials, &config);

  for &idx in &output.indices {
    assert!(
      (idx as usize) < output.vertices.len(),
      "Invalid index {} with {} vertices",
      idx,
      output.vertices.len()
    );
  }
}

#[test]
fn test_indices_are_triangles() {
  let volume = create_sphere_sdf(8.0, [16.0, 16.0, 16.0]);
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::default();

  let output = generate(&volume, &materials, &config);

  assert_eq!(
    output.indices.len() % 3,
    0,
    "Index count must be multiple of 3"
  );
}

// =============================================================================
// Normal mode tests
// =============================================================================

fn normals_are_normalized(output: &MeshOutput) -> bool {
  for vertex in &output.vertices {
    let len = (vertex.normal[0] * vertex.normal[0]
      + vertex.normal[1] * vertex.normal[1]
      + vertex.normal[2] * vertex.normal[2])
      .sqrt();
    if (len - 1.0).abs() > 0.01 {
      return false;
    }
  }
  true
}

#[test]
fn test_normal_mode_gradient() {
  let volume = create_sphere_sdf(8.0, [16.0, 16.0, 16.0]);
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::new().with_normal_mode(NormalMode::Gradient);

  let output = generate(&volume, &materials, &config);

  assert!(!output.is_empty());
  assert!(
    normals_are_normalized(&output),
    "Gradient normals should be normalized"
  );
}

#[test]
fn test_normal_mode_geometry() {
  let volume = create_sphere_sdf(8.0, [16.0, 16.0, 16.0]);
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::new().with_normal_mode(NormalMode::Geometry);

  let output = generate(&volume, &materials, &config);

  assert!(!output.is_empty());
  assert!(
    normals_are_normalized(&output),
    "Geometry normals should be normalized"
  );
}

#[test]
fn test_normal_mode_blended() {
  let volume = create_sphere_sdf(8.0, [16.0, 16.0, 16.0]);
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::new().with_normal_mode(NormalMode::Blended {
    blend_distance: 3.0,
  });

  let output = generate(&volume, &materials, &config);

  assert!(!output.is_empty());
  assert!(
    normals_are_normalized(&output),
    "Blended normals should be normalized"
  );
}

#[test]
fn test_normal_modes_produce_different_results() {
  let volume = create_sphere_sdf(8.0, [16.0, 16.0, 16.0]);
  let materials = [0u8; SAMPLE_SIZE_CB];

  let output_grad = generate(
    &volume,
    &materials,
    &MeshConfig::new().with_normal_mode(NormalMode::Gradient),
  );
  let output_geom = generate(
    &volume,
    &materials,
    &MeshConfig::new().with_normal_mode(NormalMode::Geometry),
  );

  // Both should have same number of vertices (geometry is identical)
  assert_eq!(output_grad.vertices.len(), output_geom.vertices.len());

  // But normals should differ (at least some)
  let mut differ = false;

  for i in 0..output_grad.vertices.len() {
    let n1 = output_grad.vertices[i].normal;
    let n2 = output_geom.vertices[i].normal;

    let diff = (n1[0] - n2[0]).abs() + (n1[1] - n2[1]).abs() + (n1[2] - n2[2]).abs();

    if diff > 0.01 {
      differ = true;
      break;
    }
  }

  assert!(
    differ,
    "Gradient and Geometry should produce different normals"
  );
}
