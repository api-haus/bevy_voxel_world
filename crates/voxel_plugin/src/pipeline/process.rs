//! Pipeline Orchestrator
//!
//! Runs the full presample → meshing → composition → presentation pipeline
//! using rayon for parallelism. This is the main entry point for game engine
//! integration.
//!
//! # Usage
//!
//! ```ignore
//! // After running refinement:
//! let output = refine(input);
//!
//! // Process transitions into ready chunks:
//! let ready_chunks = process_transitions(
//!     world_id,
//!     &output.transition_groups,
//!     &sampler,
//!     &leaves,
//!     &config,
//! );
//!
//! // Game engine: spawn/despawn entities based on ready_chunks
//! ```

use std::collections::HashSet;

use rayon::prelude::*;

use super::composition::compose;
use super::presample::sample_volume_for_node;
use super::presentation::present;
use super::types::{ReadyChunk, VolumeSampler, WorkSource};
use crate::noise::is_homogeneous;
use crate::octree::{OctreeConfig, OctreeNode, TransitionGroup, TransitionType};
use crate::types::MeshConfig;
use crate::world::WorldId;

/// Compute neighbor mask for seam handling.
///
/// Detects which faces have coarser LOD neighbors (LOD diff > 0).
fn compute_neighbor_mask(
  node: &OctreeNode,
  leaves: &HashSet<OctreeNode>,
  config: &OctreeConfig,
) -> u8 {
  const FACE_OFFSETS: [(i32, i32, i32); 6] = [
    (-1, 0, 0), // -X
    (1, 0, 0),  // +X
    (0, -1, 0), // -Y
    (0, 1, 0),  // +Y
    (0, 0, -1), // -Z
    (0, 0, 1),  // +Z
  ];

  let mut mask = 0u8;

  for (face_idx, (dx, dy, dz)) in FACE_OFFSETS.iter().enumerate() {
    let neighbor_pos = (node.x + dx, node.y + dy, node.z + dz);

    // Check for coarser neighbor (parent level)
    for lod in (node.lod + 1)..=config.max_lod {
      let scale = 1 << (lod - node.lod);
      let coarser_pos = (
        neighbor_pos.0.div_euclid(scale),
        neighbor_pos.1.div_euclid(scale),
        neighbor_pos.2.div_euclid(scale),
      );
      let coarser = OctreeNode::new(coarser_pos.0, coarser_pos.1, coarser_pos.2, lod);

      if leaves.contains(&coarser) {
        // Found coarser neighbor - set bit
        mask |= 1 << face_idx;
        break;
      }
    }
  }

  mask
}

// Note: is_homogeneous and sample_volume_for_node are imported from their
// canonical locations (noise module and presample module respectively)
// to avoid code duplication.

/// Process transition groups through the full pipeline.
///
/// This is a synchronous function that uses rayon internally for parallelism.
/// It runs: presample → meshing → composition → presentation.
///
/// # Arguments
///
/// * `world_id` - The world these chunks belong to
/// * `transition_groups` - Groups from refinement output
/// * `sampler` - Volume sampler for noise/terrain
/// * `leaves` - Current leaf set (for neighbor mask computation)
/// * `config` - Octree configuration
///
/// # Returns
///
/// Ready chunks with presentation hints, ready for engine integration.
pub fn process_transitions<S: VolumeSampler>(
  world_id: WorldId,
  transition_groups: &[TransitionGroup],
  sampler: &S,
  leaves: &HashSet<OctreeNode>,
  config: &OctreeConfig,
) -> Vec<ReadyChunk> {
  if transition_groups.is_empty() {
    return Vec::new();
  }

  // Collect all nodes that need meshing
  let nodes_to_mesh: Vec<OctreeNode> = transition_groups
    .iter()
    .flat_map(|group| match group.transition_type {
      TransitionType::Subdivide => group.nodes_to_add.iter().copied().collect::<Vec<_>>(),
      TransitionType::Merge => vec![group.group_key],
    })
    .collect();

  if nodes_to_mesh.is_empty() {
    return Vec::new();
  }

  // Stage 2 & 3: Parallel presample + meshing
  let mesh_results: Vec<_> = nodes_to_mesh
    .into_par_iter()
    .filter_map(|node| {
      // Presample using centralized helper
      let sampled = sample_volume_for_node(&node, sampler, config);

      // Skip homogeneous volumes (all solid or all air)
      if is_homogeneous(&sampled.volume) {
        return None;
      }

      // Compute neighbor mask for seam handling
      let neighbor_mask = compute_neighbor_mask(&node, leaves, config);

      // Create mesh config
      let voxel_size = config.get_voxel_size(node.lod);
      let mesh_config = MeshConfig::default()
        .with_voxel_size(voxel_size as f32)
        .with_neighbor_mask(neighbor_mask as u32);

      // Generate mesh
      let output = crate::surface_nets::generate(&sampled.volume, &sampled.materials, &mesh_config);

      if output.is_empty() {
        return None;
      }

      Some(super::types::MeshResult {
        node,
        output,
        timing_us: 0, // Skip timing for batch processing
        work_source: WorkSource::Refinement,
      })
    })
    .collect();

  // Stage 4: Composition
  let composition_output = compose(mesh_results, transition_groups);

  // Stage 5: Presentation
  present(world_id, composition_output)
}

/// Process transitions with timing information.
///
/// Same as `process_transitions` but returns timing stats.
pub fn process_transitions_timed<S: VolumeSampler>(
  world_id: WorldId,
  transition_groups: &[TransitionGroup],
  sampler: &S,
  leaves: &HashSet<OctreeNode>,
  config: &OctreeConfig,
) -> (Vec<ReadyChunk>, ProcessingStats) {
  use web_time::Instant;

  let start = Instant::now();
  let chunks = process_transitions(world_id, transition_groups, sampler, leaves, config);
  let total_us = start.elapsed().as_micros() as u64;

  let stats = ProcessingStats {
    chunk_count: chunks.len(),
    total_us,
  };

  (chunks, stats)
}

/// Statistics from pipeline processing.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessingStats {
  /// Number of chunks produced.
  pub chunk_count: usize,
  /// Total processing time in microseconds.
  pub total_us: u64,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::constants::SAMPLE_SIZE_CB;
  use crate::octree::OctreeNode;

  struct TestSampler;

  impl VolumeSampler for TestSampler {
    fn sample_volume(
      &self,
      _grid_offset: [i64; 3],
      _voxel_size: f64,
      volume: &mut [i8; SAMPLE_SIZE_CB],
      materials: &mut [u8; SAMPLE_SIZE_CB],
    ) {
      // Create a surface at z=16
      for x in 0..32 {
        for y in 0..32 {
          for z in 0..32 {
            let idx = x * 32 * 32 + y * 32 + z;
            // Positive = air, negative = solid
            volume[idx] = if z < 16 { -1 } else { 1 };
            materials[idx] = 1;
          }
        }
      }
    }
  }

  #[test]
  fn test_process_empty_transitions() {
    let world_id = WorldId::new();
    let config = OctreeConfig::default();
    let sampler = TestSampler;
    let leaves = HashSet::new();

    let result = process_transitions(world_id, &[], &sampler, &leaves, &config);
    assert!(result.is_empty());
  }

  #[test]
  fn test_process_subdivide_transition() {
    let world_id = WorldId::new();
    let config = OctreeConfig::default();
    let sampler = TestSampler;

    // Create a parent and its children
    let parent = OctreeNode::new(0, 0, 0, 2);
    let children: Vec<_> = (0..8)
      .filter_map(|octant| parent.get_child(octant))
      .collect();

    // Leaves are the children
    let leaves: HashSet<_> = children.iter().copied().collect();

    // Create subdivide transition
    let transition = TransitionGroup::new_subdivide(parent).unwrap();

    let result = process_transitions(world_id, &[transition], &sampler, &leaves, &config);

    // Should produce chunks for non-empty children
    assert!(!result.is_empty());

    // All should have FadeIn hint
    for chunk in &result {
      match &chunk.hint {
        super::super::types::PresentationHint::FadeIn { group_key } => {
          assert_eq!(*group_key, parent);
        }
        _ => panic!("Expected FadeIn hint"),
      }
    }
  }
}
