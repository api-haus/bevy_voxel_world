//! VoxelWorld - isolated voxel world container.
//!
//! Each world has its own octree state, configuration, sampler, and transform.
//! Multiple worlds can exist independently (overworld, dioramas, voxel
//! characters).

use std::sync::atomic::{AtomicU64, Ordering};

use glam::{DAffine3, DVec3};

use crate::octree::{OctreeConfig, OctreeLeaves, RefinementBudget, RefinementInput, RefinementOutput};
use crate::pipeline::{
  process_transitions, ChunkPresentation, PresentationBatch, ReadyChunk, VolumeSampler,
};
#[cfg(feature = "metrics")]
use crate::metrics::WorldMetrics;

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
/// Engine bridges (Bevy, Unity) may use `Box<dyn VolumeSampler>` for runtime
/// flexibility.
///
/// # Transform
///
/// The `transform` field positions the world in global space. Use helper
/// methods to convert between local (octree) and world coordinates:
/// - `viewer_to_local`: Convert global viewer position to local octree space
/// - `local_to_world`: Convert local octree position to global space
///
/// # Refinement API
///
/// Use `refine()` to compute LOD transitions based on viewer position:
///
/// ```ignore
/// // Each frame:
/// let output = world.refine(viewer_local_pos);
/// // Apply output.transition_groups to spawn/despawn chunks
/// ```
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

  /// Refinement budget (limits per-frame work).
  pub budget: RefinementBudget,

  /// World metrics (timing histograms, counters).
  /// Only available when compiled with `metrics` feature.
  #[cfg(feature = "metrics")]
  pub metrics: WorldMetrics,
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
      budget: RefinementBudget::DEFAULT,
      #[cfg(feature = "metrics")]
      metrics: WorldMetrics::default(),
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
      budget: RefinementBudget::DEFAULT,
      #[cfg(feature = "metrics")]
      metrics: WorldMetrics::default(),
    }
  }

  /// Set the world transform.
  pub fn set_transform(&mut self, transform: DAffine3) {
    self.transform = transform;
  }

  /// Set the refinement budget (limits per-frame work).
  pub fn set_budget(&mut self, budget: RefinementBudget) {
    self.budget = budget;
  }

  /// Convert a global position to local octree space.
  ///
  /// Use this to transform viewer position before refinement calculations.
  #[inline]
  pub fn viewer_to_local(&self, global_pos: DVec3) -> DVec3 {
    self.transform.inverse().transform_point3(global_pos)
  }

  /// Convert a local octree position to global world space.
  ///
  /// Use this to position chunks in the scene.
  #[inline]
  pub fn local_to_world(&self, local_pos: DVec3) -> DVec3 {
    self.transform.transform_point3(local_pos)
  }

  /// Refine the octree based on viewer position.
  ///
  /// Returns transition groups describing chunks to spawn/despawn.
  /// Also updates internal leaves state.
  ///
  /// # Example
  ///
  /// ```ignore
  /// let viewer_local = world.viewer_to_local(viewer_world_pos);
  /// let output = world.refine(viewer_local);
  /// for group in &output.transition_groups {
  ///     // Despawn group.nodes_to_remove, spawn group.nodes_to_add
  /// }
  /// ```
  pub fn refine(&mut self, viewer_pos: DVec3) -> RefinementOutput {
    #[cfg(feature = "metrics")]
    let start = web_time::Instant::now();

    let input = RefinementInput {
      viewer_pos,
      config: self.config.clone(),
      prev_leaves: self.leaves.as_set().clone(),
      budget: self.budget,
    };

    let output = crate::octree::refine(input);

    // Update leaves to match refinement output
    self.leaves = OctreeLeaves::from(output.next_leaves.clone());

    #[cfg(feature = "metrics")]
    {
      let elapsed = start.elapsed().as_micros() as u64;
      self.metrics.record_refine_timing(elapsed);
      self.metrics.record_transitions(output.transition_groups.len());
    }

    output
  }

  /// Update world state based on viewer position.
  ///
  /// Runs refinement and parallel mesh generation in one call.
  /// Returns PresentationBatch with chunks to despawn/spawn.
  ///
  /// This is the primary API for engine bridges - combines `refine()` with
  /// `process_transitions()` to produce ready-to-render chunks.
  ///
  /// # Example
  ///
  /// ```ignore
  /// // Each frame:
  /// let batch = world.update(viewer_pos);
  /// for node in &batch.to_despawn {
  ///     // Despawn entity for node
  /// }
  /// for chunk in &batch.to_spawn {
  ///     // Spawn entity with chunk.output mesh at chunk.position with chunk.scale
  /// }
  /// ```
  pub fn update(&mut self, viewer_pos: DVec3) -> PresentationBatch {
    // 1. Run refinement (updates self.leaves, records timing if metrics enabled)
    let output = self.refine(viewer_pos);

    if output.transition_groups.is_empty() {
      return PresentationBatch::default();
    }

    // 2. Process transitions through pipeline (parallel via rayon)
    let ready_chunks = process_transitions(
      self.id,
      &output.transition_groups,
      &self.sampler,
      self.leaves.as_set(),
      &self.config,
    );

    // 3. Record mesh timing metrics (aggregate from ready_chunks)
    #[cfg(feature = "metrics")]
    {
      // Sum all mesh timings from this batch
      let total_mesh_us: u64 = ready_chunks.iter().map(|c| c.timing_us).sum();
      if total_mesh_us > 0 {
        self.metrics.record_mesh_timing(total_mesh_us);
      }
      self.metrics.record_chunks_meshed(ready_chunks.len());
    }

    // 4. Build presentation batch
    self.build_presentation_batch(&output, ready_chunks)
  }

  /// Build presentation batch from refinement output and ready chunks.
  fn build_presentation_batch(
    &self,
    output: &RefinementOutput,
    ready_chunks: Vec<ReadyChunk>,
  ) -> PresentationBatch {
    let to_despawn = output
      .transition_groups
      .iter()
      .flat_map(|g| g.nodes_to_remove.iter().copied())
      .collect();

    let to_spawn = ready_chunks
      .into_iter()
      .map(|chunk| {
        let position = self.config.get_node_min(&chunk.node);
        let scale = self.config.get_voxel_size(chunk.node.lod);
        ChunkPresentation {
          node: chunk.node,
          position,
          scale,
          output: chunk.output,
          hint: chunk.hint,
        }
      })
      .collect();

    PresentationBatch { to_despawn, to_spawn }
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  use crate::constants::SAMPLE_SIZE_CB;
  use crate::octree::DAabb3;
  use crate::types::{MaterialId, SdfSample};

  /// Mock sampler for testing.
  struct MockSampler;

  impl VolumeSampler for MockSampler {
    fn sample_volume(
      &self,
      _grid_offset: [i64; 3],
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

  /// Integration test: Simulate the bug scenario where camera at far position
  /// causes infinite subdivision cascade at world boundaries.
  ///
  /// This test reproduces the production bug where:
  /// - World bounds: -50000 to +50000 (100k x 100k x 100k)
  /// - Camera at (-8814, 8569, -12520) - inside bounds but causing boundary issues
  /// - Multiple refinement cycles cause leaf explosion
  ///
  /// IMPORTANT: This uses the UNITY configuration where world_origin is at ZERO,
  /// which means nodes have negative coordinates in the negative world space.
  #[test]
  fn test_no_infinite_subdivision_at_world_boundary() {
    // Setup: Match UNITY production config (world_origin at ZERO)
    let world_half_extent = 50000.0;
    let world_bounds = DAabb3::from_center_half_extents(
      DVec3::ZERO,
      DVec3::new(world_half_extent, world_half_extent, world_half_extent),
    );

    let config = OctreeConfig {
      voxel_size: 0.25,
      world_origin: DVec3::ZERO, // Unity uses ZERO, not -half_extent!
      min_lod: 0,
      max_lod: 31,
      lod_exponent: 1.0,
      world_bounds: Some(world_bounds),
    };

    // Initialize world with computed initial leaves
    let mut world = VoxelWorld::new(config.clone(), MockSampler);

    // Compute initial leaves like the game does
    let initial_lod = config.suggest_initial_lod();
    for node in config.compute_initial_leaves(initial_lod) {
      world.leaves.insert(node);
    }

    let initial_leaf_count = world.leaves.len();
    eprintln!(
      "Initial: {} leaves at LOD {}",
      initial_leaf_count, initial_lod
    );

    // Problematic viewer position (from user report)
    let viewer_pos = DVec3::new(-8814.0, 8569.0, -12520.0);

    // Set budget similar to production
    world.budget = RefinementBudget {
      max_subdivisions: 32,
      max_collapses: 128,
      ..RefinementBudget::DEFAULT
    };

    // Run multiple refinement cycles (simulate continuous refinement)
    let mut total_subdivisions = 0;
    let mut total_neighbor_subdivisions = 0;
    let mut max_leaves = initial_leaf_count;

    for cycle in 0..50 {
      let output = world.refine(viewer_pos);

      total_subdivisions += output.stats.subdivisions_performed;
      total_neighbor_subdivisions += output.stats.neighbor_subdivisions_performed;

      let current_leaves = world.leaves.len();
      if current_leaves > max_leaves {
        max_leaves = current_leaves;
      }

      // Debug output for first few cycles
      if cycle < 5 || output.stats.total_transitions() > 0 {
        eprintln!(
          "Cycle {}: {} leaves, +{} subdivs, +{} neighbor subdivs, +{} collapses",
          cycle,
          current_leaves,
          output.stats.subdivisions_performed,
          output.stats.neighbor_subdivisions_performed,
          output.stats.collapses_performed
        );
      }

      // Early exit if converged
      if output.stats.total_transitions() == 0 {
        eprintln!("Converged at cycle {}", cycle);
        break;
      }
    }

    let final_leaves = world.leaves.len();
    eprintln!(
      "Final: {} leaves (max: {}, total subdivs: {}, neighbor subdivs: {})",
      final_leaves, max_leaves, total_subdivisions, total_neighbor_subdivisions
    );

    // Assertions: leaf count should stay bounded
    // In a healthy system, leaves should stabilize around a reasonable number
    // (dependent on viewer position and LOD thresholds)
    // With the bug, we'd see 50k+ leaves
    assert!(
      max_leaves < 5000,
      "BOUNDARY BUG: Leaf count exploded to {} (should be < 5000)",
      max_leaves
    );

    // Neighbor subdivisions should be minimal with the fix
    assert!(
      total_neighbor_subdivisions < 500,
      "BOUNDARY BUG: Too many neighbor subdivisions ({}, should be < 500)",
      total_neighbor_subdivisions
    );
  }

  /// Stress test: Run many refinement cycles with UNLIMITED budget to catch
  /// any explosive behavior.
  #[test]
  fn test_stress_unlimited_refinement_budget() {
    let world_half_extent = 50000.0;
    let world_bounds = DAabb3::from_center_half_extents(
      DVec3::ZERO,
      DVec3::new(world_half_extent, world_half_extent, world_half_extent),
    );

    let config = OctreeConfig {
      voxel_size: 0.25,
      world_origin: DVec3::ZERO,
      min_lod: 0,
      max_lod: 31,
      lod_exponent: 1.0,
      world_bounds: Some(world_bounds),
    };

    let mut world = VoxelWorld::new(config.clone(), MockSampler);

    // Initialize with computed leaves
    let initial_lod = config.suggest_initial_lod();
    for node in config.compute_initial_leaves(initial_lod) {
      world.leaves.insert(node);
    }

    // Use UNLIMITED budget to see full convergence behavior
    world.budget = crate::octree::RefinementBudget::UNLIMITED;

    // Problematic viewer position
    let viewer_pos = DVec3::new(-8814.0, 8569.0, -12520.0);

    eprintln!("Stress test starting with unlimited budget...");

    let mut total_subdivisions = 0;
    let mut total_neighbor_subdivisions = 0;

    for cycle in 0..100 {
      let output = world.refine(viewer_pos);

      total_subdivisions += output.stats.subdivisions_performed;
      total_neighbor_subdivisions += output.stats.neighbor_subdivisions_performed;

      let current_leaves = world.leaves.len();

      if cycle < 10 || output.stats.total_transitions() > 0 {
        eprintln!(
          "Stress cycle {}: {} leaves, subdivs: {}, neighbor: {}, collapses: {}",
          cycle,
          current_leaves,
          output.stats.subdivisions_performed,
          output.stats.neighbor_subdivisions_performed,
          output.stats.collapses_performed
        );
      }

      // Safety check: abort if leaves explode
      if current_leaves > 50000 {
        panic!(
          "BOUNDARY BUG DETECTED: Leaf count exploded to {} at cycle {}!",
          current_leaves, cycle
        );
      }

      if output.stats.total_transitions() == 0 {
        eprintln!("Stress test converged at cycle {}", cycle);
        break;
      }
    }

    let final_leaves = world.leaves.len();
    eprintln!(
      "Stress test final: {} leaves, total subdivs: {}, neighbor subdivs: {}",
      final_leaves, total_subdivisions, total_neighbor_subdivisions
    );

    assert!(
      final_leaves < 50000,
      "Stress test failed: {} leaves",
      final_leaves
    );
  }

  /// Test with viewer OUTSIDE world bounds - should not cause explosion
  #[test]
  fn test_viewer_outside_world_bounds_no_explosion() {
    let world_half_extent = 50000.0;
    let world_bounds = DAabb3::from_center_half_extents(
      DVec3::ZERO,
      DVec3::new(world_half_extent, world_half_extent, world_half_extent),
    );

    let config = OctreeConfig {
      voxel_size: 0.25,
      world_origin: DVec3::ZERO, // Match Unity config
      min_lod: 0,
      max_lod: 31,
      lod_exponent: 1.0,
      world_bounds: Some(world_bounds),
    };

    let mut world = VoxelWorld::new(config.clone(), MockSampler);

    // Initialize with computed leaves
    let initial_lod = config.suggest_initial_lod();
    for node in config.compute_initial_leaves(initial_lod) {
      world.leaves.insert(node);
    }

    let initial_leaf_count = world.leaves.len();

    // Viewer OUTSIDE world bounds
    let viewer_pos = DVec3::new(-60000.0, 60000.0, -70000.0);

    world.budget = RefinementBudget {
      max_subdivisions: 32,
      max_collapses: 128,
      ..RefinementBudget::DEFAULT
    };

    // Run refinement cycles
    let mut max_leaves = initial_leaf_count;
    for _ in 0..20 {
      let output = world.refine(viewer_pos);
      max_leaves = max_leaves.max(world.leaves.len());

      if output.stats.total_transitions() == 0 {
        break;
      }
    }

    eprintln!(
      "Viewer outside bounds: initial {} leaves, max {} leaves",
      initial_leaf_count, max_leaves
    );

    // Leaf count should stay bounded
    assert!(
      max_leaves < 1000,
      "Viewer outside bounds caused leaf explosion: {} leaves",
      max_leaves
    );
  }
}
