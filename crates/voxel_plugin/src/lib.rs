#![feature(portable_simd)]

//! voxel_plugin - Framework/engine independent voxel world meshing
//!
//! This crate provides high-performance voxel meshing algorithms optimized for
//! 32³ chunk layouts. The primary algorithm is Surface Nets, which generates
//! smooth isosurfaces from signed distance field (SDF) volumes.
//!
//! # Features
//!
//! - **Naive Surface Nets**: Fast isosurface extraction with centroid-based
//!   vertex placement
//! - **LOD Seam Resolution**: Seamless transitions between different
//!   level-of-detail chunks
//! - **Material Blending**: Per-vertex material weights for texture splatting
//! - **FastNoiseLite**: Portable noise generation with SIMD batch processing
//!   using portable_simd (works on native and WASM)
//!
//! # Example
//!
//! ```ignore
//! use voxel_plugin::{surface_nets, MeshConfig, SdfSample, MaterialId};
//!
//! // Create SDF volume (sphere at center)
//! let mut volume = [0i8; 32768];
//! let mut materials = [0u8; 32768];
//!
//! // Fill with sphere SDF...
//!
//! // Generate mesh
//! let config = MeshConfig::default();
//! let output = surface_nets::generate(&volume, &materials, &config);
//!
//! println!("Generated {} vertices, {} triangles",
//!     output.vertices.len(), output.triangle_count());
//! ```

pub mod constants;
pub mod edge_table;
pub mod types;

// Re-export commonly used items
pub use constants::{
  coord_to_index, index_to_coord, CORNER_OFFSETS, SAMPLE_SIZE, SAMPLE_SIZE_CB, SAMPLE_SIZE_SQ,
};
pub use edge_table::{EDGE_CORNERS, EDGE_TABLE};
pub use types::{
  sdf_conversion, MaterialId, MeshConfig, MeshOutput, MinMaxAABB, NormalMode, SdfSample, Vertex,
};

// Surface Nets module
pub mod surface_nets;

// Task queue for parallel meshing
pub mod task_queue;
pub use task_queue::{MeshCompletion, MeshRequest, MeshingStage};

// Octree module for LOD-based spatial subdivision
pub mod octree;
pub use octree::OctreeNode;

// Pipeline task graph for parallel voxel processing
pub mod pipeline;

// World isolation - multi-world support
pub mod world;
pub use world::{VoxelWorld, WorldId};

// Cross-platform threading abstraction
pub mod threading;
pub use threading::{TaskExecutor, TaskId};

// Noise generation with FastNoise2 (native + WASM)
pub mod noise;
pub use noise::FastNoise2Terrain;
