//! WASM NoiseNode implementation using JS bridge to Emscripten module.
//!
//! Uses wasm-bindgen to call the pre-compiled FastNoise2 Emscripten module
//! via the JS bridge in voxel_noise/js/.

use js_sys::Float32Array;
use wasm_bindgen::prelude::*;

// JS bridge functions (from voxel_noise_bridge.js)
// Path navigates from voxel_plugin crate root up to crates/, then into voxel_noise/
#[wasm_bindgen(module = "/../voxel_noise/js/voxel_noise_bridge.js")]
extern "C" {
  /// Create a noise node from encoded string. Returns handle (0 on error).
  #[wasm_bindgen(js_name = vx_create)]
  fn vx_create(encoded: &str) -> u32;

  /// Generate 3D noise grid. Returns Float32Array.
  #[wasm_bindgen(js_name = vx_gen_3d)]
  fn vx_gen_3d(
    handle: u32,
    x_off: f32,
    y_off: f32,
    z_off: f32,
    x_cnt: i32,
    y_cnt: i32,
    z_cnt: i32,
    x_step: f32,
    y_step: f32,
    z_step: f32,
    seed: i32,
  ) -> Float32Array;

  /// Generate 2D noise grid. Returns Float32Array.
  #[wasm_bindgen(js_name = vx_gen_2d)]
  fn vx_gen_2d(
    handle: u32,
    x_off: f32,
    y_off: f32,
    x_cnt: i32,
    y_cnt: i32,
    x_step: f32,
    y_step: f32,
    seed: i32,
  ) -> Float32Array;

  /// Destroy a noise node handle.
  #[wasm_bindgen(js_name = vx_destroy)]
  fn vx_destroy(handle: u32);
}

/// A noise generator node created from an encoded node tree string (WASM).
///
/// Uses JS bridge to call pre-compiled FastNoise2 Emscripten module.
/// Implements Drop for automatic cleanup.
pub struct NoiseNode {
  handle: u32,
}

impl NoiseNode {
  /// Create a noise node from an encoded node tree string.
  ///
  /// Returns `None` if the encoded string is invalid.
  pub fn from_encoded(encoded: &str) -> Option<Self> {
    let handle = vx_create(encoded);
    if handle == 0 {
      None
    } else {
      Some(Self { handle })
    }
  }

  /// Generate noise values on a uniform 3D grid.
  ///
  /// # Arguments
  /// * `output` - Buffer to write noise values into (must be x_cnt * y_cnt * z_cnt in size)
  /// * `x_off, y_off, z_off` - Grid origin offset
  /// * `x_cnt, y_cnt, z_cnt` - Grid dimensions (number of samples per axis)
  /// * `x_step, y_step, z_step` - Step size between samples
  /// * `seed` - Random seed for noise generation
  pub fn gen_uniform_grid_3d(
    &self,
    output: &mut [f32],
    x_off: f32,
    y_off: f32,
    z_off: f32,
    x_cnt: i32,
    y_cnt: i32,
    z_cnt: i32,
    x_step: f32,
    y_step: f32,
    z_step: f32,
    seed: i32,
  ) {
    let result = vx_gen_3d(
      self.handle, x_off, y_off, z_off, x_cnt, y_cnt, z_cnt, x_step, y_step, z_step, seed,
    );
    result.copy_to(output);
  }

  /// Generate noise values on a uniform 2D grid.
  ///
  /// # Arguments
  /// * `output` - Buffer to write noise values into (must be x_cnt * y_cnt in size)
  /// * `x_off, y_off` - Grid origin offset
  /// * `x_cnt, y_cnt` - Grid dimensions
  /// * `x_step, y_step` - Step size between samples
  /// * `seed` - Random seed
  pub fn gen_uniform_grid_2d(
    &self,
    output: &mut [f32],
    x_off: f32,
    y_off: f32,
    x_cnt: i32,
    y_cnt: i32,
    x_step: f32,
    y_step: f32,
    seed: i32,
  ) {
    let result = vx_gen_2d(self.handle, x_off, y_off, x_cnt, y_cnt, x_step, y_step, seed);
    result.copy_to(output);
  }
}

impl Drop for NoiseNode {
  fn drop(&mut self) {
    vx_destroy(self.handle);
  }
}

// WASM NoiseNode is NOT Send + Sync because JS bridge calls are context-bound.
// For worker parallelism, each worker creates its own NoiseNode instances.
