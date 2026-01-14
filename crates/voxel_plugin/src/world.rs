//! VoxelWorld - isolated voxel world container.
//!
//! Each world has its own octree state, configuration, sampler, and transform.
//! Multiple worlds can exist independently (overworld, dioramas, voxel characters).

use std::sync::atomic::{AtomicU64, Ordering};

use glam::DAffine3;

use crate::octree::{OctreeConfig, OctreeLeaves};
use crate::pipeline::VolumeSampler;

// =============================================================================
// WorldId - unique identifier
// =============================================================================

/// Atomic counter for generating unique WorldIds.
static WORLD_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Opaque world identifier.
///
/// Generated atomically - guaranteed unique within process lifetime.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WorldId(u64);

impl WorldId {
    /// Generate a new unique WorldId.
    pub fn new() -> Self {
        Self(WORLD_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for WorldId {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// VoxelWorld<S> - per-world state container
// =============================================================================

/// Per-world voxel state container, generic over sampler.
///
/// Type parameter `S` allows compile-time sampler specialization for hot paths.
/// Engine bridges (Bevy, Unity) may use `Box<dyn VolumeSampler>` for runtime flexibility.
///
/// # Transform
///
/// The `transform` field positions the world in global space. Use helper methods
/// to convert between local (octree) and world coordinates:
/// - `viewer_to_local`: Convert global viewer position to local octree space
/// - `local_to_world`: Convert local octree position to global space
pub struct VoxelWorld<S: VolumeSampler> {
    /// Unique world identifier.
    pub id: WorldId,

    /// Octree configuration (LOD settings, voxel size, etc.).
    pub config: OctreeConfig,

    /// Current octree leaf nodes (implicit tree structure).
    pub leaves: OctreeLeaves,

    /// Volume sampler for this world.
    pub sampler: S,

    /// World-space transform (position, rotation, scale).
    /// Converts from local octree space to global world space.
    pub transform: DAffine3,
}

impl<S: VolumeSampler> VoxelWorld<S> {
    /// Create a new world with identity transform.
    pub fn new(config: OctreeConfig, sampler: S) -> Self {
        Self {
            id: WorldId::new(),
            config,
            leaves: OctreeLeaves::default(),
            sampler,
            transform: DAffine3::IDENTITY,
        }
    }

    /// Create a new world with initial leaves at given LOD.
    pub fn new_with_initial_lod(config: OctreeConfig, sampler: S, initial_lod: i32) -> Self {
        Self {
            id: WorldId::new(),
            config,
            leaves: OctreeLeaves::new_with_initial(initial_lod),
            sampler,
            transform: DAffine3::IDENTITY,
        }
    }

    /// Set the world transform.
    pub fn set_transform(&mut self, transform: DAffine3) {
        self.transform = transform;
    }

    /// Convert a global position to local octree space.
    ///
    /// Use this to transform viewer position before refinement calculations.
    #[inline]
    pub fn viewer_to_local(&self, global_pos: glam::DVec3) -> glam::DVec3 {
        self.transform.inverse().transform_point3(global_pos)
    }

    /// Convert a local octree position to global world space.
    ///
    /// Use this to position chunks in the scene.
    #[inline]
    pub fn local_to_world(&self, local_pos: glam::DVec3) -> glam::DVec3 {
        self.transform.transform_point3(local_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::SAMPLE_SIZE_CB;
    use crate::types::{MaterialId, SdfSample};

    /// Mock sampler for testing.
    struct MockSampler;

    impl VolumeSampler for MockSampler {
        fn sample_volume(
            &self,
            _sample_start: [f64; 3],
            _voxel_size: f64,
            volume: &mut [SdfSample; SAMPLE_SIZE_CB],
            materials: &mut [MaterialId; SAMPLE_SIZE_CB],
        ) {
            // Fill with all-air (positive values = outside)
            volume.fill(127);
            materials.fill(0);
        }
    }

    #[test]
    fn world_id_is_unique() {
        let id1 = WorldId::new();
        let id2 = WorldId::new();
        let id3 = WorldId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn world_creation() {
        let config = OctreeConfig::default();
        let world = VoxelWorld::new(config, MockSampler);

        assert!(world.leaves.is_empty());
        assert_eq!(world.transform, DAffine3::IDENTITY);
    }

    #[test]
    fn world_with_initial_lod() {
        let config = OctreeConfig::default();
        let world = VoxelWorld::new_with_initial_lod(config, MockSampler, 5);

        assert_eq!(world.leaves.len(), 1);
    }

    #[test]
    fn transform_roundtrip() {
        let config = OctreeConfig::default();
        let mut world = VoxelWorld::new(config, MockSampler);

        // Set a non-identity transform
        let translation = glam::DVec3::new(100.0, 50.0, 200.0);
        world.set_transform(DAffine3::from_translation(translation));

        // Round-trip a point
        let global_pos = glam::DVec3::new(150.0, 75.0, 250.0);
        let local_pos = world.viewer_to_local(global_pos);
        let back_to_global = world.local_to_world(local_pos);

        assert!((global_pos - back_to_global).length() < 1e-10);
    }
}
