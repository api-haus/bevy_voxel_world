//! Bevy resources for voxel LOD management.

use std::collections::HashMap;

use bevy::prelude::*;
use voxel_plugin::octree::{OctreeConfig, OctreeLeaves, OctreeNode};

/// Resource containing LOD-colored materials for visualization.
#[derive(Resource)]
pub struct LodMaterials {
  pub materials: Vec<Handle<StandardMaterial>>,
  pub neutral: Handle<StandardMaterial>,
}

impl LodMaterials {
  /// Get material for a given LOD level.
  pub fn get(&self, lod: i32, use_lod_colors: bool) -> Handle<StandardMaterial> {
    if use_lod_colors {
      let idx = (lod as usize).min(self.materials.len() - 1);
      self.materials[idx].clone()
    } else {
      self.neutral.clone()
    }
  }
}

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
