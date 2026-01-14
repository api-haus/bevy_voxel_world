use crate::constants::INTERIOR_CELLS;

// =========================================================================
// Batch 1: Volume Constants Tests
// =========================================================================

/// INTERIOR_CELLS (aliased as VOXELS_PER_CELL in octree context) must be 28.
///
/// This is the fundamental constant for cell sizing:
/// Cell Size = VOXELS_PER_CELL * voxel_size * 2^LOD
#[test]
fn test_voxels_per_cell_is_28() {
  assert_eq!(
    INTERIOR_CELLS, 28,
    "INTERIOR_CELLS must be 28 for octree cell sizing"
  );
}

/// Verify all edge sample indices are valid (within 0..32 range).
///
/// The sample grid is 32x32x32 with indices 0..31. Interior cells span
/// indices 1..28, with apron at 0 and displacement padding at 29-31.
#[test]
fn test_index_bounds() {
  use crate::constants::{
    coord_to_index, index_to_coord, FIRST_INTERIOR_CELL, LAST_INTERIOR_CELL, LAST_INTERIOR_SAMPLE,
    MAX_SAMPLE_INDEX, NEGATIVE_APRON, SAMPLE_SIZE,
  };

  // Verify constant relationships
  assert_eq!(NEGATIVE_APRON, 1, "Negative apron should be 1");
  assert_eq!(FIRST_INTERIOR_CELL, 1, "First interior cell should be 1");
  assert_eq!(LAST_INTERIOR_CELL, 28, "Last interior cell should be 28");
  assert_eq!(
    LAST_INTERIOR_SAMPLE, 29,
    "Last interior sample should be 29"
  );
  assert_eq!(MAX_SAMPLE_INDEX, 31, "Max sample index should be 31");

  // Verify all interior cell indices are valid
  for i in FIRST_INTERIOR_CELL..=LAST_INTERIOR_CELL {
    assert!(
      i < SAMPLE_SIZE,
      "Interior cell {} should be within sample bounds",
      i
    );
  }

  // Verify corner indices work correctly
  let corners = [
    (0, 0, 0),                                              // negative apron corner
    (MAX_SAMPLE_INDEX, MAX_SAMPLE_INDEX, MAX_SAMPLE_INDEX), // max corner
    (
      FIRST_INTERIOR_CELL,
      FIRST_INTERIOR_CELL,
      FIRST_INTERIOR_CELL,
    ), // first interior
    (LAST_INTERIOR_CELL, LAST_INTERIOR_CELL, LAST_INTERIOR_CELL), // last interior
  ];

  for (x, y, z) in corners {
    let idx = coord_to_index(x, y, z);
    let (rx, ry, rz) = index_to_coord(idx);
    assert_eq!(
      (x, y, z),
      (rx, ry, rz),
      "Roundtrip failed for corner ({}, {}, {})",
      x,
      y,
      z
    );
  }
}
