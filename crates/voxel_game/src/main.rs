//! voxel_game - Bevy-based voxel meshing demo
//!
//! Octree-based LOD terrain with FastNoise2.
//!
//! Controls:
//! - 1: Switch to Noise LOD scene
//! - Esc: Return to menu

mod scenes;
mod shared;

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use scenes::{noise_lod::NoiseLodPlugin, sdf_test::SdfTestPlugin, Scene, ScenePlugin};
use shared::EguiPerfPlugin;
use voxel_bevy::CameraInputPlugin;
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
        title: "Voxel Game - Demo Scenes".into(),
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
    .add_plugins(SdfTestPlugin)
    // Persistent camera (spawned once at startup, not tied to any scene)
    .add_systems(Startup, spawn_main_camera)
    // Menu scene
    .add_systems(OnEnter(Scene::Menu), setup_menu)
    .add_systems(
      EguiPrimaryContextPass,
      menu_ui.run_if(in_state(Scene::Menu)),
    )
    // Global scene switching
    .add_systems(Update, scene_keyboard_shortcuts)
    .run();
}

// =============================================================================
// Main Camera (persists across all scenes)
// =============================================================================

/// Marker for the main camera (not scene-specific, persists across scenes)
#[derive(Component)]
pub struct MainCamera;

/// Spawn the main camera at startup (before any scene)
fn spawn_main_camera(mut commands: Commands) {
  commands.spawn((Camera3d::default(), MainCamera));
}

// =============================================================================
// Menu Scene
// =============================================================================

/// Setup the menu scene
fn setup_menu(mut commands: Commands) {
  // Dark background
  commands.insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)));

  info!("[Menu] Scene loaded");
}

/// Menu UI with scene selection
fn menu_ui(mut contexts: EguiContexts, mut next_state: ResMut<NextState<Scene>>) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  egui::CentralPanel::default().show(ctx, |ui| {
    ui.vertical_centered(|ui| {
      ui.add_space(100.0);

      ui.heading(
        egui::RichText::new("Voxel Game")
          .size(48.0)
          .color(egui::Color32::WHITE),
      );

      ui.add_space(20.0);

      ui.label(
        egui::RichText::new("Select a demo scene")
          .size(18.0)
          .color(egui::Color32::GRAY),
      );

      ui.add_space(40.0);

      let button_size = egui::vec2(300.0, 60.0);

      if ui
        .add_sized(
          button_size,
          egui::Button::new(egui::RichText::new("1. Noise LOD Demo").size(20.0)),
        )
        .clicked()
      {
        next_state.set(Scene::NoiseLod);
      }

      ui.add_space(5.0);
      ui.label(
        egui::RichText::new("Octree terrain with FastNoise2 generation")
          .size(14.0)
          .color(egui::Color32::DARK_GRAY),
      );

      ui.add_space(20.0);

      if ui
        .add_sized(
          button_size,
          egui::Button::new(egui::RichText::new("2. SDF Test Scene").size(20.0)),
        )
        .clicked()
      {
        next_state.set(Scene::SdfTest);
      }

      ui.add_space(5.0);
      ui.label(
        egui::RichText::new("Simple shapes (plane, sphere, box) for testing chunk tiling")
          .size(14.0)
          .color(egui::Color32::DARK_GRAY),
      );

      ui.add_space(60.0);

      ui.label(
        egui::RichText::new("Press 1 or 2 to start | Esc to return to menu")
          .size(14.0)
          .color(egui::Color32::from_rgb(100, 100, 100)),
      );
    });
  });
}

// =============================================================================
// Scene Switching
// =============================================================================

/// Keyboard shortcuts for scene switching
fn scene_keyboard_shortcuts(
  keyboard: Res<ButtonInput<KeyCode>>,
  current_state: Res<State<Scene>>,
  mut next_state: ResMut<NextState<Scene>>,
) {
  if keyboard.just_pressed(KeyCode::Digit1) && *current_state.get() != Scene::NoiseLod {
    info!("Switching to Noise LOD scene");
    next_state.set(Scene::NoiseLod);
  }

  if keyboard.just_pressed(KeyCode::Digit2) && *current_state.get() != Scene::SdfTest {
    info!("Switching to SDF Test scene");
    next_state.set(Scene::SdfTest);
  }

  if keyboard.just_pressed(KeyCode::Escape) && *current_state.get() != Scene::Menu {
    info!("Returning to menu");
    next_state.set(Scene::Menu);
  }
}
