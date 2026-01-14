//! Naive Surface Nets meshing algorithm.
//!
//! This module implements a high-performance Surface Nets algorithm for
//! converting signed distance field (SDF) volumes into triangulated polygon
//! meshes.
//!
//! # Algorithm Overview
//!
//! Surface Nets is a dual contouring method that generates ONE vertex per cell
//! containing a surface crossing, placing it at the centroid of all edge
//! crossings. This produces smoother output with fewer vertices than Marching
//! Cubes.
//!
//! ```text
//! Traditional Marching Cubes:
//!   - Vertices placed ON edges where surface crosses
//!   - Creates many vertices per cell
//!   - Sharp features, aliasing artifacts
//!
//! Surface Nets:
//!   - ONE vertex per cell containing surface
//!   - Vertex placed at centroid of edge crossings
//!   - Smoother output, fewer vertices
//! ```
//!
//! # Processing Pipeline
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        INPUT                                    │
//! │  volume: [i8; 32768]     - SDF values (-128 to +127)            │
//! │  materials: [u8; 32768]  - Material IDs (0-3)                   │
//! │  edge_table: [u16; 256]  - Precomputed edge lookup              │
//! │  neighbor_mask: u32      - LOD transition flags                 │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    PHASE 1: Corner Classification               │
//! │  For each 2×2×2 cell:                                           │
//! │    Load 8 SDF samples at cube corners                           │
//! │    Build 8-bit corner mask from sign bits                       │
//! │    Early-out if homogeneous (mask == 0 or mask == 255)          │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    PHASE 2: Cell Processing                     │
//! │    Lookup edge mask from table[corner_mask]                     │
//! │    Compute vertex position: centroid of edge crossings          │
//! │    Compute normal from SDF gradient (or defer)                  │
//! │    Compute material weights from solid corners                  │
//! │    Apply LOD displacement if near coarser neighbor              │
//! │    Store vertex, record buffer index                            │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    PHASE 3: Triangulation                       │
//! │  For each active edge (3 edges per cell checked):               │
//! │    Skip if at boundary (prevent duplicate quads)                │
//! │    Lookup 4 adjacent vertex indices from buffer                 │
//! │    Split quad along shorter diagonal                            │
//! │    Emit 2 triangles (6 indices) with correct winding            │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    PHASE 4: Normal Calculation                  │
//! │  Option A: Gradient normals (computed in Phase 2)               │
//! │  Option B: Geometry normals (post-process from triangles)       │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        OUTPUT                                   │
//! │  vertices: Vec<Vertex>   - Position, normal, material weights   │
//! │  indices: Vec<u32>       - Triangle indices                     │
//! │  bounds: AABB            - Mesh bounding box                    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Coordinate System
//!
//! ```text
//!         +Y
//!          │
//!          │
//!          │
//!          └───────── +X
//!         /
//!        /
//!       +Z
//!
//! Cell corner indices (binary: ZYX):
//!   0 = (0,0,0)    4 = (0,0,1)
//!   1 = (1,0,0)    5 = (1,0,1)
//!   2 = (0,1,0)    6 = (0,1,1)
//!   3 = (1,1,0)    7 = (1,1,1)
//! ```
//!
//! # Edge Indexing
//!
//! Each edge connects two corners of the 2×2×2 cube:
//!
//! ```text
//! Edge  Axis  Corners    Description
//! ────  ────  ─────────  ─────────────────
//!   0    X    [0, 1]     X-edge at origin
//!   1    Y    [0, 2]     Y-edge at origin
//!   2    Z    [0, 4]     Z-edge at origin
//!   3    Y    [1, 3]     Y-edge at X+
//!   4    Z    [1, 5]     Z-edge at X+
//!   5    X    [2, 3]     X-edge at Y+
//!   6    Z    [2, 6]     Z-edge at Y+
//!   7    Z    [3, 7]     Z-edge at X+Y+
//!   8    X    [4, 5]     X-edge at Z+
//!   9    Y    [4, 6]     Y-edge at Z+
//!  10    Y    [5, 7]     Y-edge at X+Z+
//!  11    X    [6, 7]     X-edge at Y+Z+
//! ```
//!
//! # Pipeline Steps (Summary)
//!
//! 1. **Cell Iteration**: Process 31×31×31 cells (from 32³ samples)
//! 2. **Corner Classification**: Build 8-bit mask from SDF signs at cube
//!    corners
//! 3. **Early Exit**: Skip homogeneous cells (all solid or all air)
//! 4. **Edge Lookup**: Map corner mask to edge mask via precomputed table
//! 5. **Vertex Placement**: Compute centroid of edge crossing points
//! 6. **Normal Calculation**: Derive normal from SDF gradient
//! 7. **Material Weights**: Blend materials from solid corners
//! 8. **Triangulation**: Emit triangles for active edges connecting to previous
//!    cells

mod corner_mask;
mod gradient;
mod lod_seams;
mod material_weights;
mod vertex_calc;

pub use lod_seams::NeighborMask;

use crate::constants::*;
use crate::edge_table::*;
use crate::types::sdf_conversion;
use crate::types::*;

// =============================================================================
// Pass-based meshing pipeline
// =============================================================================
//
// The meshing pipeline is structured as sequential passes:
//
// Pass 1: Geometry
//   - Cell iteration and classification
//   - Vertex position computation
//   - LOD displacement
//   - Material weight computation
//   - Triangle emission
//   - Normals set to placeholder
//
// Pass 2: Normals
//   - Compute normals based on configured mode
//   - Gradient2x2: Fast, cell-local
//   - Gradient3x3: Smoother, uses neighbor samples
//   - Geometry: From triangle face normals
//   - Blended: Geometry interior, gradient at boundaries
//
// Future passes could include:
// - Pass 3: Materials (advanced blending, texture coordinates)
// - Pass 4: LOD stitching refinement
//

/// Index buffer for tracking vertex indices during triangulation.
/// Uses a checkerboard ping-pong pattern for memory efficiency.
struct IndexBuffer {
  data: Vec<i32>,
  size: usize,
}

impl IndexBuffer {
  fn new() -> Self {
    // Buffer needs (SAMPLE_SIZE + 1)² × 2 for ping-pong pattern
    let size = (SAMPLE_SIZE + 1) * (SAMPLE_SIZE + 1) * 2;
    Self {
      data: vec![-1; size],
      size,
    }
  }

  #[inline]
  fn calculate_index(&self, x: usize, y: usize, z: usize) -> usize {
    let base = z + (SAMPLE_SIZE + 1) * y;

    if x % 2 == 0 {
      base + 1 + (SAMPLE_SIZE + 1) * (SAMPLE_SIZE + 2)
    } else {
      base + SAMPLE_SIZE + 2
    }
  }

  #[inline]
  fn get(&self, x: usize, y: usize, z: usize) -> i32 {
    let idx = self.calculate_index(x, y, z);
    if idx < self.size {
      self.data[idx]
    } else {
      -1
    }
  }

  #[inline]
  fn set(&mut self, x: usize, y: usize, z: usize, value: i32) {
    let idx = self.calculate_index(x, y, z);
    if idx < self.size {
      self.data[idx] = value;
    }
  }

  #[allow(dead_code)]
  fn clear(&mut self) {
    self.data.fill(-1);
  }
}

/// Generate mesh from SDF volume using Naive Surface Nets algorithm.
///
/// # Arguments
///
/// * `volume` - 32³ signed distance field samples (negative = solid)
/// * `materials` - 32³ material IDs (0-3)
/// * `config` - Mesh generation configuration
///
/// # Returns
///
/// Mesh output containing vertices, indices, and bounds.
pub fn generate(
  volume: &[SdfSample; SAMPLE_SIZE_CB],
  materials: &[MaterialId; SAMPLE_SIZE_CB],
  config: &MeshConfig,
) -> MeshOutput {
  let mut output = MeshOutput::new();
  let mut index_buffer = IndexBuffer::new();

  // Extract transition bits once (skip ALL_SAME_LOD flag at bit 0)
  let transition_bits = config.neighbor_mask & lod_seams::ALL_TRANSITION_BITS;

  // =========================================================================
  // Pass 1: Geometry
  // =========================================================================
  // Process all cells, emit vertices and triangles.
  // Normals are set to placeholder [0, 1, 0].
  for x in 0..(SAMPLE_SIZE - 1) {
    for y in 0..(SAMPLE_SIZE - 1) {
      for z in 0..(SAMPLE_SIZE - 1) {
        process_cell_geometry(
          volume,
          materials,
          [x, y, z],
          &mut index_buffer,
          &mut output,
          config,
          transition_bits,
        );
      }
    }
  }

  // =========================================================================
  // Pass 2: Normals
  // =========================================================================
  compute_normals(volume, &mut output, config);

  output
}

/// Compute normals for all vertices based on the configured mode.
fn compute_normals(
  volume: &[SdfSample; SAMPLE_SIZE_CB],
  output: &mut MeshOutput,
  config: &MeshConfig,
) {
  match config.normal_mode {
    NormalMode::Gradient => {
      // Compute gradient normals from cell corner samples
      compute_gradient_normals(volume, output);
    }
    NormalMode::Geometry => {
      // Compute normals from triangle geometry
      gradient::recalculate_from_geometry(output);
    }
    NormalMode::Blended { blend_distance } => {
      // First compute geometry normals
      gradient::recalculate_from_geometry(output);

      // Then blend with gradient at boundaries
      blend_boundary_normals(volume, output, blend_distance);
    }
  }
}

/// Compute gradient normals for all vertices.
fn compute_gradient_normals(volume: &[SdfSample; SAMPLE_SIZE_CB], output: &mut MeshOutput) {
  for vertex in &mut output.vertices {
    let [x, y, z] = vertex.cell_position;
    let base_idx = coord_to_index(x as usize, y as usize, z as usize);

    // Load 8 corner samples
    let samples: [f32; 8] =
      std::array::from_fn(|i| sdf_conversion::to_float(volume[base_idx + CORNER_OFFSETS[i]]));

    vertex.normal = gradient::compute(&samples);
  }
}

/// Blend geometry normals with gradient normals at chunk boundaries.
fn blend_boundary_normals(
  volume: &[SdfSample; SAMPLE_SIZE_CB],
  output: &mut MeshOutput,
  blend_distance: f32,
) {
  use glam::Vec3A;

  for vertex in &mut output.vertices {
    let cell_pos = vertex.cell_position;

    // Skip interior vertices (blend_factor = 1.0 means pure geometry)
    if !lod_seams::needs_boundary_blend(cell_pos, blend_distance) {
      continue;
    }

    // Compute blend factor: 0 = boundary (gradient), 1 = interior (geometry)
    let blend = lod_seams::compute_boundary_blend_factor(cell_pos, blend_distance);

    // Compute gradient normal
    let base_idx = coord_to_index(
      cell_pos[0] as usize,
      cell_pos[1] as usize,
      cell_pos[2] as usize,
    );
    let samples: [f32; 8] =
      std::array::from_fn(|i| sdf_conversion::to_float(volume[base_idx + CORNER_OFFSETS[i]]));
    let gradient_normal = gradient::compute(&samples);

    // Blend: lerp from gradient (at boundary) to geometry (interior)
    let geom = Vec3A::from_array(vertex.normal);
    let grad = Vec3A::from_array(gradient_normal);
    let blended = geom * blend + grad * (1.0 - blend);

    // Normalize the blended result
    let len_sq = blended.length_squared();
    if len_sq > 1e-8 {
      let normalized = blended * len_sq.sqrt().recip();
      vertex.normal = normalized.to_array();
    }
  }
}

/// Process a single 2×2×2 cell of the volume (geometry pass).
///
/// Creates vertices with placeholder normals. Actual normals are computed
/// in the normal pass.
fn process_cell_geometry(
  volume: &[SdfSample; SAMPLE_SIZE_CB],
  materials: &[MaterialId; SAMPLE_SIZE_CB],
  pos: [usize; 3],
  index_buffer: &mut IndexBuffer,
  output: &mut MeshOutput,
  _config: &MeshConfig,
  transition_bits: u32,
) {
  use vertex_calc::Vec3A;

  let [x, y, z] = pos;

  // Sample 8 corners of the cube
  let base_idx = coord_to_index(x, y, z);

  // Load raw i8 samples for corner mask
  let raw_samples: [i8; 8] = std::array::from_fn(|i| volume[base_idx + CORNER_OFFSETS[i]]);

  // Build corner mask for material weights and triangulation winding
  let corner_mask = corner_mask::build(raw_samples);

  // Early exit for homogeneous cells (all solid or all air)
  if corner_mask == 0 || corner_mask == 255 {
    return;
  }

  // Convert to f32 for vertex calculations
  let samples: [f32; 8] = std::array::from_fn(|i| sdf_conversion::to_float(raw_samples[i]));

  // Compute vertex position using direct edge iteration (returns Vec3A)
  let cell_origin = Vec3A::new(x as f32, y as f32, z as f32);
  let position = cell_origin + vertex_calc::compute_position_direct(&samples);

  // Compute material weights
  let material_weights = material_weights::compute(materials, corner_mask, base_idx);

  // Check for boundary vertex and compute displaced position
  let cell_pos = [x as i32, y as i32, z as i32];
  let position_arr = position.to_array();
  let displaced_pos =
    if transition_bits != 0 && lod_seams::is_boundary_vertex(cell_pos, transition_bits) {
      lod_seams::compute_displaced_position(volume, cell_pos, position_arr)
    } else {
      position_arr
    };

  // Store vertex with placeholder normal (computed in normal pass)
  let vertex_index = output.vertices.len() as i32;
  index_buffer.set(x, y, z, vertex_index);

  output.vertices.push(Vertex {
    position: displaced_pos,
    normal: [0.0, 1.0, 0.0], // Placeholder
    material_weights,
    cell_position: cell_pos,
  });
  output.displaced_positions.push(displaced_pos);
  output.bounds.encapsulate(displaced_pos);

  // Look up edge mask for triangulation (still needed for determining which quads
  // to emit)
  let edge_mask = EDGE_TABLE[corner_mask as usize];

  // Emit triangles for active edges
  emit_triangles(pos, edge_mask, corner_mask, index_buffer, output);
}

/// Emit triangles for active edges of a cell.
///
/// Uses shorter diagonal optimization: splits quads along the shorter diagonal
/// to produce better quality triangles with less degenerate cases.
fn emit_triangles(
  pos: [usize; 3],
  edge_mask: u16,
  corner_mask: u8,
  index_buffer: &IndexBuffer,
  output: &mut MeshOutput,
) {
  let [x, y, z] = pos;

  // Determine winding order based on corner 0
  // Flip if corner 0 is outside (positive SDF)
  let flip = (corner_mask & 1) == 0;

  // Check edges 0, 1, 2 (X, Y, Z directions)
  for axis in 0..3 {
    if (edge_mask & (1 << axis)) == 0 {
      continue;
    }

    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;

    // Skip boundary positions to prevent duplicate quads
    let pos_arr = [x, y, z];
    if pos_arr[u] == 0 || pos_arr[v] == 0 {
      continue;
    }

    // Get 4 vertex indices forming the quad
    let v_a = index_buffer.get(x, y, z);

    // Calculate offset positions
    let mut pos_b = [x, y, z];
    pos_b[u] = pos_b[u].wrapping_sub(1);
    pos_b[v] = pos_b[v].wrapping_sub(1);

    let mut pos_c = [x, y, z];
    pos_c[u] = pos_c[u].wrapping_sub(1);

    let mut pos_d = [x, y, z];
    pos_d[v] = pos_d[v].wrapping_sub(1);

    let v_b = index_buffer.get(pos_b[0], pos_b[1], pos_b[2]);
    let v_c = index_buffer.get(pos_c[0], pos_c[1], pos_c[2]);
    let v_d = index_buffer.get(pos_d[0], pos_d[1], pos_d[2]);

    // Skip if any vertex is invalid
    if v_a < 0 || v_b < 0 || v_c < 0 || v_d < 0 {
      continue;
    }

    // Get vertex positions for shorter diagonal calculation
    let p_a = output.displaced_positions[v_a as usize];
    let p_b = output.displaced_positions[v_b as usize];
    let p_c = output.displaced_positions[v_c as usize];
    let p_d = output.displaced_positions[v_d as usize];

    // Calculate diagonal lengths squared
    // Diagonal 1: A-B (opposite corners)
    // Diagonal 2: C-D (other opposite corners)
    let diag_ab = dist_sq(p_a, p_b);
    let diag_cd = dist_sq(p_c, p_d);

    // Split along shorter diagonal for better triangle quality
    if diag_ab < diag_cd {
      // Split along A-B diagonal
      if flip {
        output.indices.extend_from_slice(&[
          v_a as u32, v_d as u32, v_b as u32, v_a as u32, v_b as u32, v_c as u32,
        ]);
      } else {
        output.indices.extend_from_slice(&[
          v_a as u32, v_b as u32, v_d as u32, v_a as u32, v_c as u32, v_b as u32,
        ]);
      }
    } else {
      // Split along C-D diagonal
      if flip {
        output.indices.extend_from_slice(&[
          v_c as u32, v_d as u32, v_b as u32, v_c as u32, v_a as u32, v_d as u32,
        ]);
      } else {
        output.indices.extend_from_slice(&[
          v_c as u32, v_b as u32, v_d as u32, v_c as u32, v_d as u32, v_a as u32,
        ]);
      }
    }
  }
}

/// Squared distance between two points.
#[inline(always)]
fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
  let dx = a[0] - b[0];
  let dy = a[1] - b[1];
  let dz = a[2] - b[2];
  dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
