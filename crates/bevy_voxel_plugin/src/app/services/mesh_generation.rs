//! Mesh generation service

use crate::app::commands::{RemeshChunkCommand, execute_remesh_chunk};

use crate::voxel::{ChunkCoords, MeshData, VoxelVolume, ports::Mesher};

use std::collections::VecDeque;

/// Service for managing mesh generation
pub struct MeshGenerationService<M: Mesher> {
	pub mesher: M,
	pub queue: VecDeque<ChunkCoords>,
}

impl<M: Mesher> MeshGenerationService<M> {
	/// Create a new mesh generation service
	pub fn new(mesher: M) -> Self {
		Self {
			mesher,
			queue: VecDeque::new(),
		}
	}

	/// Queue chunks for meshing
	pub fn queue_chunks(&mut self, chunks: Vec<ChunkCoords>) {
		self.queue.extend(chunks);
	}

	/// Process queued chunks up to a budget
	pub fn process_queue(
		&mut self,
		volume: &VoxelVolume,
		budget: usize,
	) -> Vec<(ChunkCoords, Option<MeshData>)> {
		let mut results = Vec::new();

		for _ in 0..budget {
			let Some(coords) = self.queue.pop_front() else {
				break;
			};

			let command = RemeshChunkCommand {
				chunk_coords: coords,
			};

			match execute_remesh_chunk(command, volume, &self.mesher) {
				Ok(result) => {
					results.push((coords, result.mesh_data));
				}

				Err(_) => {
					// Log error in real implementation
					results.push((coords, None));
				}
			}
		}

		results
	}

	/// Get queue length
	pub fn queue_len(&self) -> usize {
		self.queue.len()
	}
}
