use super::*;

#[test]
fn test_sample_size_is_power_of_two() {
  assert!(SAMPLE_SIZE.is_power_of_two());
  assert_eq!(SAMPLE_SIZE, 32);
}

#[test]
fn test_coord_to_index_roundtrip() {
  for x in 0..SAMPLE_SIZE {
    for y in 0..SAMPLE_SIZE {
      for z in 0..SAMPLE_SIZE {
        let idx = coord_to_index(x, y, z);
        let (rx, ry, rz) = index_to_coord(idx);
        assert_eq!(
          (x, y, z),
          (rx, ry, rz),
          "Roundtrip failed for ({}, {}, {})",
          x,
          y,
          z
        );
      }
    }
  }
}

#[test]
fn test_corner_offsets() {
  // Verify corner 0 is at base
  assert_eq!(CORNER_OFFSETS[0], 0);

  // Verify corner 7 is at (1,1,1)
  let expected = coord_to_index(1, 0, 0) + coord_to_index(0, 1, 0) + 1;
  assert_eq!(CORNER_OFFSETS[7], expected);
}
