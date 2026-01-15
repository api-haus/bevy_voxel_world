//! Unified noise generation for native and WASM platforms.
//!
//! This module provides a platform-agnostic `NoiseNode` API:
//! - Native: Uses `voxel_noise::NoiseNode` (fastnoise2 FFI)
//! - WASM: Uses wasm-bindgen to call JS bridge to Emscripten module
//!
//! The `FastNoise2Terrain` sampler uses `NoiseNode` and works identically
//! on both platforms.

// Platform-specific NoiseNode implementations
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

// Re-export unified NoiseNode
#[cfg(not(target_arch = "wasm32"))]
pub use native::NoiseNode;
#[cfg(target_arch = "wasm32")]
pub use wasm::NoiseNode;

// Terrain sampler (platform-agnostic, uses NoiseNode)
mod terrain;
pub use terrain::FastNoise2Terrain;

// Re-export presets
#[cfg(not(target_arch = "wasm32"))]
pub use voxel_noise::presets;

// For WASM, define presets locally (voxel_noise isn't a dep for wasm32)
#[cfg(target_arch = "wasm32")]
pub mod presets {
  /// Simple terrain - FBm with domain warp (working preset from NoiseTool)
  pub const SIMPLE_TERRAIN: &str =
    "E@BBZEE@BD8JFgIECArXIzwECiQIw/UoPwkuAAE@BJDQAE@BC@AIEAJBwQDZmYmPwsAAIA/HAMAAHBCBA==";
}

use crate::constants::SAMPLE_SIZE_CB;
use crate::types::SdfSample;

/// Check if a volume is entirely air or solid (can skip meshing).
pub fn is_homogeneous(volume: &[SdfSample; SAMPLE_SIZE_CB]) -> bool {
  if volume.is_empty() {
    return true;
  }
  let first_sign = volume[0] < 0;
  volume.iter().all(|&v| (v < 0) == first_sign)
}
