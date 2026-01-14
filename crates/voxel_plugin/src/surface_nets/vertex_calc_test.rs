use super::*;

#[test]
fn test_compute_position_single_solid_corner() {
  // Corner 0 is solid (-1), all others are air (+1)
  // Edges 0, 1, 2 should cross (they connect corner 0 to corners 1, 2, 4)
  let samples = [-1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

  let pos = compute_position_direct(&samples);

  // Should be near corner 0 since it's the only solid corner
  assert!(pos.x < 0.5);
  assert!(pos.y < 0.5);
  assert!(pos.z < 0.5);
}

#[test]
fn test_compute_position_fallback() {
  // All same sign = no edge crossings
  let samples = [1.0; 8];

  let pos = compute_position_direct(&samples);

  assert!((pos.x - 0.5).abs() < 1e-6);
  assert!((pos.y - 0.5).abs() < 1e-6);
  assert!((pos.z - 0.5).abs() < 1e-6);
}

#[test]
fn test_compute_position_half_solid() {
  // Bottom half solid (corners 0,1,2,3), top half air (corners 4,5,6,7)
  let samples = [-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0];

  let pos = compute_position_direct(&samples);

  // Surface should be at z=0.5 (between bottom and top)
  assert!((pos.z - 0.5).abs() < 0.1);
  // X and Y should be centered
  assert!((pos.x - 0.5).abs() < 0.1);
  assert!((pos.y - 0.5).abs() < 0.1);
}

#[test]
fn test_compute_position_interpolation() {
  // Corner 0 very solid (-3), corner 1 barely air (+1)
  // Edge crossing should be at t = 3/4 = 0.75 along X axis
  let samples = [-3.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

  let pos = compute_position_direct(&samples);

  // Three edges cross: 0 (x), 1 (y), 2 (z)
  // Each crossing is at (0.75, 0, 0), (0, 0.75, 0), (0, 0, 0.75)
  // Centroid = (0.25, 0.25, 0.25)
  assert!((pos.x - 0.25).abs() < 0.1);
  assert!((pos.y - 0.25).abs() < 0.1);
  assert!((pos.z - 0.25).abs() < 0.1);
}

#[test]
fn test_corner_positions_match_bit_layout() {
  for i in 0..8 {
    let p = CORNER_POSITIONS[i];
    let expected_x = (i & 1) as f32;
    let expected_y = ((i >> 1) & 1) as f32;
    let expected_z = ((i >> 2) & 1) as f32;
    assert_eq!(p.x, expected_x, "corner {} x mismatch", i);
    assert_eq!(p.y, expected_y, "corner {} y mismatch", i);
    assert_eq!(p.z, expected_z, "corner {} z mismatch", i);
  }
}

#[test]
fn test_cube_edges_valid() {
  for (i, &[c0, c1]) in CUBE_EDGES.iter().enumerate() {
    assert!(c0 < 8, "edge {} corner 0 out of bounds", i);
    assert!(c1 < 8, "edge {} corner 1 out of bounds", i);
    assert_ne!(c0, c1, "edge {} connects corner to itself", i);
  }
}
