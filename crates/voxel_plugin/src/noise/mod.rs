//! Unified noise generation for native and WASM platforms.
//!
//! This module provides a platform-agnostic `NoiseNode` API:
//! - Native: Uses `voxel_noise::NoiseNode` (fastnoise2 FFI)
//! - WASM: Uses wasm-bindgen to call JS bridge to Emscripten module
//!
//! The `FastNoise2Terrain` sampler uses a single 3D noise graph directly
//! as SDF values. Works identically on native and WASM.

// Platform-specific NoiseNode implementations
#[cfg(target_arch = "wasm32")]
mod wasm;

// Re-export unified NoiseNode
#[cfg(not(target_arch = "wasm32"))]
pub use voxel_noise::NoiseNode;
#[cfg(target_arch = "wasm32")]
pub use wasm::NoiseNode;

// Terrain sampler (platform-agnostic, uses NoiseNode)
mod terrain;
#[cfg(test)]
mod terrain_test;
pub use terrain::FastNoise2Terrain;


// Re-export presets
#[cfg(not(target_arch = "wasm32"))]
pub use voxel_noise::presets;

// For WASM, define presets locally (voxel_noise isn't a dep for wasm32)
#[cfg(target_arch = "wasm32")]
pub mod presets {
  /// Simple terrain noise - FBm with domain warp (from NoiseTool built-in "Simple Terrain")
  pub const SIMPLE_TERRAIN: &str = "E@BBZEE@BD8JFgIECArXIzwECiQIw/UoPwkuAAE@BJDQAE@BC@AIEAJBwQDZmYmPwsAAIA/HAMAAHBCBA==";
}

use crate::constants::{coord_to_index, SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::types::SdfSample;

/// Check if volume has any surface crossings (sign changes along edges).
///
/// This matches what Surface Nets uses to generate geometry.
/// A crossing exists when adjacent samples have different signs.
///
/// Returns true if meshing is needed, false if chunk can be skipped.
pub fn has_surface_crossing(volume: &[SdfSample; SAMPLE_SIZE_CB]) -> bool {
  // Check x-axis edges (x varies, y and z fixed)
  for y in 0..SAMPLE_SIZE {
    for z in 0..SAMPLE_SIZE {
      for x in 0..(SAMPLE_SIZE - 1) {
        let i0 = coord_to_index(x, y, z);
        let i1 = coord_to_index(x + 1, y, z);
        if (volume[i0] < 0) != (volume[i1] < 0) {
          return true;
        }
      }
    }
  }

  // Check y-axis edges (y varies, x and z fixed)
  for x in 0..SAMPLE_SIZE {
    for z in 0..SAMPLE_SIZE {
      for y in 0..(SAMPLE_SIZE - 1) {
        let i0 = coord_to_index(x, y, z);
        let i1 = coord_to_index(x, y + 1, z);
        if (volume[i0] < 0) != (volume[i1] < 0) {
          return true;
        }
      }
    }
  }

  // Check z-axis edges (z varies, x and y fixed)
  for x in 0..SAMPLE_SIZE {
    for y in 0..SAMPLE_SIZE {
      for z in 0..(SAMPLE_SIZE - 1) {
        let i0 = coord_to_index(x, y, z);
        let i1 = coord_to_index(x, y, z + 1);
        if (volume[i0] < 0) != (volume[i1] < 0) {
          return true;
        }
      }
    }
  }

  false // No surface crossings found
}

/// Check if a volume is entirely air or solid (can skip meshing).
#[deprecated(note = "Use has_surface_crossing() instead - it correctly detects adjacent sign changes")]
pub fn is_homogeneous(volume: &[SdfSample; SAMPLE_SIZE_CB]) -> bool {
  !has_surface_crossing(volume)
}
