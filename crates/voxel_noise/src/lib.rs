//! FastNoise2 native wrapper for noise generation.
//!
//! This crate provides a Rust wrapper around FastNoise2 C++ library via FFI.
//! For WASM builds, use `voxel_plugin::noise::NoiseNode` which provides a
//! unified API with wasm-bindgen support.
//!
//! # Usage
//! ```ignore
//! use voxel_noise::{NoiseNode, presets};
//!
//! let node = NoiseNode::from_encoded(presets::SIMPLE_TERRAIN).unwrap();
//! let mut output = vec![0.0f32; 32 * 32 * 32];
//! node.gen_uniform_grid_3d(&mut output, 0.0, 0.0, 0.0, 32, 32, 32, 0.02, 0.02, 0.02, 1337);
//! ```
//!
//! # WASM Emscripten Module Build
//! The Emscripten module (for JS bridge) is built separately:
//! ```bash
//! make build  # Uses Emscripten toolchain
//! ```
//! This produces `dist/voxel_noise.js` + `dist/voxel_noise.wasm`.

mod native;
pub use native::NoiseNode;

/// Encoded node tree presets (from FastNoise2 NoiseTool)
pub mod presets {
  /// Simple terrain - FBm with domain warp (working preset)
  pub const SIMPLE_TERRAIN: &str =
    "E@BBZEE@BD8JFgIECArXIzwECiQIw/UoPwkuAAE@BJDQAE@BC@AIEAJBwQDZmYmPwsAAIA/HAMAAHBCBA==";
}

#[cfg(test)]
mod tests {
  use super::{presets, NoiseNode};

  #[test]
  fn test_simple_terrain() {
    let node =
      NoiseNode::from_encoded(presets::SIMPLE_TERRAIN).expect("Failed to create noise node");
    let mut output = vec![0.0f32; 32 * 32 * 32];
    node.gen_uniform_grid_3d(&mut output, 0.0, 0.0, 0.0, 32, 32, 32, 0.02, 0.02, 0.02, 1337);
    assert!(output.iter().any(|&v| v != 0.0), "All values are zero");
  }

  #[test]
  fn test_2d_grid() {
    let node =
      NoiseNode::from_encoded(presets::SIMPLE_TERRAIN).expect("Failed to create noise node");
    let mut output = vec![0.0f32; 32 * 32];
    node.gen_uniform_grid_2d(&mut output, 0.0, 0.0, 32, 32, 0.02, 0.02, 1337);
    assert!(output.iter().any(|&v| v != 0.0), "All values are zero");
  }
}
