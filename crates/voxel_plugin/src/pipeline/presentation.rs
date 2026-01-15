//! Stage 5: Presentation
//!
//! Assigns presentation hints to ready chunks.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │ Presentation Stage                                                      │
//! │                                                                         │
//! │ PresentationHint determines how the renderer should handle the chunk:   │
//! │                                                                         │
//! │   - Immediate       → Invalidation: swap mesh instantly                 │
//! │   - FadeIn { key }  → Subdivide: fade in new children                   │
//! │   - FadeOut { key } → Merge: fade out children, keep parent             │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```

use super::composition::CompositionOutput;
use super::types::{GroupedMesh, MeshResult, PresentationHint, ReadyChunk};
use crate::octree::TransitionType;
use crate::world::WorldId;

/// Present grouped meshes as ready chunks.
///
/// - Subdivide → FadeIn hint for each child
/// - Merge → FadeOut hint for parent
pub fn present_grouped(world_id: WorldId, grouped: Vec<GroupedMesh>) -> Vec<ReadyChunk> {
  let mut chunks = Vec::new();

  for group in grouped {
    let hint = match group.transition_type {
      TransitionType::Subdivide => PresentationHint::FadeIn {
        group_key: group.group_key,
      },
      TransitionType::Merge => PresentationHint::FadeOut {
        group_key: group.group_key,
      },
    };

    for node_mesh in group.meshes {
      chunks.push(ReadyChunk {
        world_id,
        node: node_mesh.node,
        output: node_mesh.output,
        hint: hint.clone(),
      });
    }
  }

  chunks
}

/// Present ungrouped meshes (invalidation) as ready chunks.
///
/// All ungrouped meshes get Immediate hint.
pub fn present_ungrouped(world_id: WorldId, ungrouped: Vec<MeshResult>) -> Vec<ReadyChunk> {
  ungrouped
    .into_iter()
    .map(|result| ReadyChunk {
      world_id,
      node: result.node,
      output: result.output,
      hint: PresentationHint::Immediate,
    })
    .collect()
}

/// Present all composition output as ready chunks.
///
/// Combines grouped (with FadeIn/FadeOut hints) and ungrouped (with Immediate
/// hints).
pub fn present(world_id: WorldId, output: CompositionOutput) -> Vec<ReadyChunk> {
  let mut chunks = present_grouped(world_id, output.grouped);
  chunks.extend(present_ungrouped(world_id, output.ungrouped));
  chunks
}

#[cfg(test)]
#[path = "presentation_test.rs"]
mod presentation_test;
