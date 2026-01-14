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
