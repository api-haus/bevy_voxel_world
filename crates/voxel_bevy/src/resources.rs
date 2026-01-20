//! Bevy resources for voxel LOD management.

use std::collections::HashMap;

use bevy::prelude::*;
use voxel_plugin::metrics::WorldMetrics;
use voxel_plugin::octree::{OctreeConfig, OctreeLeaves, OctreeNode};


/// Resource containing octree LOD state.
#[derive(Resource)]
pub struct OctreeLodState {
  /// Set of current leaf nodes.
  pub leaves: OctreeLeaves,
  /// Octree configuration for coordinate mapping.
  pub config: OctreeConfig,
}

/// Resource mapping octree nodes to their mesh entities.
#[derive(Resource, Default)]
pub struct ChunkEntityMap {
  pub map: HashMap<OctreeNode, Entity>,
}

impl ChunkEntityMap {
  pub fn insert(&mut self, node: OctreeNode, entity: Entity) {
    self.map.insert(node, entity);
  }

  pub fn remove(&mut self, node: &OctreeNode) -> Option<Entity> {
    self.map.remove(node)
  }

  pub fn get(&self, node: &OctreeNode) -> Option<Entity> {
    self.map.get(node).copied()
  }
}

/// Resource wrapping WorldMetrics for Bevy integration.
///
/// Provides per-frame statistics from the voxel pipeline including:
/// - LOD distribution
/// - Mesh timing histograms
/// - Memory usage
#[derive(Resource)]
pub struct VoxelMetricsResource {
  /// Current world metrics snapshot.
  pub current: WorldMetrics,
  /// Whether metrics collection is enabled.
  pub enabled: bool,
}

impl Default for VoxelMetricsResource {
  fn default() -> Self {
    Self {
      current: WorldMetrics::new(),
      enabled: true, // Enable by default
    }
  }
}

impl VoxelMetricsResource {
  /// Create a new metrics resource.
  pub fn new() -> Self {
    Self {
      current: WorldMetrics::new(),
      enabled: true,
    }
  }

  /// Record refinement stats from async pipeline.
  pub fn record_refinement_stats(&mut self, refine_us: u64, mesh_us: u64) {
    if self.enabled {
      self.current.record_refine_timing(refine_us);
      self.current.record_mesh_timing(mesh_us);
    }
  }

  /// Record chunk spawn.
  pub fn record_chunk_spawn(&mut self, lod: i32, vertex_count: u32, index_count: u32) {
    if self.enabled {
      self.current.record_chunk(lod, vertex_count, index_count);
    }
  }

  /// Record chunk despawn.
  pub fn record_chunk_despawn(&mut self, lod: i32, vertex_count: u32, index_count: u32) {
    if self.enabled {
      self.current.remove_chunk(lod, vertex_count, index_count);
    }
  }

  /// Reset all metrics.
  pub fn reset(&mut self) {
    self.current.reset();
  }

  /// Toggle metrics collection.
  pub fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
    voxel_plugin::metrics::COLLECT_METRICS.store(enabled, std::sync::atomic::Ordering::Relaxed);
  }
}
