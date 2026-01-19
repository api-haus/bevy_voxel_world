//! Scene management for voxel_game
//!
//! Provides scene state machine and automatic cleanup.

pub mod noise_lod;

use bevy::prelude::*;

/// Application scene states
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Scene {
  /// Octree LOD terrain with noise (default scene)
  #[default]
  NoiseLod,
}

/// Marker component for entities that should be despawned on scene exit
#[derive(Component)]
pub struct SceneEntity;

/// Plugin for scene management
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
  fn build(&self, app: &mut App) {
    app
      .init_state::<Scene>()
      .add_systems(OnExit(Scene::NoiseLod), cleanup_scene);
  }
}

/// System to despawn all entities marked with SceneEntity
fn cleanup_scene(mut commands: Commands, query: Query<Entity, With<SceneEntity>>) {
  for entity in &query {
    commands.entity(entity).despawn();
  }
}
