//! FastNoise2 abstraction for native and WASM builds
//!
//! This crate provides a unified API for FastNoise2 noise generation:
//! - Native: Direct FFI to FastNoise2 C++ library
//! - WASM: Compiles to Emscripten, exports C functions for JS bridge
//!
//! # Usage (Native)
//! ```ignore
//! use voxel_noise::{NoiseNode, presets};
//!
//! let node = NoiseNode::from_encoded(presets::SIMPLEX_FBM).unwrap();
//! let mut output = vec![0.0f32; 32 * 32 * 32];
//! node.gen_uniform_grid_3d(&mut output, 0.0, 0.0, 0.0, 32, 32, 32, 0.02, 0.02, 0.02, 1337);
//! ```
//!
//! # WASM Build
//! For WASM, build with `wasm32-unknown-emscripten` target:
//! ```bash
//! make build  # Uses Emscripten toolchain
//! ```
//! This produces `voxel_noise.js` + `voxel_noise.wasm` with exported C
//! functions.

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
  use fastnoise2::SafeNode;

  #[test]
  fn test_simple_terrain() {
    let encoded =
      "E@BBZEE@BD8JFgIECArXIzwECiQIw/UoPwkuAAE@BJDQAE@BC@AIEAJBwQDZmYmPwsAAIA/HAMAAHBCBA==";
    let result = SafeNode::from_encoded_node_tree(encoded);
    match result {
      Ok(node) => {
        let mut output = vec![0.0f32; 32 * 32 * 32];
        node.gen_uniform_grid_3d(
          &mut output,
          0.0,
          0.0,
          0.0,
          32,
          32,
          32,
          0.02,
          0.02,
          0.02,
          1337,
        );
        assert!(output.iter().any(|&v| v != 0.0), "All values are zero");
      }
      Err(e) => {
        panic!("Failed to create node: {:?}", e);
      }
    }
  }
}
