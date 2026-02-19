//! voxel_game - Bevy-based voxel meshing demo
//!
//! Octree-based LOD terrain with FastNoise2.
//!
//! Controls:
//! - Right-click + drag: Look around
//! - WASD: Move camera
//! - Q/E: Move up/down
//! - Shift: Sprint

mod fly_camera;
mod scenes;
mod shared;
pub mod triplanar_material;

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use fly_camera::CameraInputPlugin;
use scenes::{noise_lod::NoiseLodPlugin, ScenePlugin};
use shared::EguiPerfPlugin;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;

/// WASM entry point
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn wasm_main() {
  use wasm_bindgen_futures::JsFuture;

  console_error_panic_hook::set_once();

  let threads = std::thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(4);

  let promise = wasm_bindgen_rayon::init_thread_pool(threads);
  let _ = JsFuture::from(promise).await;

  run();
}

/// Native entry point
#[cfg(not(target_arch = "wasm32"))]
fn main() {
  run();
}

/// WASM requires a main function even when using wasm_bindgen(start)
#[cfg(target_arch = "wasm32")]
fn main() {}

fn run() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        title: "Voxel Game".into(),
        resolution: (1600, 900).into(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(FrameTimeDiagnosticsPlugin::default())
    .add_plugins(EguiPlugin::default())
    .add_plugins(EguiPerfPlugin)
    // Input handling
    .add_plugins(CameraInputPlugin)
    // Scene management
    .add_plugins(ScenePlugin)
    .add_plugins(NoiseLodPlugin)
    // Persistent camera (spawned once at startup, not tied to any scene)
    // Use PreStartup to ensure camera exists before OnEnter(Scene::NoiseLod) runs
    .add_systems(PreStartup, spawn_main_camera)
    .run();
}

// =============================================================================
// Main Camera (persists across all scenes)
// =============================================================================

/// Marker for the main camera (not scene-specific, persists across scenes)
#[derive(Component)]
pub struct MainCamera;

/// Spawn the main camera at startup (before any scene)
fn spawn_main_camera(mut commands: Commands, query: Query<Entity, With<MainCamera>>) {
  // Only spawn if not already spawned (scene's OnEnter may have run first due to Bevy schedule order)
  if query.iter().next().is_none() {
    info!("[Main] Spawning MainCamera entity");
    commands.spawn((Camera3d::default(), MainCamera));
  } else {
    info!("[Main] MainCamera already exists, skipping spawn");
  }
}
