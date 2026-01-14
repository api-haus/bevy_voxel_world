//! Native FastNoise2 implementation using direct FFI

use fastnoise2::SafeNode;

/// A noise generator node created from an encoded node tree string.
///
/// Encoded strings can be exported from FastNoise2's NoiseTool application.
/// This provides a simple way to design complex noise graphs visually and
/// use them in code.
pub struct NoiseNode {
	inner: SafeNode,
}

impl NoiseNode {
	/// Create a noise node from an encoded node tree string.
	///
	/// Returns `None` if the encoded string is invalid.
	///
	/// # Example
	/// ```ignore
	/// let node = NoiseNode::from_encoded("DQAFAAAAAAAAQAgAAAAAAD8AAAAAAA==").unwrap();
	/// ```
	pub fn from_encoded(encoded: &str) -> Option<Self> {
		SafeNode::from_encoded_node_tree(encoded)
			.ok()
			.map(|inner| Self { inner })
	}

	/// Generate noise values on a uniform 3D grid.
	///
	/// # Arguments
	/// * `output` - Buffer to write noise values into (must be x_cnt * y_cnt * z_cnt in size)
	/// * `x_off, y_off, z_off` - Grid origin offset
	/// * `x_cnt, y_cnt, z_cnt` - Grid dimensions (number of samples per axis)
	/// * `x_step, y_step, z_step` - Step size between samples (effectively frequency)
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
		self.inner.gen_uniform_grid_3d(
			output, x_off, y_off, z_off, x_cnt, y_cnt, z_cnt, x_step, y_step, z_step, seed,
		);
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
		self.inner
			.gen_uniform_grid_2d(output, x_off, y_off, x_cnt, y_cnt, x_step, y_step, seed);
	}
}

// NoiseNode is Send + Sync because SafeNode is
unsafe impl Send for NoiseNode {}
unsafe impl Sync for NoiseNode {}
