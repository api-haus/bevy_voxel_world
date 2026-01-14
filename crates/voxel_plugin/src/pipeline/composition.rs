//! Stage 4: Composition
//!
//! Groups mesh results by their TransitionGroup for coordinated presentation.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │ Composition Stage                                                       │
//! │                                                                         │
//! │ Input:                                                                  │
//! │   - mesh_results: Vec<MeshResult>                                       │
//! │   - transition_groups: &[TransitionGroup]                               │
//! │                                                                         │
//! │ Processing:                                                             │
//! │   1. Separate INVALIDATION (bypass) from REFINEMENT (group)             │
//! │   2. For each TransitionGroup, collect matching meshes                  │
//! │   3. Create GroupedMesh with appropriate transition_type                │
//! │                                                                         │
//! │ Subdivide (1→8):              Merge (8→1):                              │
//! │ ┌───────────────────┐         ┌───────────────────┐                     │
//! │ │ Group contains:   │         │ Group contains:   │                     │
//! │ │  - 8 child meshes │         │  - 1 parent mesh  │                     │
//! │ │  - transition     │         │  - transition     │                     │
//! │ │    type=Subdivide │         │    type=Merge     │                     │
//! │ └───────────────────┘         └───────────────────┘                     │
//! │                                                                         │
//! │ INVALIDATION work_source bypasses grouping → goes to ungrouped          │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;

use smallvec::SmallVec;

use super::types::{GroupedMesh, MeshResult, NodeMesh, WorkSource};
use crate::octree::{OctreeNode, TransitionGroup, TransitionType};

/// Output from composition stage.
pub struct CompositionOutput {
  /// Grouped meshes (from refinement work).
  pub grouped: Vec<GroupedMesh>,
  /// Ungrouped meshes (from invalidation work, bypass).
  pub ungrouped: Vec<MeshResult>,
}

/// Compose mesh results into groups based on transition groups.
///
/// # Algorithm
///
/// 1. Separate mesh results by work_source:
///    - INVALIDATION → bypass to `ungrouped`
///    - REFINEMENT → process for grouping
///
/// 2. For each TransitionGroup:
///    - Subdivide: collect meshes for all 8 children
///    - Merge: collect mesh for parent
///
/// 3. Create GroupedMesh for each TransitionGroup with matching meshes
pub fn compose(
  mesh_results: Vec<MeshResult>,
  transition_groups: &[TransitionGroup],
) -> CompositionOutput {
  // Separate by work source
  let mut ungrouped = Vec::new();
  let mut refinement_meshes: HashMap<OctreeNode, MeshResult> = HashMap::new();

  for result in mesh_results {
    match result.work_source {
      WorkSource::Invalidation => {
        ungrouped.push(result);
      }
      WorkSource::Refinement => {
        refinement_meshes.insert(result.node, result);
      }
    }
  }

  // Group by TransitionGroup
  let mut grouped = Vec::with_capacity(transition_groups.len());

  for group in transition_groups {
    let mut meshes: SmallVec<[NodeMesh; 9]> = SmallVec::new();

    match group.transition_type {
      TransitionType::Subdivide => {
        // Collect meshes for all children (nodes_to_add)
        for child_node in &group.nodes_to_add {
          if let Some(result) = refinement_meshes.remove(child_node) {
            meshes.push(NodeMesh {
              node: result.node,
              output: result.output,
            });
          }
        }
      }
      TransitionType::Merge => {
        // Collect mesh for parent (group_key)
        if let Some(result) = refinement_meshes.remove(&group.group_key) {
          meshes.push(NodeMesh {
            node: result.node,
            output: result.output,
          });
        }
      }
    }

    // Only create group if we have at least one mesh
    if !meshes.is_empty() {
      grouped.push(GroupedMesh {
        group_key: group.group_key,
        meshes,
        transition_type: group.transition_type,
      });
    }
  }

  CompositionOutput { grouped, ungrouped }
}

#[cfg(test)]
#[path = "composition_test.rs"]
mod composition_test;
