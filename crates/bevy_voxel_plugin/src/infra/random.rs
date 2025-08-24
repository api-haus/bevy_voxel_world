//! Random number generator adapter

use crate::voxel::ports::RandomGenerator;
use rand::Rng;

/// WyRand-based random generator adapter
pub struct WyRandGenerator<R: Rng> {
	rng: R,
}

impl<R: Rng> WyRandGenerator<R> {
	pub fn new(rng: R) -> Self {
		Self { rng }
	}
}

impl<R: Rng> RandomGenerator for WyRandGenerator<R> {
	#[allow(deprecated)]
	fn random_range_i32(&mut self, min: i32, max: i32) -> i32 {
		self.rng.gen_range(min..max)
	}

	#[allow(deprecated)]
	fn random_range_f32(&mut self, min: f32, max: f32) -> f32 {
		self.rng.gen_range(min..max)
	}
}
