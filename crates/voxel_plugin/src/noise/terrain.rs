//! FastNoise2-based terrain generation implementing VolumeSampler.

use super::{presets, NoiseNode};
use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::pipeline::VolumeSampler;
use crate::types::{sdf_conversion, MaterialId, SdfSample};

/// Terrain sampler using FastNoise2 encoded node trees.
///
/// Uses 2D heightmap noise combined with 3D cave noise to generate
/// terrain SDF values. Works identically on native and WASM through
/// the unified NoiseNode API.
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
  /// Create a new terrain sampler with default presets.
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

  /// Create a terrain sampler with custom encoded noise graphs.
  ///
  /// Encoded strings can be exported from FastNoise2's NoiseTool application.
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

  /// Set terrain height parameters.
  pub fn with_terrain(mut self, amplitude: f32, base_height: f32) -> Self {
    self.terrain_amplitude = amplitude;
    self.terrain_base_height = base_height;
    self
  }

  /// Set cave carving threshold.
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

    // Create noise nodes (uses native FFI or WASM JS bridge automatically)
    let terrain_node =
      NoiseNode::from_encoded(self.terrain_encoded).expect("Invalid terrain encoded node tree");
    let cave_node =
      NoiseNode::from_encoded(self.cave_encoded).expect("Invalid cave encoded node tree");

    // Generate 2D heightmap
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

    // Generate 3D cave noise
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

    // Combine terrain heightmap and cave noise into SDF
    for idx in 0..SAMPLE_SIZE_CB {
      let x = idx / (SIZE * SIZE);
      let yz = idx % (SIZE * SIZE);
      let y = yz / SIZE;
      let z = yz % SIZE;

      // 2D heightmap lookup (x, z coordinates)
      let h_idx = z * SIZE + x;
      let world_y = start_y + y as f32 * vs;
      let height = self.terrain_base_height + heightmap[h_idx] * self.terrain_amplitude;

      // Terrain SDF: positive above surface, negative below
      let terrain_sdf = world_y - height;

      // Cave SDF: carve out where noise exceeds threshold
      let cave_sdf = cave_noise[idx] - self.cave_threshold;

      // Union via max: air where either terrain or cave is air
      let final_sdf = terrain_sdf.max(cave_sdf);

      volume[idx] = sdf_conversion::to_storage(final_sdf);
      materials[idx] = 0;
    }
  }
}
