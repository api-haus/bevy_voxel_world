//! Seed volume with random content

use crate::voxel::{ports::RandomGenerator, services, VoxelVolume};

/// Command to seed a volume with random spheres
#[derive(Debug, Clone)]
pub struct SeedVolumeCommand {
	pub sphere_count: usize,
	pub min_radius: f32,
	pub max_radius: f32,
}

/// Result of seed operation
#[derive(Debug, Clone)]
pub struct SeedResult {
	pub chunks_with_content: usize,
}

/// Execute the seed volume command
pub fn execute_seed_volume(
	command: SeedVolumeCommand,
	volume: &mut VoxelVolume,
	rng: &mut impl RandomGenerator,
) -> SeedResult {
	services::generate_random_spheres(
		volume,
		command.sphere_count,
		command.min_radius,
		command.max_radius,
		rng,
	);

	let chunks_with_content = volume
		.chunks
		.iter()
		.filter(|chunk| chunk.has_solids())
		.count();

	SeedResult {
		chunks_with_content,
	}
}
