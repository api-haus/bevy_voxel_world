use super::*;

// =========================================================================
// Batch 3: OctreeConfig Tests - Coordinate Math
// =========================================================================

/// At LOD 0 with voxel_size=1.0, cell_size = 1.0 * 28 * 1 = 28
#[test]
fn test_cell_size_at_lod_0() {
  let config = OctreeConfig::default();
  let cell_size = config.get_cell_size(0);
  assert_eq!(cell_size, 28.0, "LOD 0 cell size should be 28");
}

/// Cell size doubles with each LOD level.
#[test]
fn test_cell_size_doubles_per_lod() {
  let config = OctreeConfig::default();

  let size_0 = config.get_cell_size(0);
  let size_1 = config.get_cell_size(1);
  let size_2 = config.get_cell_size(2);
  let size_5 = config.get_cell_size(5);

  assert_eq!(size_1, size_0 * 2.0, "LOD 1 should be 2x LOD 0");
  assert_eq!(size_2, size_0 * 4.0, "LOD 2 should be 4x LOD 0");
  assert_eq!(size_5, size_0 * 32.0, "LOD 5 should be 32x LOD 0");
}

/// Voxel size at LOD = voxel_size * 2^LOD
#[test]
fn test_voxel_size_at_lod() {
  let config = OctreeConfig::default();

  assert_eq!(config.get_voxel_size(0), 1.0, "LOD 0 voxel size");
  assert_eq!(config.get_voxel_size(1), 2.0, "LOD 1 voxel size");
  assert_eq!(config.get_voxel_size(3), 8.0, "LOD 3 voxel size");
}

/// Node (0,0,0) at LOD 0 with origin (0,0,0) has min (0,0,0).
#[test]
fn test_node_min_at_origin() {
  let config = OctreeConfig::default();
  let node = OctreeNode::new(0, 0, 0, 0);

  let min = config.get_node_min(&node);
  assert_eq!(min, DVec3::ZERO, "Origin node should have zero min");
}

/// Node position offsets by world_origin.
#[test]
fn test_node_min_with_world_origin() {
  let mut config = OctreeConfig::default();
  config.world_origin = DVec3::new(100.0, 200.0, 300.0);

  let node = OctreeNode::new(0, 0, 0, 0);
  let min = config.get_node_min(&node);

  assert_eq!(
    min, config.world_origin,
    "Origin node min should equal world_origin"
  );

  // Non-origin node
  let node2 = OctreeNode::new(1, 0, 0, 0);
  let min2 = config.get_node_min(&node2);
  let expected = config.world_origin + DVec3::new(28.0, 0.0, 0.0);
  assert_eq!(min2, expected, "Node (1,0,0) should offset by cell_size");
}

/// Center = min + cell_size * 0.5
#[test]
fn test_node_center() {
  let config = OctreeConfig::default();
  let node = OctreeNode::new(0, 0, 0, 0);

  let center = config.get_node_center(&node);
  let expected = DVec3::splat(14.0); // 28 / 2

  assert_eq!(center, expected, "Center should be half cell_size from min");
}

/// Default lod_exponent=0 means threshold = cell_size * 1.0
#[test]
fn test_lod_threshold_with_exponent() {
  let config = OctreeConfig::default(); // lod_exponent = 0.0

  let cell_size = config.get_cell_size(0);
  let threshold = config.get_threshold(0);

  assert_eq!(
    threshold, cell_size,
    "With exponent 0, threshold equals cell_size"
  );
}

/// Different exponents produce different thresholds.
#[test]
fn test_lod_exponent_affects_threshold() {
  let mut config = OctreeConfig::default();
  let cell_size = 28.0; // LOD 0 cell size

  config.lod_exponent = 1.0; // 2^1 = 2
  assert_eq!(
    config.get_threshold(0),
    cell_size * 2.0,
    "Exponent 1 doubles threshold"
  );

  config.lod_exponent = -1.0; // 2^-1 = 0.5
  assert_eq!(
    config.get_threshold(0),
    cell_size * 0.5,
    "Exponent -1 halves threshold"
  );

  config.lod_exponent = 2.0; // 2^2 = 4
  assert_eq!(
    config.get_threshold(0),
    cell_size * 4.0,
    "Exponent 2 quadruples threshold"
  );
}
