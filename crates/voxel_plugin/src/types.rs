//! Core data types for Surface Nets meshing.

/// Signed distance field sample value.
/// Negative = inside/solid, Positive = outside/air.
pub type SdfSample = i8;

/// Normal computation mode for mesh generation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NormalMode {
  /// Gradient from cell corners (fast, consistent across chunks).
  Gradient,

  /// Normals from triangle geometry (accurate interior, discontinuous at chunk
  /// edges).
  Geometry,

  /// Blend geometry normals (interior) with gradient normals (boundary).
  /// Provides best visual quality: accurate geometry normals inside chunks,
  /// smooth gradient normals at chunk boundaries for seamless transitions.
  Blended {
    /// Cells from boundary where blending starts (typically 2-4).
    blend_distance: f32,
  },
}

impl Default for NormalMode {
  fn default() -> Self {
    NormalMode::Gradient
  }
}

/// Material identifier (0-3 for 4-material blending).
pub type MaterialId = u8;

/// SDF conversion utilities for quantized storage.
///
/// Maps float SDF to i8 [-127, +127] with ±10 world unit range.
/// Precision: ~0.079 world units per level (254 levels over 20 units).
/// Extended range allows smooth blend fillet depths up to 10.
pub mod sdf_conversion {
  /// SDF range in world units (values outside this clamp to ±127).
  pub const RANGE: f32 = 10.0;

  /// Scale factor: 127 / RANGE = 12.7
  pub const SCALE: f32 = 127.0 / RANGE;

  /// Inverse scale for converting back to float.
  pub const INV_SCALE: f32 = RANGE / 127.0;

  /// Convert float SDF to quantized i8 storage.
  ///
  /// # Arguments
  /// * `sdf` - SDF value in world units
  ///
  /// # Returns
  /// Quantized i8 value scaled to fit ±127 range
  #[inline(always)]
  pub fn to_storage(sdf: f32) -> i8 {
    (sdf * SCALE).clamp(-127.0, 127.0).round() as i8
  }

  /// Convert quantized i8 storage back to float SDF.
  ///
  /// # Arguments
  /// * `value` - Quantized i8 sample
  ///
  /// # Returns
  /// SDF value in world units
  #[inline(always)]
  pub fn to_float(value: i8) -> f32 {
    value as f32 * INV_SCALE
  }
}

/// Output vertex with all mesh attributes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
  /// Vertex position in chunk-local coordinates [0, 31].
  pub position: [f32; 3],

  /// Surface normal (unit vector).
  pub normal: [f32; 3],

  /// Material blend weights (sum to 1.0).
  pub material_weights: [f32; 4],

  /// Original cell position for debugging/LOD.
  pub cell_position: [i32; 3],
}

impl Default for Vertex {
  fn default() -> Self {
    Self {
      position: [0.0; 3],
      normal: [0.0, 1.0, 0.0],
      material_weights: [1.0, 0.0, 0.0, 0.0],
      cell_position: [0; 3],
    }
  }
}

/// Axis-aligned bounding box.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MinMaxAABB {
  pub min: [f32; 3],
  pub max: [f32; 3],
}

impl MinMaxAABB {
  /// Create AABB with inverted extents (ready for encapsulation).
  pub fn empty() -> Self {
    Self {
      min: [f32::INFINITY; 3],
      max: [f32::NEG_INFINITY; 3],
    }
  }

  /// Create AABB from min/max corners.
  pub fn new(min: [f32; 3], max: [f32; 3]) -> Self {
    Self { min, max }
  }

  /// Expand AABB to include a point.
  #[inline]
  pub fn encapsulate(&mut self, point: [f32; 3]) {
    for i in 0..3 {
      self.min[i] = self.min[i].min(point[i]);
      self.max[i] = self.max[i].max(point[i]);
    }
  }

  /// Check if AABB is valid (min <= max on all axes).
  pub fn is_valid(&self) -> bool {
    self.min[0] <= self.max[0] && self.min[1] <= self.max[1] && self.min[2] <= self.max[2]
  }
}

impl Default for MinMaxAABB {
  fn default() -> Self {
    Self::empty()
  }
}

/// Mesh generation result.
#[derive(Default)]
pub struct MeshOutput {
  /// Output vertices with positions, normals, and material weights.
  pub vertices: Vec<Vertex>,

  /// Triangle indices (3 indices per triangle).
  pub indices: Vec<u32>,

  /// Displaced positions for LOD seam vertices (parallel to vertices).
  pub displaced_positions: Vec<[f32; 3]>,

  /// Bounding box encompassing all vertices.
  pub bounds: MinMaxAABB,
}

impl MeshOutput {
  pub fn new() -> Self {
    Self::default()
  }

  /// Clear all buffers, preserving capacity.
  pub fn clear(&mut self) {
    self.vertices.clear();
    self.indices.clear();
    self.displaced_positions.clear();
    self.bounds = MinMaxAABB::empty();
  }

  /// Returns true if no geometry was generated.
  pub fn is_empty(&self) -> bool {
    self.vertices.is_empty()
  }

  /// Number of triangles in the mesh.
  pub fn triangle_count(&self) -> usize {
    self.indices.len() / 3
  }
}

/// Configuration for mesh generation.
#[derive(Clone, Debug)]
pub struct MeshConfig {
  /// Scale factor applied to vertex positions.
  pub voxel_size: f32,

  /// 26-bit mask indicating coarser LOD neighbors.
  /// Bit 0: ALL_SAME_LOD flag
  /// Bits 1-6: Face transitions
  /// Bits 7-14: Corner transitions
  /// Bits 15-26: Edge transitions
  pub neighbor_mask: u32,

  /// Normal computation mode.
  pub normal_mode: NormalMode,

  /// Apply MicroSplat-compatible weight encoding.
  pub use_microsplat_encoding: bool,
}

impl Default for MeshConfig {
  fn default() -> Self {
    Self {
      voxel_size: 1.0,
      neighbor_mask: 0,
      normal_mode: NormalMode::default(),
      use_microsplat_encoding: false,
    }
  }
}

impl MeshConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_voxel_size(mut self, size: f32) -> Self {
    self.voxel_size = size;
    self
  }

  pub fn with_neighbor_mask(mut self, mask: u32) -> Self {
    self.neighbor_mask = mask;
    self
  }

  pub fn with_normal_mode(mut self, mode: NormalMode) -> Self {
    self.normal_mode = mode;
    self
  }

  pub fn with_microsplat_encoding(mut self, use_microsplat: bool) -> Self {
    self.use_microsplat_encoding = use_microsplat;
    self
  }

  /// Legacy compatibility: set gradient normals (true) or geometry normals
  /// (false).
  #[deprecated(note = "Use with_normal_mode instead")]
  pub fn with_gradient_normals(mut self, use_gradient: bool) -> Self {
    self.normal_mode = if use_gradient {
      NormalMode::Gradient
    } else {
      NormalMode::Geometry
    };
    self
  }
}

#[cfg(test)]
#[path = "types_test.rs"]
mod types_test;
