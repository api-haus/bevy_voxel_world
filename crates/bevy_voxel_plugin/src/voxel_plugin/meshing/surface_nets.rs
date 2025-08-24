use crate::core::index::linear_index;
use crate::voxel_plugin::voxels::storage::{VoxelStorage, AIR_ID};
use fast_surface_nets::ndshape::ConstShape3u32;
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};
use ilattice::prelude::UVec3;

/// A simple wrapper that runs Surface Nets on fixed chunk sizes.
/// Returns `None` if the chunk is entirely positive or entirely negative, including apron.
pub fn remesh_chunk_fixed<const SX: u32, const SY: u32, const SZ: u32>(
	storage: &VoxelStorage,
) -> Option<SurfaceNetsBuffer> {
	// Expect matching dims (including apron)
	debug_assert_eq!(storage.dims.sample.x, SX);
	debug_assert_eq!(storage.dims.sample.y, SY);
	debug_assert_eq!(storage.dims.sample.z, SZ);

	// Early skip if all positive or all negative
	let mut any_pos = false;
	let mut any_neg = false;
	for &s in storage.sdf.iter() {
		if s <= 0.0 {
			any_neg = true;
		} else {
			any_pos = true;
		}
		if any_pos && any_neg {
			break;
		}
	}
	if !(any_pos && any_neg) {
		return None;
	}

	// Run surface nets on the padded array
	let mut buffer = SurfaceNetsBuffer::default();
	surface_nets(
		&storage.sdf,
		&ConstShape3u32::<SX, SY, SZ>,
		[0; 3],
		[SX - 1, SY - 1, SZ - 1],
		&mut buffer,
	);

	if buffer.positions.is_empty() {
		None
	} else {
		Some(buffer)
	}
}

/// Dispatch to a supported fixed-size meshing implementation based on the storage sample dims.
/// Currently supports only 16^3 core (18^3 sample) chunks.
pub fn remesh_chunk_dispatch(storage: &VoxelStorage) -> Option<SurfaceNetsBuffer> {
	let s = storage.dims.sample;
	if s.x == 18 && s.y == 18 && s.z == 18 {
		return remesh_chunk_fixed::<18, 18, 18>(storage);
	}
	if s.x == 34 && s.y == 34 && s.z == 34 {
		return remesh_chunk_fixed::<34, 34, 34>(storage);
	}
	None
}

/// Select per-vertex material IDs based on surrounding voxel SDF/material data.
///
/// For each vertex position, determine the cell min-corner by flooring the position and clamping so that (cx+1,cy+1,cz+1) are in-bounds.
/// Inspect the 8 neighboring samples (SDF + material), choose the corner with minimal abs(sdf).
/// Tie-break by majority material among tied corners; fallback to first. Prefer non-air when present.
pub fn select_vertex_materials_from_positions_arrays(
	sample_dims: UVec3,
	sdf: &[f32],
	mat: &[u8],
	positions: &[[f32; 3]],
) -> Vec<u8> {
	debug_assert_eq!(
		sdf.len(),
		(sample_dims.x * sample_dims.y * sample_dims.z) as usize
	);
	debug_assert_eq!(
		mat.len(),
		(sample_dims.x * sample_dims.y * sample_dims.z) as usize
	);
	let mut out = Vec::with_capacity(positions.len());
	if sample_dims.x < 2 || sample_dims.y < 2 || sample_dims.z < 2 {
		out.resize(positions.len(), AIR_ID);
		return out;
	}
	let max_x = sample_dims.x - 2;
	let max_y = sample_dims.y - 2;
	let max_z = sample_dims.z - 2;
	for p in positions.iter() {
		let mut cx = p[0].floor() as i32;
		let mut cy = p[1].floor() as i32;
		let mut cz = p[2].floor() as i32;
		if cx < 0 {
			cx = 0;
		}
		if cy < 0 {
			cy = 0;
		}
		if cz < 0 {
			cz = 0;
		}
		if cx as u32 > max_x {
			cx = max_x as i32;
		}
		if cy as u32 > max_y {
			cy = max_y as i32;
		}
		if cz as u32 > max_z {
			cz = max_z as i32;
		}
		let cx = cx as u32;
		let cy = cy as u32;
		let cz = cz as u32;

		let mut min_abs = f32::INFINITY;
		let mut tie_indices: [usize; 8] = [0; 8];
		let mut tie_count: usize = 0;
		let mut any_non_air = false;
		let mut non_air_best_mat: u8 = AIR_ID;
		let mut non_air_best_abs = f32::INFINITY;

		for dz in 0..=1u32 {
			for dy in 0..=1u32 {
				for dx in 0..=1u32 {
					let x = cx + dx;
					let y = cy + dy;
					let z = cz + dz;
					let idx = linear_index(x, y, z, sample_dims);
					let v = sdf[idx].abs();
					let m = mat[idx];
					if m != AIR_ID {
						any_non_air = true;
					}
					if m != AIR_ID && v < non_air_best_abs {
						non_air_best_abs = v;
						non_air_best_mat = m;
					}
					if v < min_abs - 1e-7 {
						min_abs = v;
						tie_count = 0;
						tie_indices[tie_count] = idx;
						tie_count = 1;
					} else if (v - min_abs).abs() <= 1e-7 {
						tie_indices[tie_count] = idx;
						tie_count += 1;
					}
				}
			}
		}

		let mut chosen_mat = mat[tie_indices[0]];
		if tie_count > 1 {
			// Majority material among ties
			let mut counts: [u32; 256] = [0; 256];
			for i in 0..tie_count {
				let m = mat[tie_indices[i]] as usize;
				counts[m] += 1;
			}
			let mut best_m: usize = chosen_mat as usize;
			let mut best_c: u32 = counts[best_m];
			for m in 0..256usize {
				if counts[m] > best_c {
					best_c = counts[m];
					best_m = m;
				}
			}
			chosen_mat = best_m as u8;
		}
		if chosen_mat == AIR_ID && any_non_air {
			chosen_mat = non_air_best_mat;
		}
		out.push(chosen_mat);
	}
	out
}

/// Convenience wrapper: select vertex materials using an owned storage.
pub fn select_vertex_materials_from_positions(
	storage: &VoxelStorage,
	positions: &[[f32; 3]],
) -> Vec<u8> {
	select_vertex_materials_from_positions_arrays(
		storage.dims.sample,
		&storage.sdf,
		&storage.mat,
		positions,
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn selects_non_air_on_tie_and_majority_material() {
		let sample_dims = UVec3::new(3, 3, 3);
		let len = (sample_dims.x * sample_dims.y * sample_dims.z) as usize;
		let mut sdf = vec![10.0f32; len];
		let mut mat = vec![AIR_ID; len];

		// Target cell min-corner (1,1,1)
		let cx = 1u32;
		let cy = 1u32;
		let cz = 1u32;

		// Case A: two corners tie on minimal abs(sdf)=0.2; one AIR, one non-air (4)
		let idx_air = linear_index(cx, cy, cz, sample_dims);
		sdf[idx_air] = 0.2;
		mat[idx_air] = AIR_ID;
		let idx_non_air = linear_index(cx + 1, cy, cz, sample_dims);
		sdf[idx_non_air] = 0.2;
		mat[idx_non_air] = 4u8;

		// Case B (majority): set three corners to equal minimal abs(sdf)=0.1 with mats 5,5,7
		let v1 = linear_index(cx, cy + 1, cz, sample_dims);
		sdf[v1] = 0.1;
		mat[v1] = 5u8;
		let v2 = linear_index(cx + 1, cy + 1, cz, sample_dims);
		sdf[v2] = 0.1;
		mat[v2] = 5u8;
		let v3 = linear_index(cx, cy, cz + 1, sample_dims);
		sdf[v3] = 0.1;
		mat[v3] = 7u8;

		let positions = vec![[1.2f32, 1.3f32, 1.4f32]];
		let out = select_vertex_materials_from_positions_arrays(sample_dims, &sdf, &mat, &positions);
		assert_eq!(out, vec![5u8]);

		// Remove the 0.1 majority to test non-air preference at 0.2 tie
		sdf[v1] = 10.0;
		mat[v1] = AIR_ID;
		sdf[v2] = 10.0;
		mat[v2] = AIR_ID;
		sdf[v3] = 10.0;
		mat[v3] = AIR_ID;

		let out2 = select_vertex_materials_from_positions_arrays(sample_dims, &sdf, &mat, &positions);
		assert_eq!(out2, vec![4u8]);
	}
}
