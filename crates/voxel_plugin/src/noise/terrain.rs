//! FastNoise2-based 3D volume sampler implementing VolumeSampler.

use super::{presets, NoiseNode};
use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::pipeline::VolumeSampler;
use crate::types::{sdf_conversion, MaterialId, SdfSample};

/// Volume sampler using a single FastNoise2 encoded node tree.
///
/// Samples a 3D noise graph as SDF values for volumetric shapes.
/// The noise output is scaled to properly utilize the i8 quantization range.
///
/// SDF formula: `sdf = noise * scale`
///
/// Where `sdf < 0` is solid and `sdf > 0` is air.
///
/// **Important:** FastNoise2 typically outputs [-1, 1]. To avoid quantization
/// stepping artifacts, set `scale` to utilize more of the ±10.0 storage range.
/// Default scale of 8.0 maps noise [-1, 1] to SDF [-8, 8], using ~200 of 254
/// quantization levels.
#[derive(Clone)]
pub struct FastNoise2Terrain {
  encoded: &'static str,
  /// Multiplier for noise output (default: 8.0)
  /// Maps noise range to SDF range. Higher = more quantization levels used.
  /// With noise in [-1,1]: scale=8.0 → SDF in [-8,8] → ~200 quantization levels
  pub scale: f32,
  /// Frequency multiplier for noise sampling (default: 0.1)
  /// Smaller = larger terrain features
  pub frequency: f32,
  pub seed: i32,
}

impl FastNoise2Terrain {
	/// Create a new volume sampler with default preset.
	pub fn new(seed: i32) -> Self {
		Self {
			encoded: presets::SIMPLE_TERRAIN,
			scale: 8.0,  // Use most of ±10.0 quantization range
			frequency: 0.1,
			seed,
		}
	}

	/// Create a volume sampler with a custom encoded noise graph.
	///
	/// Encoded strings can be exported from FastNoise2's NoiseTool application.
	pub fn with_encoded(encoded: &'static str, seed: i32) -> Self {
		Self {
			encoded,
			scale: 8.0,
			frequency: 0.1,
			seed,
		}
	}

  /// Set scale for noise-to-SDF conversion.
  ///
  /// FastNoise2 outputs [-1, 1]. Scale maps this to the SDF storage range (±10.0).
  /// - scale=8.0 (default): Uses ~200 of 254 quantization levels
  /// - scale=1.0: Only ~25 levels, causes visible stepping
  pub fn with_scale(mut self, scale: f32) -> Self {
    self.scale = scale;
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
  #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, name = "noise::sample_volume"))]
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

    // Convert noise to SDF with scale
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

      // Scale noise to world units, then quantize with voxel-size awareness
      // Noise typically [-1, 1], scale converts to world units
      let sdf = noise[fn_idx] * self.scale;
      volume[vol_idx] = sdf_conversion::to_storage(sdf, voxel_size as f32);
      materials[vol_idx] = 0;
    }
  }
}
