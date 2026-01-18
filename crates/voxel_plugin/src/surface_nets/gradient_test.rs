use super::*;

fn approx_eq(a: [f32; 3], b: [f32; 3], epsilon: f32) -> bool {
  (a[0] - b[0]).abs() < epsilon && (a[1] - b[1]).abs() < epsilon && (a[2] - b[2]).abs() < epsilon
}

// Reference scalar for test comparison
fn reference_scalar(samples: &[f32; 8]) -> [f32; 3] {
  let dx = (samples[1] + samples[3] + samples[5] + samples[7])
    - (samples[0] + samples[2] + samples[4] + samples[6]);
  let dy = (samples[2] + samples[3] + samples[6] + samples[7])
    - (samples[0] + samples[1] + samples[4] + samples[5]);
  let dz = (samples[4] + samples[5] + samples[6] + samples[7])
    - (samples[0] + samples[1] + samples[2] + samples[3]);

  let length_sq = dx * dx + dy * dy + dz * dz;
  if length_sq < 1e-8 {
    return [0.0, 1.0, 0.0];
  }
  let inv_length = 1.0 / length_sq.sqrt();
  [dx * inv_length, dy * inv_length, dz * inv_length]
}

#[test]
fn test_gradient_flat_x() {
  // All negative on left, all positive on right
  let samples = [-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0];
  let result = compute(&samples);

  assert!((result[0] - 1.0).abs() < 0.01);
  assert!(result[1].abs() < 0.01);
  assert!(result[2].abs() < 0.01);
}

#[test]
fn test_gradient_flat_y() {
  // Bottom negative, top positive
  let samples = [-1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0];
  let result = compute(&samples);

  assert!(result[0].abs() < 0.01);
  assert!((result[1] - 1.0).abs() < 0.01);
  assert!(result[2].abs() < 0.01);
}

#[test]
fn test_gradient_flat_z() {
  // Back negative, front positive
  let samples = [-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0];
  let result = compute(&samples);

  assert!(result[0].abs() < 0.01);
  assert!(result[1].abs() < 0.01);
  assert!((result[2] - 1.0).abs() < 0.01);
}

#[test]
fn test_gradient_degenerate() {
  let samples = [0.0; 8];
  let result = compute(&samples);

  // Should return fallback up vector
  assert_eq!(result, [0.0, 1.0, 0.0]);
}

#[test]
fn test_matches_reference() {
  // Test various gradient directions
  let test_cases = [
    [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0],
    [-2.0, 3.0, -1.0, 4.0, -3.0, 2.0, -4.0, 1.0],
    [0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5],
  ];

  for samples in test_cases {
    let result = compute(&samples);
    let reference = reference_scalar(&samples);
    assert!(
      approx_eq(result, reference, 0.0001),
      "Mismatch: result={:?}, reference={:?}",
      result,
      reference
    );
  }
}

// =============================================================================
// Interpolated gradient tests
// =============================================================================

#[test]
fn test_interpolated_at_center_matches_original() {
  // At cell center (0.5, 0.5, 0.5), interpolated should approximate original
  let samples = [-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0];
  let original = compute(&samples);
  let interpolated = compute_interpolated(&samples, [0.5, 0.5, 0.5]);

  // Should be close (not exact due to different computation methods)
  assert!(
    approx_eq(original, interpolated, 0.2),
    "Center mismatch: original={:?}, interpolated={:?}",
    original,
    interpolated
  );
}

#[test]
fn test_interpolated_varies_with_position() {
  // For a surface with gradient in X, normals at different positions should differ
  let samples = [-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0];

  let at_left = compute_interpolated(&samples, [0.1, 0.5, 0.5]);
  let at_right = compute_interpolated(&samples, [0.9, 0.5, 0.5]);

  // Both should point roughly in +X but may have slight differences
  assert!(at_left[0] > 0.9, "Left should point +X: {:?}", at_left);
  assert!(at_right[0] > 0.9, "Right should point +X: {:?}", at_right);
}

#[test]
fn test_interpolated_flat_surfaces() {
  // Flat X surface: left face negative, right face positive
  let samples_x = [-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0];
  let result_x = compute_interpolated(&samples_x, [0.5, 0.5, 0.5]);
  assert!(result_x[0] > 0.9, "X gradient should point +X: {:?}", result_x);

  // Flat Y surface
  let samples_y = [-1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0];
  let result_y = compute_interpolated(&samples_y, [0.5, 0.5, 0.5]);
  assert!(result_y[1] > 0.9, "Y gradient should point +Y: {:?}", result_y);

  // Flat Z surface
  let samples_z = [-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0];
  let result_z = compute_interpolated(&samples_z, [0.5, 0.5, 0.5]);
  assert!(result_z[2] > 0.9, "Z gradient should point +Z: {:?}", result_z);
}

#[test]
fn test_interpolated_degenerate() {
  let samples = [0.0; 8];
  let result = compute_interpolated(&samples, [0.5, 0.5, 0.5]);

  // Should return fallback up vector
  assert_eq!(result, [0.0, 1.0, 0.0]);
}

#[test]
fn test_interpolated_corners_use_one_sided_differences() {
  // At corner 0 (0,0,0), gradient should use forward differences
  let samples = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

  let at_corner_0 = compute_interpolated(&samples, [0.0, 0.0, 0.0]);
  // Forward diffs at corner 0: gx=s1-s0=1, gy=s2-s0=2, gz=s4-s0=4
  // Normalized: (1,2,4)/sqrt(21) ≈ (0.218, 0.436, 0.873)
  let expected_len = (1.0f32 + 4.0 + 16.0).sqrt();
  let expected = [1.0 / expected_len, 2.0 / expected_len, 4.0 / expected_len];
  assert!(
    approx_eq(at_corner_0, expected, 0.01),
    "Corner 0 mismatch: got={:?}, expected={:?}",
    at_corner_0,
    expected
  );
}
