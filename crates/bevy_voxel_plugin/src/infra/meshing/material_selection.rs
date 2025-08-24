//! Material selection for mesh vertices

use crate::voxel::{LocalVoxelPos, MaterialId, VoxelChunk};

/// Select per-vertex material IDs based on surrounding voxel data
pub fn select_vertex_materials(chunk: &VoxelChunk, positions: &[[f32; 3]]) -> Vec<MaterialId> {
	let mut materials = Vec::with_capacity(positions.len());

	for pos in positions {
		let material = select_material_at_position(chunk, pos);
		materials.push(material);
	}

	materials
}

fn select_material_at_position(chunk: &VoxelChunk, pos: &[f32; 3]) -> MaterialId {
	// Find the cell containing this position
	let cx = pos[0].floor() as i32;
	let cy = pos[1].floor() as i32;
	let cz = pos[2].floor() as i32;

	// Clamp to valid range
	let dims = chunk.dims.sample;
	let cx = cx.clamp(0, dims.x as i32 - 2) as u32;
	let cy = cy.clamp(0, dims.y as i32 - 2) as u32;
	let cz = cz.clamp(0, dims.z as i32 - 2) as u32;

	// Sample 8 corners of the cell
	let mut min_abs_sdf = f32::INFINITY;
	let mut best_material = MaterialId::AIR;
	let mut any_non_air = false;
	let mut non_air_material = MaterialId::AIR;

	for dz in 0..=1u32 {
		for dy in 0..=1u32 {
			for dx in 0..=1u32 {
				let local_pos = LocalVoxelPos {
					x: cx + dx,
					y: cy + dy,
					z: cz + dz,
				};

				let sdf = chunk.sdf_at(local_pos);
				let mat = chunk.material_at(local_pos);
				let abs_sdf = sdf.0.abs();

				if mat != MaterialId::AIR {
					any_non_air = true;
					if abs_sdf < min_abs_sdf || non_air_material == MaterialId::AIR {
						non_air_material = mat;
					}
				}

				if abs_sdf < min_abs_sdf {
					min_abs_sdf = abs_sdf;
					best_material = mat;
				}
			}
		}
	}

	// Prefer non-air materials when available
	if best_material == MaterialId::AIR && any_non_air {
		non_air_material
	} else {
		best_material
	}
}
