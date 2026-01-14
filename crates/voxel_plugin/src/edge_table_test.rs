use super::*;

#[test]
fn test_edge_table_homogeneous() {
  // All corners same sign = no crossings
  assert_eq!(EDGE_TABLE[0], 0, "All air should have no edges");
  assert_eq!(EDGE_TABLE[255], 0, "All solid should have no edges");
}

#[test]
fn test_edge_table_single_corner() {
  // Single solid corner should activate exactly 3 edges
  for corner in 0..8 {
    let mask = 1u8 << corner;
    let edge_count = EDGE_TABLE[mask as usize].count_ones();
    assert_eq!(
      edge_count, 3,
      "Corner {} should have 3 edges, got {}",
      corner, edge_count
    );
  }
}

#[test]
fn test_edge_table_symmetry() {
  // Complementary corner masks should have same edge mask
  for i in 0..128 {
    assert_eq!(
      EDGE_TABLE[i],
      EDGE_TABLE[255 - i],
      "Edge masks should be symmetric for {} and {}",
      i,
      255 - i
    );
  }
}

#[test]
fn test_edge_corners_validity() {
  // All corner indices should be 0-7
  for edge in &EDGE_CORNERS {
    assert!(edge[0] < 8);
    assert!(edge[1] < 8);
    assert_ne!(edge[0], edge[1]);
  }
}

#[test]
fn test_corner_position() {
  assert_eq!(corner_position(0), [0.0, 0.0, 0.0]);
  assert_eq!(corner_position(1), [1.0, 0.0, 0.0]);
  assert_eq!(corner_position(2), [0.0, 1.0, 0.0]);
  assert_eq!(corner_position(4), [0.0, 0.0, 1.0]);
  assert_eq!(corner_position(7), [1.0, 1.0, 1.0]);
}
