use super::*;

// Reference scalar for test comparison
fn reference_scalar(samples: [i8; 8]) -> u8 {
  let mut corner_mask = 0u8;
  for (i, &sample) in samples.iter().enumerate() {
    if sample < 0 {
      corner_mask |= 1 << i;
    }
  }
  corner_mask
}

#[test]
fn test_all_positive() {
  let samples = [1, 2, 3, 4, 5, 6, 7, 8];
  assert_eq!(build(samples), 0b00000000);
}

#[test]
fn test_all_negative() {
  let samples = [-1, -2, -3, -4, -5, -6, -7, -8];
  assert_eq!(build(samples), 0b11111111);
}

#[test]
fn test_mixed() {
  // Corners 0, 2, 4, 6 negative (checkerboard pattern)
  let samples = [-1, 1, -1, 1, -1, 1, -1, 1];
  assert_eq!(build(samples), 0b01010101);
}

#[test]
fn test_first_corner_only() {
  let samples = [-1, 1, 1, 1, 1, 1, 1, 1];
  assert_eq!(build(samples), 0b00000001);
}

#[test]
fn test_last_corner_only() {
  let samples = [1, 1, 1, 1, 1, 1, 1, -1];
  assert_eq!(build(samples), 0b10000000);
}

#[test]
fn test_zero_is_positive() {
  // Zero should NOT be considered "inside" (negative)
  let samples = [0, 0, 0, 0, 0, 0, 0, 0];
  assert_eq!(build(samples), 0b00000000);
}

#[test]
fn test_boundary_values() {
  let samples = [-128, 127, -1, 0, 1, -127, 126, -126];
  // Negative: -128, -1, -127, -126 at positions 0, 2, 5, 7
  assert_eq!(build(samples), 0b10100101);
}

#[test]
fn test_matches_reference() {
  // Exhaustive check for all patterns
  for pattern in 0u8..=255 {
    let samples: [i8; 8] = std::array::from_fn(|i| if (pattern >> i) & 1 == 1 { -1 } else { 1 });
    assert_eq!(
      build(samples),
      reference_scalar(samples),
      "Mismatch for pattern {:#010b}",
      pattern
    );
  }
}
