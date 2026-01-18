//! Pure-Rust SIMD terrain sampler using the simdnoise crate.
//!
//! Generates terrain by intersecting 3D noise with a height-based density gradient.
//! This creates realistic terrain features: hills, valleys, cliffs, and optional caves.
//!
//! The terrain density function is:
//! ```text
//! density = (y - base_height) * height_gradient + surface_noise + cave_noise
//! ```
//!
//! Where density < 0 is solid and density > 0 is air.

use simdnoise::NoiseBuilder;

use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::pipeline::VolumeSampler;
use crate::types::{sdf_conversion, MaterialId, SdfSample};

/// Noise type for terrain surface modulation.
#[derive(Clone, Copy, Debug, Default)]
pub enum NoiseType {
	/// Fractal Brownian Motion - smooth rolling hills
	#[default]
	Fbm,
	/// Ridge noise - sharp mountain ridges
	Ridge,
	/// Turbulence - billowy, cloud-like formations
	Turbulence,
}

/// Volume sampler that generates terrain by combining a height gradient with 3D noise.
///
/// Creates realistic terrain by:
/// 1. Base height gradient: solid below `base_height`, air above
/// 2. Surface noise: modulates the terrain surface for hills/valleys
/// 3. Cave noise (optional): carves out underground cavities
///
/// Unlike pure volumetric noise, this produces terrain-like features with
/// a definite ground level and sky.
#[derive(Clone)]
pub struct SimdNoiseTerrain {
	/// Noise type for surface modulation
	pub noise_type: NoiseType,

	/// Base terrain height in world units (default: 0.0)
	/// Below this = solid, above = air (before noise modulation)
	pub base_height: f32,

	/// Height gradient strength (default: 1.0)
	/// Higher values = sharper transition between ground and air
	/// Lower values = more gradual, noise-dominated terrain
	pub height_gradient: f32,

	/// Surface noise amplitude - how much noise affects terrain height
	/// Higher = more dramatic hills/valleys (default: 100.0)
	pub surface_amplitude: f32,

	/// Surface noise frequency - terrain feature size
	/// Smaller = larger features (default: 0.002)
	pub surface_frequency: f32,

	/// Number of octaves for surface noise (default: 6)
	pub surface_octaves: u8,

	/// Enable 3D cave generation
	pub caves_enabled: bool,

	/// Cave noise threshold - values above this are carved out
	/// Range [-1, 1], higher = fewer caves (default: 0.3)
	pub cave_threshold: f32,

	/// Cave noise frequency - cave tunnel size
	/// Smaller = larger caves (default: 0.01)
	pub cave_frequency: f32,

	/// Cave depth limit - caves only below this Y level
	/// Prevents surface from being Swiss cheese (default: -10.0)
	pub cave_depth: f32,

	/// Lacunarity for fractal noise (default: 2.0)
	pub lacunarity: f32,

	/// Gain for fractal noise (default: 0.5)
	pub gain: f32,

	/// Random seed
	pub seed: i32,
}

impl Default for SimdNoiseTerrain {
	fn default() -> Self {
		Self {
			noise_type: NoiseType::Fbm,
			base_height: 0.0,
			height_gradient: 0.1,        // Gentle gradient for noise-dominated terrain
			surface_amplitude: 500.0,    // Large amplitude for visible hills
			surface_frequency: 0.01,     // Noise frequency (100 unit features at voxel_size=1)
			surface_octaves: 6,
			caves_enabled: false,
			cave_threshold: 0.3,
			cave_frequency: 0.05,        // Higher freq for smaller cave tunnels
			cave_depth: -100.0,
			lacunarity: 2.0,
			gain: 0.5,
			seed: 0,
		}
	}
}

impl SimdNoiseTerrain {
	/// Create a new terrain sampler with default settings.
	pub fn new(seed: i32) -> Self {
		Self {
			seed,
			..Default::default()
		}
	}

	/// Set the noise type for surface modulation.
	pub fn with_noise_type(mut self, noise_type: NoiseType) -> Self {
		self.noise_type = noise_type;
		self
	}

	/// Set base terrain height.
	///
	/// The terrain surface will be approximately at this Y level,
	/// modulated by noise.
	pub fn with_base_height(mut self, height: f32) -> Self {
		self.base_height = height;
		self
	}

	/// Set height gradient strength.
	///
	/// Controls how sharply the terrain transitions from solid to air.
	/// - 1.0: Normal gradient (default)
	/// - < 1.0: Smoother, more noise-influenced
	/// - > 1.0: Sharper, flatter terrain
	pub fn with_height_gradient(mut self, gradient: f32) -> Self {
		self.height_gradient = gradient;
		self
	}

	/// Set surface noise parameters.
	///
	/// - `amplitude`: Height variation (larger = bigger hills)
	/// - `frequency`: Feature size (smaller = larger features)
	pub fn with_surface_noise(mut self, amplitude: f32, frequency: f32) -> Self {
		self.surface_amplitude = amplitude;
		self.surface_frequency = frequency;
		self
	}

	/// Set number of octaves for surface noise.
	pub fn with_surface_octaves(mut self, octaves: u8) -> Self {
		self.surface_octaves = octaves;
		self
	}

	/// Enable cave generation with given parameters.
	///
	/// - `threshold`: Cave density (0.0-1.0, higher = fewer caves)
	/// - `frequency`: Cave size (smaller = larger caves)
	/// - `depth`: Caves only below this Y level
	pub fn with_caves(mut self, threshold: f32, frequency: f32, depth: f32) -> Self {
		self.caves_enabled = true;
		self.cave_threshold = threshold;
		self.cave_frequency = frequency;
		self.cave_depth = depth;
		self
	}

	/// Disable cave generation.
	pub fn without_caves(mut self) -> Self {
		self.caves_enabled = false;
		self
	}

	/// Set fractal noise parameters.
	pub fn with_fractal(mut self, lacunarity: f32, gain: f32) -> Self {
		self.lacunarity = lacunarity;
		self.gain = gain;
		self
	}

	/// Generate surface noise for a 32x32x32 volume.
	///
	/// simdnoise offset API:
	/// - Samples at positions: (x_offset + i, y_offset + j, z_offset + k) for i,j,k in 0..SIZE
	/// - with_freq(f) scales ALL inputs: noise((x + i) * f, (y + j) * f, (z + k) * f)
	///
	/// To get proper world-space sampling with LOD scaling:
	/// - offset = chunk origin in grid units (grid_offset)
	/// - freq = voxel_size * surface_frequency
	///
	/// This makes the noise sample at: noise((grid_offset + sample_idx) * voxel_size * frequency)
	/// = noise(world_position * frequency)
	fn generate_surface_noise(&self, grid_x: f32, grid_y: f32, grid_z: f32, freq: f32) -> Vec<f32> {
		match self.noise_type {
			NoiseType::Fbm => NoiseBuilder::fbm_3d_offset(
				grid_x,
				SAMPLE_SIZE,
				grid_y,
				SAMPLE_SIZE,
				grid_z,
				SAMPLE_SIZE,
			)
			.with_seed(self.seed)
			.with_freq(freq)
			.with_octaves(self.surface_octaves)
			.with_lacunarity(self.lacunarity)
			.with_gain(self.gain)
			.generate()
			.0,

			NoiseType::Ridge => NoiseBuilder::ridge_3d_offset(
				grid_x,
				SAMPLE_SIZE,
				grid_y,
				SAMPLE_SIZE,
				grid_z,
				SAMPLE_SIZE,
			)
			.with_seed(self.seed)
			.with_freq(freq)
			.with_octaves(self.surface_octaves)
			.with_lacunarity(self.lacunarity)
			.with_gain(self.gain)
			.generate()
			.0,

			NoiseType::Turbulence => NoiseBuilder::turbulence_3d_offset(
				grid_x,
				SAMPLE_SIZE,
				grid_y,
				SAMPLE_SIZE,
				grid_z,
				SAMPLE_SIZE,
			)
			.with_seed(self.seed)
			.with_freq(freq)
			.with_octaves(self.surface_octaves)
			.with_lacunarity(self.lacunarity)
			.with_gain(self.gain)
			.generate()
			.0,
		}
	}

	/// Generate cave noise for a 32x32x32 volume.
	fn generate_cave_noise(&self, grid_x: f32, grid_y: f32, grid_z: f32, freq: f32) -> Vec<f32> {
		// Scale frequency for caves (relative to surface)
		let cave_freq = freq * (self.cave_frequency / self.surface_frequency);

		// Use a different seed for caves to decorrelate from surface
		NoiseBuilder::fbm_3d_offset(
			grid_x,
			SAMPLE_SIZE,
			grid_y,
			SAMPLE_SIZE,
			grid_z,
			SAMPLE_SIZE,
		)
		.with_seed(self.seed.wrapping_add(12345))
		.with_freq(cave_freq)
		.with_octaves(3)
		.with_lacunarity(2.0)
		.with_gain(0.5)
		.generate()
		.0
	}
}

impl VolumeSampler for SimdNoiseTerrain {
	fn sample_volume(
		&self,
		grid_offset: [i64; 3],
		voxel_size: f64,
		volume: &mut [SdfSample; SAMPLE_SIZE_CB],
		materials: &mut [MaterialId; SAMPLE_SIZE_CB],
	) {
		const SIZE: usize = SAMPLE_SIZE;

		// Pass grid coordinates directly to noise generator
		// simdnoise will sample at: (grid_offset + sample_idx) * freq
		// where freq = voxel_size * surface_frequency
		// This equals: world_position * surface_frequency
		let grid_x = grid_offset[0] as f32;
		let grid_y = grid_offset[1] as f32;
		let grid_z = grid_offset[2] as f32;

		// Frequency combines voxel_size (for LOD scaling) with surface_frequency
		let freq = voxel_size as f32 * self.surface_frequency;

		// For height calculations, we need actual world Y
		let world_y_base = (grid_offset[1] as f64 * voxel_size) as f32;
		let step = voxel_size as f32;

		// Generate surface noise
		let surface_noise = self.generate_surface_noise(grid_x, grid_y, grid_z, freq);

		// Generate cave noise if enabled
		let cave_noise = if self.caves_enabled {
			Some(self.generate_cave_noise(grid_x, grid_y, grid_z, freq))
		} else {
			None
		};

		// Combine height gradient with noise
		// simdnoise outputs in X-fastest order: idx = z * SIZE² + y * SIZE + x
		// We need X-slowest order: idx = x * SIZE² + y * SIZE + z
		for vol_idx in 0..SAMPLE_SIZE_CB {
			let x = vol_idx / (SIZE * SIZE);
			let yz = vol_idx % (SIZE * SIZE);
			let y = yz / SIZE;
			let z = yz % SIZE;

			// Calculate world Y for this sample
			let sample_world_y = world_y_base + (y as f32) * step;

			// simdnoise index: X-fastest layout
			let sn_idx = z * SIZE * SIZE + y * SIZE + x;

			// Base density: height gradient
			// Positive above base_height (air), negative below (solid)
			let height_density = (sample_world_y - self.base_height) * self.height_gradient;

			// Surface noise modulation
			// Noise typically in [-1, 1] range, scaled by amplitude
			let surface_mod = surface_noise[sn_idx] * self.surface_amplitude;

			// Combined terrain density
			let mut density = height_density + surface_mod;

			// Cave carving (only underground)
			if let Some(ref caves) = cave_noise {
				if sample_world_y < self.cave_depth {
					let cave_value = caves[sn_idx];
					// Carve out where cave noise exceeds threshold
					if cave_value > self.cave_threshold {
						// Make it air (positive density)
						// The further above threshold, the more "air-like"
						let cave_strength = (cave_value - self.cave_threshold) / (1.0 - self.cave_threshold);
						density = density.max(cave_strength * 5.0);
					}
				}
			}

			volume[vol_idx] = sdf_conversion::to_storage(density, voxel_size as f32);
			materials[vol_idx] = 0;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_terrain_above_base_height_is_air() {
		let sampler = SimdNoiseTerrain::new(42)
			.with_base_height(0.0)
			.with_height_gradient(1.0)
			.with_surface_noise(0.0, 0.01); // No amplitude = pure height gradient

		let mut volume = [0i8; SAMPLE_SIZE_CB];
		let mut materials = [0u8; SAMPLE_SIZE_CB];

		// Sample at Y=100 (well above base height of 0)
		sampler.sample_volume([0, 100, 0], 1.0, &mut volume, &mut materials);

		// All samples should be positive (air)
		let all_air = volume.iter().all(|&v| v > 0);
		assert!(all_air, "Samples above base height should be air");
	}

	#[test]
	fn test_terrain_below_base_height_is_solid() {
		let sampler = SimdNoiseTerrain::new(42)
			.with_base_height(0.0)
			.with_height_gradient(1.0)
			.with_surface_noise(0.0, 0.01); // No amplitude = pure height gradient

		let mut volume = [0i8; SAMPLE_SIZE_CB];
		let mut materials = [0u8; SAMPLE_SIZE_CB];

		// Sample at Y=-100 (well below base height of 0)
		sampler.sample_volume([0, -100, 0], 1.0, &mut volume, &mut materials);

		// All samples should be negative (solid)
		let all_solid = volume.iter().all(|&v| v < 0);
		assert!(all_solid, "Samples below base height should be solid");
	}

	#[test]
	fn test_terrain_has_surface_at_base_height() {
		let sampler = SimdNoiseTerrain::new(42)
			.with_base_height(16.0)
			.with_height_gradient(1.0)
			.with_surface_noise(5.0, 0.1);

		let mut volume = [0i8; SAMPLE_SIZE_CB];
		let mut materials = [0u8; SAMPLE_SIZE_CB];

		// Sample chunk that spans the surface (Y=0 to Y=32)
		sampler.sample_volume([0, 0, 0], 1.0, &mut volume, &mut materials);

		// Should have both solid (negative) and air (positive) samples
		let has_solid = volume.iter().any(|&v| v < 0);
		let has_air = volume.iter().any(|&v| v > 0);
		assert!(has_solid && has_air, "Terrain should have both solid and air near surface");
	}

	#[test]
	fn test_noise_creates_variation() {
		let sampler = SimdNoiseTerrain::new(42)
			.with_base_height(16.0)
			.with_surface_noise(50.0, 0.1);

		let mut volume = [0i8; SAMPLE_SIZE_CB];
		let mut materials = [0u8; SAMPLE_SIZE_CB];

		sampler.sample_volume([0, 0, 0], 1.0, &mut volume, &mut materials);

		// Should have variation in values
		let first = volume[0];
		let has_variation = volume.iter().any(|&v| v != first);
		assert!(has_variation, "Surface noise should create variation");
	}

	#[test]
	fn test_different_seeds_produce_different_terrain() {
		let sampler1 = SimdNoiseTerrain::new(42)
			.with_surface_noise(100.0, 0.1);
		let sampler2 = SimdNoiseTerrain::new(123)
			.with_surface_noise(100.0, 0.1);

		let mut volume1 = [0i8; SAMPLE_SIZE_CB];
		let mut volume2 = [0i8; SAMPLE_SIZE_CB];
		let mut materials = [0u8; SAMPLE_SIZE_CB];

		sampler1.sample_volume([0, 0, 0], 1.0, &mut volume1, &mut materials);
		sampler2.sample_volume([0, 0, 0], 1.0, &mut volume2, &mut materials);

		assert_ne!(volume1, volume2, "Different seeds should produce different terrain");
	}

	#[test]
	fn test_chunk_boundary_coherency() {
		let sampler = SimdNoiseTerrain::new(42)
			.with_surface_noise(100.0, 0.1);

		let mut volume1 = [0i8; SAMPLE_SIZE_CB];
		let mut volume2 = [0i8; SAMPLE_SIZE_CB];
		let mut materials = [0u8; SAMPLE_SIZE_CB];

		// Sample two adjacent chunks (grid offset differs by 32)
		sampler.sample_volume([0, 0, 0], 1.0, &mut volume1, &mut materials);
		sampler.sample_volume([32, 0, 0], 1.0, &mut volume2, &mut materials);

		// Different chunks should have different content
		assert_ne!(volume1, volume2, "Adjacent chunks should have different values");
	}

	#[test]
	fn test_lod_scaling() {
		let sampler = SimdNoiseTerrain::new(42)
			.with_surface_noise(100.0, 0.1);

		let mut volume_lod0 = [0i8; SAMPLE_SIZE_CB];
		let mut volume_lod1 = [0i8; SAMPLE_SIZE_CB];
		let mut materials = [0u8; SAMPLE_SIZE_CB];

		// Sample same grid position but different voxel sizes (LOD levels)
		sampler.sample_volume([0, 0, 0], 1.0, &mut volume_lod0, &mut materials);
		sampler.sample_volume([0, 0, 0], 2.0, &mut volume_lod1, &mut materials);

		// Different LODs should produce different results
		assert_ne!(volume_lod0, volume_lod1, "Different LOD levels should produce different noise");
	}
}
