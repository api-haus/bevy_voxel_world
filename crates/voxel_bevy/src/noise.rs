//! FastNoise2-based noise generation for terrain sampling.
//!
//! Native builds use direct FFI to FastNoise2 via voxel_noise crate.
//! WASM builds use JS bridge to Emscripten-compiled FastNoise2 module,
//! with per-worker initialization (each web worker initializes its own module).

use voxel_plugin::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use voxel_plugin::pipeline::VolumeSampler;
use voxel_plugin::types::{sdf_conversion, MaterialId, SdfSample};

/// Encoded node tree presets for terrain generation.
pub mod noise_presets {
  /// Simple terrain - FBm with domain warp (working preset from NoiseTool)
  pub const SIMPLE_TERRAIN: &str =
    "E@BBZEE@BD8JFgIECArXIzwECiQIw/UoPwkuAAE@BJDQAE@BC@AIEAJBwQDZmYmPwsAAIA/HAMAAHBCBA==";
}

// =============================================================================
// Native implementation (direct FFI via voxel_noise crate)
// =============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native {
  /// Re-export presets from voxel_noise for native builds.
  pub use voxel_noise::presets as external_presets;
  use voxel_noise::{presets, NoiseNode};

  use super::*;

  /// Terrain sampler using FastNoise2 encoded node trees (native).
  #[derive(Clone)]
  pub struct FastNoise2Terrain {
    terrain_encoded: &'static str,
    cave_encoded: &'static str,
    pub terrain_amplitude: f32,
    pub terrain_base_height: f32,
    pub cave_threshold: f32,
    pub seed: i32,
  }

  impl FastNoise2Terrain {
    pub fn new(seed: i32) -> Self {
      Self {
        terrain_encoded: presets::SIMPLE_TERRAIN,
        cave_encoded: presets::SIMPLE_TERRAIN,
        terrain_amplitude: 50.0,
        terrain_base_height: 0.0,
        cave_threshold: 0.3,
        seed,
      }
    }

    pub fn with_encoded(
      terrain_encoded: &'static str,
      cave_encoded: &'static str,
      seed: i32,
    ) -> Self {
      Self {
        terrain_encoded,
        cave_encoded,
        terrain_amplitude: 50.0,
        terrain_base_height: 0.0,
        cave_threshold: 0.3,
        seed,
      }
    }

    pub fn with_terrain(mut self, amplitude: f32, base_height: f32) -> Self {
      self.terrain_amplitude = amplitude;
      self.terrain_base_height = base_height;
      self
    }

    pub fn with_cave_threshold(mut self, threshold: f32) -> Self {
      self.cave_threshold = threshold;
      self
    }
  }

  impl VolumeSampler for FastNoise2Terrain {
    fn sample_volume(
      &self,
      sample_start: [f64; 3],
      voxel_size: f64,
      volume: &mut [SdfSample; SAMPLE_SIZE_CB],
      materials: &mut [MaterialId; SAMPLE_SIZE_CB],
    ) {
      const SIZE: usize = SAMPLE_SIZE;
      let vs = voxel_size as f32;
      let start_x = sample_start[0] as f32;
      let start_y = sample_start[1] as f32;
      let start_z = sample_start[2] as f32;
      let step = vs;

      let terrain_node =
        NoiseNode::from_encoded(self.terrain_encoded).expect("Invalid terrain encoded node tree");
      let cave_node =
        NoiseNode::from_encoded(self.cave_encoded).expect("Invalid cave encoded node tree");

      let mut heightmap = vec![0.0f32; SIZE * SIZE];
      terrain_node.gen_uniform_grid_2d(
        &mut heightmap,
        start_x,
        start_z,
        SIZE as i32,
        SIZE as i32,
        step,
        step,
        self.seed,
      );

      let mut cave_noise = vec![0.0f32; SAMPLE_SIZE_CB];
      cave_node.gen_uniform_grid_3d(
        &mut cave_noise,
        start_x,
        start_y,
        start_z,
        SIZE as i32,
        SIZE as i32,
        SIZE as i32,
        step,
        step,
        step,
        self.seed + 1000,
      );

      for idx in 0..SAMPLE_SIZE_CB {
        let x = idx / (SIZE * SIZE);
        let yz = idx % (SIZE * SIZE);
        let y = yz / SIZE;
        let z = yz % SIZE;
        let h_idx = z * SIZE + x;
        let world_y = start_y + y as f32 * vs;
        let height = self.terrain_base_height + heightmap[h_idx] * self.terrain_amplitude;
        let terrain_sdf = world_y - height;
        let cave_sdf = cave_noise[idx] - self.cave_threshold;
        let final_sdf = terrain_sdf.max(cave_sdf);
        volume[idx] = sdf_conversion::to_storage(final_sdf);
        materials[idx] = 0;
      }
    }
  }
}

// =============================================================================
// WASM implementation (JS bridge to Emscripten-compiled FastNoise2)
// =============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm {
  use wasm_bindgen::prelude::*;

  use super::*;

  // JS bridge functions (from voxel_noise_bridge.js)
  // Each worker initializes its own Emscripten module via top-level await.
  #[wasm_bindgen(module = "/js/voxel_noise_bridge.js")]
  extern "C" {
    /// Wait for module initialization (call before using noise functions).
    #[wasm_bindgen(js_name = vx_init)]
    pub async fn vx_init() -> JsValue;

    /// Create a noise node from encoded string. Returns handle.
    #[wasm_bindgen(js_name = vx_create)]
    pub fn vx_create(encoded: &str) -> u32;

    /// Generate 3D noise grid. Returns Float32Array.
    #[wasm_bindgen(js_name = vx_gen_3d)]
    pub fn vx_gen_3d(
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
    ) -> js_sys::Float32Array;

    /// Generate 2D noise grid. Returns Float32Array.
    #[wasm_bindgen(js_name = vx_gen_2d)]
    pub fn vx_gen_2d(
      handle: u32,
      x_off: f32,
      y_off: f32,
      x_cnt: i32,
      y_cnt: i32,
      x_step: f32,
      y_step: f32,
      seed: i32,
    ) -> js_sys::Float32Array;

    /// Destroy a noise node handle.
    #[wasm_bindgen(js_name = vx_destroy)]
    pub fn vx_destroy(handle: u32);
  }

  /// Terrain sampler using FastNoise2 via JS bridge (WASM).
  #[derive(Clone)]
  pub struct FastNoise2Terrain {
    terrain_encoded: &'static str,
    cave_encoded: &'static str,
    pub terrain_amplitude: f32,
    pub terrain_base_height: f32,
    pub cave_threshold: f32,
    pub seed: i32,
  }

  impl FastNoise2Terrain {
    pub fn new(seed: i32) -> Self {
      Self {
        terrain_encoded: super::noise_presets::SIMPLE_TERRAIN,
        cave_encoded: super::noise_presets::SIMPLE_TERRAIN,
        terrain_amplitude: 50.0,
        terrain_base_height: 0.0,
        cave_threshold: 0.3,
        seed,
      }
    }

    pub fn with_encoded(
      terrain_encoded: &'static str,
      cave_encoded: &'static str,
      seed: i32,
    ) -> Self {
      Self {
        terrain_encoded,
        cave_encoded,
        terrain_amplitude: 50.0,
        terrain_base_height: 0.0,
        cave_threshold: 0.3,
        seed,
      }
    }

    pub fn with_terrain(mut self, amplitude: f32, base_height: f32) -> Self {
      self.terrain_amplitude = amplitude;
      self.terrain_base_height = base_height;
      self
    }

    pub fn with_cave_threshold(mut self, threshold: f32) -> Self {
      self.cave_threshold = threshold;
      self
    }
  }

  impl VolumeSampler for FastNoise2Terrain {
    fn sample_volume(
      &self,
      sample_start: [f64; 3],
      voxel_size: f64,
      volume: &mut [SdfSample; SAMPLE_SIZE_CB],
      materials: &mut [MaterialId; SAMPLE_SIZE_CB],
    ) {
      const SIZE: usize = SAMPLE_SIZE;
      let vs = voxel_size as f32;
      let start_x = sample_start[0] as f32;
      let start_y = sample_start[1] as f32;
      let start_z = sample_start[2] as f32;
      let step = vs;

      // Create noise nodes via JS bridge
      let terrain_handle = vx_create(self.terrain_encoded);
      let cave_handle = vx_create(self.cave_encoded);

      // Generate 2D heightmap
      let heightmap_js = vx_gen_2d(
        terrain_handle,
        start_x,
        start_z,
        SIZE as i32,
        SIZE as i32,
        step,
        step,
        self.seed,
      );
      let mut heightmap = vec![0.0f32; SIZE * SIZE];
      heightmap_js.copy_to(&mut heightmap);

      // Generate 3D cave noise
      let cave_js = vx_gen_3d(
        cave_handle,
        start_x,
        start_y,
        start_z,
        SIZE as i32,
        SIZE as i32,
        SIZE as i32,
        step,
        step,
        step,
        self.seed + 1000,
      );
      let mut cave_noise = vec![0.0f32; SAMPLE_SIZE_CB];
      cave_js.copy_to(&mut cave_noise);

      // Clean up handles
      vx_destroy(terrain_handle);
      vx_destroy(cave_handle);

      // Combine terrain and caves
      for idx in 0..SAMPLE_SIZE_CB {
        let x = idx / (SIZE * SIZE);
        let yz = idx % (SIZE * SIZE);
        let y = yz / SIZE;
        let z = yz % SIZE;
        let h_idx = z * SIZE + x;
        let world_y = start_y + y as f32 * vs;
        let height = self.terrain_base_height + heightmap[h_idx] * self.terrain_amplitude;
        let terrain_sdf = world_y - height;
        let cave_sdf = cave_noise[idx] - self.cave_threshold;
        let final_sdf = terrain_sdf.max(cave_sdf);
        volume[idx] = sdf_conversion::to_storage(final_sdf);
        materials[idx] = 0;
      }
    }
  }
}

// =============================================================================
// Re-exports
// =============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub use native::FastNoise2Terrain;
#[cfg(target_arch = "wasm32")]
pub use wasm::FastNoise2Terrain;

/// Check if a volume is entirely air or solid (can skip meshing).
pub fn is_homogeneous(volume: &[SdfSample; SAMPLE_SIZE_CB]) -> bool {
  if volume.is_empty() {
    return true;
  }
  let first_sign = volume[0] < 0;
  volume.iter().all(|&v| (v < 0) == first_sign)
}
