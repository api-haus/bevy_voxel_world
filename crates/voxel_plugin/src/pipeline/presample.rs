//! Stage 2: Presample
//!
//! Samples full 32³ volume and detects homogeneous regions.
//! Homogeneous chunks (all solid or all air) skip meshing entirely.

use rayon::prelude::*;

use super::types::{PresampleOutput, SampledVolume, VolumeSampler, WorkSource};
use crate::constants::SAMPLE_SIZE_CB;
use crate::noise::is_homogeneous;
use crate::octree::{OctreeConfig, OctreeNode};

/// Sample the full 32³ volume for a node using VolumeSampler.
///
/// Uses integer grid coordinates for precision at chunk boundaries.
/// Matches C# FastNoise2Sampler approach:
/// - grid_offset = round(node_min / voxel_size)
/// - Sample N world position = (grid_offset + N) * voxel_size
///
/// This ensures adjacent chunks use identical integer offsets for
/// overlapping samples, eliminating floating-point precision divergence.
pub fn sample_volume_for_node<S: VolumeSampler + ?Sized>(
  node: &OctreeNode,
  sampler: &S,
  config: &OctreeConfig,
) -> SampledVolume {
  let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
  let mut materials = Box::new([0u8; SAMPLE_SIZE_CB]);

  let node_min = config.get_node_min(node);
  let voxel_size = config.get_voxel_size(node.lod);

  // Convert to integer grid coordinates to avoid floating-point precision issues.
  // This matches C# FastNoise2Sampler: gridStart = (int3)round(worldMin / voxelSize)
  let grid_offset = [
    (node_min.x / voxel_size).round() as i64,
    (node_min.y / voxel_size).round() as i64,
    (node_min.z / voxel_size).round() as i64,
  ];

  sampler.sample_volume(grid_offset, voxel_size, &mut volume, &mut materials);

  SampledVolume { volume, materials }
}

/// Presample a single node: sample volume, check homogeneity.
///
/// Returns `Some(volume)` if surface may exist, `None` if homogeneous.
pub fn presample_node<S: VolumeSampler>(
  node: OctreeNode,
  work_source: WorkSource,
  sampler: &S,
  config: &OctreeConfig,
) -> PresampleOutput {
  let sampled = sample_volume_for_node(&node, sampler, config);

  let volume = if is_homogeneous(&sampled.volume) {
    None
  } else {
    Some(sampled)
  };

  PresampleOutput {
    node,
    volume,
    work_source,
  }
}

/// Presample multiple nodes in parallel using rayon.
pub fn presample_batch<S: VolumeSampler>(
  nodes: Vec<(OctreeNode, WorkSource)>,
  sampler: &S,
  config: &OctreeConfig,
) -> Vec<PresampleOutput> {
  if nodes.is_empty() {
    return Vec::new();
  }

  nodes
    .into_par_iter()
    .map(|(node, work_source)| presample_node(node, work_source, sampler, config))
    .collect()
}

#[cfg(test)]
#[path = "presample_test.rs"]
mod presample_test;
