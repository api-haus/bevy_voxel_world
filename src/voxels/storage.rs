use crate::core::grid::ChunkDims;
use crate::core::index::linear_index;
use bevy::prelude::Component;
use ilattice::prelude::UVec3;

pub const AIR_ID: u8 = 0u8;

/// Structure-of-Arrays voxel storage for a chunk, including a 1-voxel apron.
/// - `sdf` stores signed distances in world units.
/// - `mat` stores material IDs; `0` is reserved for air.
#[derive(Debug, Component)]
pub struct VoxelStorage {
    pub dims: ChunkDims,
    pub sdf: Box<[f32]>,
    pub mat: Box<[u8]>,
}

impl VoxelStorage {
    /// Allocate storage sized for `core_dims`, including the +1 apron on all faces.
    pub fn new(core_dims: UVec3) -> Self {
        let dims = ChunkDims::from_core(core_dims);
        let len = dims.sample_len();
        let sdf = vec![0.0f32; len].into_boxed_slice();
        let mat = vec![AIR_ID; len].into_boxed_slice();
        Self { dims, sdf, mat }
    }

    /// Fill entire arrays with constant values.
    pub fn fill_default(&mut self, sdf_value: f32, material_id: u8) {
        self.sdf.fill(sdf_value);
        self.mat.fill(material_id);
    }

    /// Get a mutable reference to the SDF value at sample coordinates (including apron).
    #[inline]
    pub fn sdf_mut_at(&mut self, x: u32, y: u32, z: u32) -> &mut f32 {
        let idx = linear_index(x, y, z, self.dims.sample);
        &mut self.sdf[idx]
    }

    /// Get a mutable reference to the material at sample coordinates (including apron).
    #[inline]
    pub fn mat_mut_at(&mut self, x: u32, y: u32, z: u32) -> &mut u8 {
        let idx = linear_index(x, y, z, self.dims.sample);
        &mut self.mat[idx]
    }

    /// Placeholder for apron refresh from neighbor chunks. To be implemented later.
    pub fn copy_apron_from_neighbors(&mut self) {
        // Stub: will be filled when chunk neighbor topology exists.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_sizes_match_dims() {
        let storage = VoxelStorage::new(UVec3::new(16, 16, 16));
        assert_eq!(storage.dims.sample, UVec3::new(18, 18, 18));
        assert_eq!(storage.sdf.len(), 18 * 18 * 18);
        assert_eq!(storage.mat.len(), 18 * 18 * 18);
    }

    #[test]
    fn fill_and_indexing_mut() {
        let mut storage = VoxelStorage::new(UVec3::new(16, 16, 16));
        storage.fill_default(42.0, 7);
        assert!(storage.sdf.iter().all(|&v| v == 42.0));
        assert!(storage.mat.iter().all(|&m| m == 7));

        // Write a couple of interior and apron samples
        *storage.sdf_mut_at(0, 0, 0) = -1.0; // apron corner
        *storage.mat_mut_at(17, 17, 17) = 3; // apron opposite corner (18 dims => last index 17)

        assert_eq!(storage.sdf[0], -1.0);

        let last_idx = linear_index(17, 17, 17, storage.dims.sample);
        assert_eq!(storage.mat[last_idx], 3);
    }
}
