//! FastNoise2-based 3D volume sampler implementing VolumeSampler.

use super::{presets, NoiseNode};
use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::pipeline::VolumeSampler;
use crate::types::{sdf_conversion, MaterialId, SdfSample};

/// Volume sampler using a single FastNoise2 encoded node tree.
///
/// Samples a 3D noise graph directly as SDF values. The noise output
/// is scaled and offset to produce the final SDF. Works identically
/// on native and WASM through the unified NoiseNode API.
#[derive(Clone)]
pub struct FastNoise2Terrain {
  encoded: &'static str,
  /// Multiplier for noise output (scales SDF range)
  pub scale: f32,
  /// Offset added to scaled noise (shifts the surface)
  pub offset: f32,
  /// Frequency multiplier for noise sampling (default: 1.0)
  /// Smaller = larger terrain features
  pub frequency: f32,
  pub seed: i32,
}

impl FastNoise2Terrain {
  /// Create a new volume sampler with default preset.
  pub fn new(seed: i32) -> Self {
    Self {
      encoded: presets::SIMPLE_TERRAIN,
      scale: 1.0,
      offset: 0.0,
      frequency: 1.0,
      seed,
    }
  }

  /// Create a volume sampler with a custom encoded noise graph.
  ///
  /// Encoded strings can be exported from FastNoise2's NoiseTool application.
  pub fn with_encoded(encoded: &'static str, seed: i32) -> Self {
    Self {
      encoded,
      scale: 1.0,
      offset: 0.0,
      frequency: 1.0,
      seed,
    }
  }

  /// Set scale and offset for noise-to-SDF conversion.
  ///
  /// `sdf = noise * scale + offset`
  pub fn with_scale_offset(mut self, scale: f32, offset: f32) -> Self {
    self.scale = scale;
    self.offset = offset;
    self
  }

  /// Set frequency multiplier for noise sampling.
  ///
  /// Smaller values = larger terrain features.
  pub fn with_frequency(mut self, frequency: f32) -> Self {
    self.frequency = frequency;
    self
  }
}

impl VolumeSampler for FastNoise2Terrain {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    const SIZE: usize = SAMPLE_SIZE;

    // Convert grid_offset to world position, then scale by frequency
    // frequency controls terrain feature size: smaller = larger features
    let world_x = (grid_offset[0] as f64 * voxel_size) as f32 * self.frequency;
    let world_y = (grid_offset[1] as f64 * voxel_size) as f32 * self.frequency;
    let world_z = (grid_offset[2] as f64 * voxel_size) as f32 * self.frequency;
    // Step must scale with voxel_size for chunk boundary coherency
    let step = voxel_size as f32 * self.frequency;

    // Create noise node from encoded preset
    let node = NoiseNode::from_encoded(self.encoded).expect("Invalid encoded node tree");

    // Generate 3D noise directly using fork's float offset API
    let mut noise = vec![0.0f32; SAMPLE_SIZE_CB];
    node.gen_uniform_grid_3d(
      &mut noise,
      world_x,
      world_y,
      world_z,
      SIZE as i32,
      SIZE as i32,
      SIZE as i32,
      step,
      step,
      step,
      self.seed,
    );

    // Convert noise to SDF with scale and offset
    // CRITICAL: Remap axis ordering from FastNoise2 to volume layout
    // FastNoise2 outputs X-fastest: fn_idx = z * SIZE² + y * SIZE + x
    // Volume uses X-slowest: vol_idx = x * SIZE² + y * SIZE + z
    for vol_idx in 0..SAMPLE_SIZE_CB {
      let x = vol_idx / (SIZE * SIZE);
      let yz = vol_idx % (SIZE * SIZE);
      let y = yz / SIZE;
      let z = yz % SIZE;

      // FastNoise2 index: X-fastest layout
      let fn_idx = z * SIZE * SIZE + y * SIZE + x;

      let sdf = noise[fn_idx] * self.scale + self.offset;
      volume[vol_idx] = sdf_conversion::to_storage(sdf);
      materials[vol_idx] = 0;
    }
  }
}
