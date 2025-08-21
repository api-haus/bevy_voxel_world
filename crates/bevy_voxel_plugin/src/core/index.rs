use ilattice::prelude::UVec3;

/// Convert 3D sample coordinates to a linear index within a `sample_dims` grid.
///
/// Panics in debug if coordinates are out of bounds.
#[inline]
pub fn linear_index(x: u32, y: u32, z: u32, sample_dims: UVec3) -> usize {
	debug_assert!(x < sample_dims.x && y < sample_dims.y && z < sample_dims.z);
	let xy = sample_dims.x * sample_dims.y;
	(z * xy + y * sample_dims.x + x) as usize
}

/// Inverse of `linear_index`. Returns `(x, y, z)` for a given index.
#[inline]
pub fn delinearize(index: usize, sample_dims: UVec3) -> (u32, u32, u32) {
	let xy = (sample_dims.x * sample_dims.y) as usize;
	let z = (index / xy) as u32;
	let rem = index % xy;
	let y = (rem / sample_dims.x as usize) as u32;
	let x = (rem % sample_dims.x as usize) as u32;
	(x, y, z)
}
