use super::*;

#[test]
fn test_single_material() {
  let materials = [0u8; SAMPLE_SIZE_CB];
  let corner_mask = 0xFF; // All solid

  let weights = compute(&materials, corner_mask, 0);

  assert_eq!(weights, [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn test_mixed_materials() {
  let mut materials = [0u8; SAMPLE_SIZE_CB];

  // Set corners 0-3 to material 0, corners 4-7 to material 1
  for i in 4..8 {
    materials[CORNER_OFFSETS[i]] = 1;
  }

  let corner_mask = 0xFF; // All solid
  let weights = compute(&materials, corner_mask, 0);

  assert!((weights[0] - 0.5).abs() < 0.001);
  assert!((weights[1] - 0.5).abs() < 0.001);
  assert_eq!(weights[2], 0.0);
  assert_eq!(weights[3], 0.0);
}

#[test]
fn test_only_solid_corners_contribute() {
  let mut materials = [0u8; SAMPLE_SIZE_CB];
  materials[CORNER_OFFSETS[0]] = 0;
  materials[CORNER_OFFSETS[1]] = 1; // Air corner

  let corner_mask = 0b00000001; // Only corner 0 is solid
  let weights = compute(&materials, corner_mask, 0);

  assert_eq!(weights, [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn test_material_id_clamping() {
  let materials = [255u8; SAMPLE_SIZE_CB]; // Invalid material IDs
  let corner_mask = 0xFF;

  let weights = compute(&materials, corner_mask, 0);

  // Should clamp to material 3
  assert_eq!(weights, [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn test_weights_sum_to_one() {
  let mut materials = [0u8; SAMPLE_SIZE_CB];
  materials[CORNER_OFFSETS[0]] = 0;
  materials[CORNER_OFFSETS[1]] = 1;
  materials[CORNER_OFFSETS[2]] = 2;
  materials[CORNER_OFFSETS[3]] = 3;

  let corner_mask = 0b00001111; // Corners 0-3 solid
  let weights = compute(&materials, corner_mask, 0);

  let sum: f32 = weights.iter().sum();
  assert!((sum - 1.0).abs() < 0.001);
}
