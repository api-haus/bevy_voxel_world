//! Stage 5: Presentation
//!
//! Converts mesh data to byte arrays and assigns presentation hints.
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
//! │                                                                         │
//! │ MeshData format (for FFI to Unity/Godot):                               │
//! │   - vertices: Vec<u8>   // Vertex struct as raw bytes                   │
//! │   - indices: Vec<u8>    // u32 indices as raw bytes                     │
//! │   - vertex_count: u32                                                   │
//! │   - index_count: u32                                                    │
//! │   - bounds: MinMaxAABB                                                  │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```

use super::composition::CompositionOutput;
use super::types::{GroupedMesh, MeshData, MeshResult, PresentationHint, ReadyChunk};
use crate::octree::TransitionType;
use crate::types::{MeshOutput, Vertex};
use crate::world::WorldId;

/// Convert a MeshOutput to byte-level MeshData for FFI.
fn mesh_output_to_data(output: &MeshOutput) -> MeshData {
  let vertex_count = output.vertices.len() as u32;
  let index_count = output.indices.len() as u32;

  // Convert vertices to raw bytes
  let vertices = if output.vertices.is_empty() {
    Vec::new()
  } else {
    let vertex_bytes = std::mem::size_of::<Vertex>() * output.vertices.len();
    let mut bytes = Vec::with_capacity(vertex_bytes);
    unsafe {
      let ptr = output.vertices.as_ptr() as *const u8;
      bytes.extend_from_slice(std::slice::from_raw_parts(ptr, vertex_bytes));
    }
    bytes
  };

  // Convert indices to raw bytes
  let indices = if output.indices.is_empty() {
    Vec::new()
  } else {
    let index_bytes = std::mem::size_of::<u32>() * output.indices.len();
    let mut bytes = Vec::with_capacity(index_bytes);
    unsafe {
      let ptr = output.indices.as_ptr() as *const u8;
      bytes.extend_from_slice(std::slice::from_raw_parts(ptr, index_bytes));
    }
    bytes
  };

  MeshData {
    vertices,
    indices,
    vertex_count,
    index_count,
    bounds: output.bounds,
  }
}

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
        mesh_data: mesh_output_to_data(&node_mesh.output),
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
      mesh_data: mesh_output_to_data(&result.output),
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
