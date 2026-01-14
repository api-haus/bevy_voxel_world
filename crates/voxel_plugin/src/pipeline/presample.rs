//! Stage 2: Presample
//!
//! Samples full 32³ volume and detects homogeneous regions.
//! Homogeneous chunks (all solid or all air) skip meshing entirely.

use rayon::prelude::*;

use super::types::{PresampleOutput, SampledVolume, VolumeSampler, WorkSource};
use crate::constants::SAMPLE_SIZE_CB;
use crate::octree::{OctreeConfig, OctreeNode};
use crate::types::SdfSample;

/// Check if volume is homogeneous (all samples have same sign).
#[inline]
fn is_homogeneous(volume: &[SdfSample; SAMPLE_SIZE_CB]) -> bool {
  let first_sign = volume[0] < 0;
  volume.iter().all(|&v| (v < 0) == first_sign)
}

/// Sample the full 32³ volume for a node using VolumeSampler.
fn sample_volume<S: VolumeSampler>(
  node: &OctreeNode,
  sampler: &S,
  config: &OctreeConfig,
) -> SampledVolume {
  let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
  let mut materials = Box::new([0u8; SAMPLE_SIZE_CB]);

  let node_min = config.get_node_min(node);
  let voxel_size = config.get_voxel_size(node.lod);

  // Call the volume sampler with sample_start and voxel_size
  sampler.sample_volume(
    [node_min.x, node_min.y, node_min.z],
    voxel_size,
    &mut volume,
    &mut materials,
  );

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
  let sampled = sample_volume(&node, sampler, config);

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
