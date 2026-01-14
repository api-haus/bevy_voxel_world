use super::*;

#[test]
fn test_interior_vertex_not_boundary() {
  // Cell in the middle should never be boundary
  let pos = [15, 15, 15];
  let mask = ALL_TRANSITION_BITS;

  assert!(!is_boundary_vertex(pos, mask));
}

#[test]
fn test_face_boundary_detection() {
  // Cell near +X face
  let pos = [27, 15, 15];

  assert!(is_boundary_vertex(pos, FACE_POS_X));
  assert!(!is_boundary_vertex(pos, FACE_NEG_X));
}

#[test]
fn test_corner_boundary_detection() {
  // Cell near +X,+Y,+Z corner
  let pos = [27, 27, 27];

  assert!(is_boundary_vertex(pos, VERTEX_PPP));
  assert!(!is_boundary_vertex(pos, VERTEX_NNN));
}

#[test]
fn test_no_mask_no_boundary() {
  let pos = [27, 27, 27];
  assert!(!is_boundary_vertex(pos, 0));
}

// =============================================================================
// Boundary blend factor tests
// =============================================================================

#[test]
fn test_blend_factor_interior() {
  // Cell in center should have blend factor = 1.0 (pure geometry)
  let pos = [15, 15, 15];
  let blend = compute_boundary_blend_factor(pos, 3.0);
  assert_eq!(blend, 1.0);
}

#[test]
fn test_blend_factor_at_boundary() {
  // Cell at boundary (cell 1) should have blend factor = 0.0 (pure gradient)
  let pos = [1, 15, 15];
  let blend = compute_boundary_blend_factor(pos, 3.0);
  assert_eq!(blend, 0.0);
}

#[test]
fn test_blend_factor_transition_zone() {
  // Cell 2 cells from boundary with blend_distance=3 should be ~0.33
  let pos = [2, 15, 15]; // 1 cell from min (cell 1)
  let blend = compute_boundary_blend_factor(pos, 3.0);
  assert!(
    (blend - 0.333).abs() < 0.01,
    "Expected ~0.33, got {}",
    blend
  );

  // Cell 3 cells from boundary with blend_distance=3 should be ~0.67
  let pos2 = [3, 15, 15]; // 2 cells from min
  let blend2 = compute_boundary_blend_factor(pos2, 3.0);
  assert!(
    (blend2 - 0.667).abs() < 0.01,
    "Expected ~0.67, got {}",
    blend2
  );
}

#[test]
fn test_blend_factor_positive_boundary() {
  // Cell 28 (last interior) should have blend factor = 0.0
  let pos = [28, 15, 15];
  let blend = compute_boundary_blend_factor(pos, 3.0);
  assert_eq!(blend, 0.0);
}

#[test]
fn test_blend_factor_corner() {
  // Corner should still be 0.0 (minimum distance to any boundary)
  let pos = [1, 1, 1];
  let blend = compute_boundary_blend_factor(pos, 3.0);
  assert_eq!(blend, 0.0);
}

#[test]
fn test_needs_boundary_blend() {
  // Interior cell doesn't need blending
  assert!(!needs_boundary_blend([15, 15, 15], 3.0));

  // Boundary cell needs blending
  assert!(needs_boundary_blend([1, 15, 15], 3.0));
  assert!(needs_boundary_blend([28, 15, 15], 3.0));

  // Cell just outside blend zone doesn't need blending
  assert!(!needs_boundary_blend([5, 15, 15], 3.0));
}
