use ilattice::prelude::{Extent, IVec3, UVec3};

/// Describes the logical dimensions of a voxel chunk.
///
/// - `core` is the interior span of editable/authorable voxels (N×N×N)
/// - `sample` is the allocated grid including a 1-voxel apron on each face: (N+2)×(N+2)×(N+2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkDims {
    pub core: UVec3,
    pub sample: UVec3,
}

impl ChunkDims {
    /// Construct from the interior (core) dimensions.
    pub fn from_core(core: UVec3) -> Self {
        let sample = core + UVec3::splat(2);
        Self { core, sample }
    }

    /// Number of samples including the apron.
    pub fn sample_len(&self) -> usize {
        (self.sample.x * self.sample.y * self.sample.z) as usize
    }
}

/// Interior extent: min at `origin_cell`, shape equals `core`.
pub fn core_extent(origin_cell: IVec3, core: UVec3) -> Extent<IVec3> {
    Extent::from_min_and_shape(
        origin_cell,
        IVec3::new(core.x as i32, core.y as i32, core.z as i32),
    )
}

/// Sample extent: min at `origin_cell - 1`, shape equals `core + 2` (apron on all faces).
pub fn sample_extent(origin_cell: IVec3, core: UVec3) -> Extent<IVec3> {
    let dims = ChunkDims::from_core(core);
    let min = origin_cell - IVec3::ONE;
    Extent::from_min_and_shape(
        min,
        IVec3::new(
            dims.sample.x as i32,
            dims.sample.y as i32,
            dims.sample.z as i32,
        ),
    )
}
