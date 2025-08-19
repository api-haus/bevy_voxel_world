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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_linearization() {
        let dims = UVec3::new(18, 18, 18);
        for z in 0..dims.z {
            for y in 0..dims.y {
                for x in 0..dims.x {
                    let idx = linear_index(x, y, z, dims);
                    let (rx, ry, rz) = delinearize(idx, dims);
                    assert_eq!((rx, ry, rz), (x, y, z));
                }
            }
        }
    }

    #[test]
    fn edge_indices_and_bounds() {
        let dims = UVec3::new(3, 4, 5);
        let last = linear_index(2, 3, 4, dims);
        assert_eq!(
            last,
            dims.x as usize * dims.y as usize * dims.z as usize - 1
        );

        let first = linear_index(0, 0, 0, dims);
        assert_eq!(first, 0);
    }
}
