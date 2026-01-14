//! Metaballs Demo Scene
//!
//! Animated metaballs with per-frame Surface Nets remeshing.
//! Uses 2x2x2 chunk octet (8 chunks) for larger coherent scenes.
//! Demonstrates different normal computation modes with egui controls.

use std::collections::VecDeque;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use voxel_plugin::{
  coord_to_index, sdf_conversion, MaterialId, MeshConfig, MeshingStage, NormalMode, SdfSample,
  SAMPLE_SIZE, SAMPLE_SIZE_CB,
};

use super::{Scene, SceneEntity};
use crate::shared::PerfEntries;

/// Plugin for the metaballs scene
pub struct MetaballsPlugin;

impl Plugin for MetaballsPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<MeshingStats>()
      .init_resource::<TaskQueueState>()
      .init_resource::<NormalSettings>()
      .add_systems(OnEnter(Scene::Metaballs), setup)
      .add_systems(OnExit(Scene::Metaballs), cleanup_camera)
      .add_systems(
        Update,
        (
          orbit_camera,
          enqueue_mesh_requests,
          process_mesh_completions,
          update_perf_entries,
        )
          .chain()
          .run_if(in_state(Scene::Metaballs)),
      )
      .add_systems(
        EguiPrimaryContextPass,
        render_normal_settings_ui.run_if(in_state(Scene::Metaballs)),
      );
  }
}

/// Remove OrbitCamera component from main camera when leaving scene
fn cleanup_camera(mut commands: Commands, camera_query: Query<Entity, With<crate::MainCamera>>) {
  if let Ok(camera_entity) = camera_query.single() {
    commands.entity(camera_entity).remove::<OrbitCamera>();
  }
}

// =============================================================================
// Normal Settings
// =============================================================================

/// Settings for normal computation mode, controllable via egui
#[derive(Resource)]
struct NormalSettings {
  /// Current normal mode selection
  mode: NormalModeSelection,
  /// Blend distance for blended mode (in cells)
  blend_distance: f32,
  /// Force remesh on next frame
  force_remesh: bool,
  /// Pause animation
  paused: bool,
  /// Animation time (for manual control when paused)
  animation_time: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum NormalModeSelection {
  Gradient,
  Geometry,
  #[default]
  Blended,
}

impl Default for NormalSettings {
  fn default() -> Self {
    Self {
      mode: NormalModeSelection::default(),
      blend_distance: 2.0,
      force_remesh: false,
      paused: false,
      animation_time: 0.0,
    }
  }
}

impl NormalSettings {
  fn to_normal_mode(&self) -> NormalMode {
    match self.mode {
      NormalModeSelection::Gradient => NormalMode::Gradient,
      NormalModeSelection::Geometry => NormalMode::Geometry,
      NormalModeSelection::Blended => NormalMode::Blended {
        blend_distance: self.blend_distance,
      },
    }
  }
}

// =============================================================================
// Chunk System
// =============================================================================

/// Number of chunks per axis (2x2x2 = 8 chunks total)
const CHUNKS_PER_AXIS: usize = 2;

/// Total number of chunks
const TOTAL_CHUNKS: usize = CHUNKS_PER_AXIS * CHUNKS_PER_AXIS * CHUNKS_PER_AXIS;

/// Voxel size (world units per voxel)
const VOXEL_SIZE: f32 = 1.0;

/// Interior cells per chunk (28 cells produce geometry)
const INTERIOR_CELLS: usize = 28;

/// World size of one chunk's interior
const CHUNK_WORLD_SIZE: f32 = INTERIOR_CELLS as f32 * VOXEL_SIZE;

/// Marker for chunk mesh entities
#[derive(Component)]
struct ChunkMesh {
  /// Chunk index (0-7 for 2x2x2)
  index: usize,
  /// Chunk position in chunk coordinates
  #[allow(dead_code)]
  chunk_pos: IVec3,
}

/// Get chunk world offset from chunk coordinates
fn chunk_world_offset(chunk_pos: IVec3) -> Vec3 {
  Vec3::new(
    chunk_pos.x as f32 * CHUNK_WORLD_SIZE,
    chunk_pos.y as f32 * CHUNK_WORLD_SIZE,
    chunk_pos.z as f32 * CHUNK_WORLD_SIZE,
  )
}

// =============================================================================
// Camera
// =============================================================================

/// Marker component for the orbit camera
#[derive(Component)]
struct OrbitCamera {
  focus: Vec3,
  radius: f32,
  pitch: f32,
  yaw: f32,
}

impl Default for OrbitCamera {
  fn default() -> Self {
    // Center the camera on the middle of the 2x2x2 chunk array
    let center = CHUNK_WORLD_SIZE; // Center of 2x2x2 chunks
    Self {
      focus: Vec3::new(center, center, center),
      radius: 100.0,
      pitch: -0.3,
      yaw: 0.5,
    }
  }
}

// =============================================================================
// Statistics
// =============================================================================

/// Number of samples to keep for rolling statistics
const STATS_WINDOW_SIZE: usize = 120;

/// Meshing performance statistics with rolling window
#[derive(Resource)]
struct MeshingStats {
  mesh_times: VecDeque<u64>,
  last_mesh_time_us: u64,
  last_vertex_count: usize,
  last_triangle_count: usize,
  pending_count: usize,
  chunks_meshed: usize,
}

impl Default for MeshingStats {
  fn default() -> Self {
    Self {
      mesh_times: VecDeque::with_capacity(STATS_WINDOW_SIZE),
      last_mesh_time_us: 0,
      last_vertex_count: 0,
      last_triangle_count: 0,
      pending_count: 0,
      chunks_meshed: 0,
    }
  }
}

impl MeshingStats {
  fn record(&mut self, time_us: u64) {
    if self.mesh_times.len() >= STATS_WINDOW_SIZE {
      self.mesh_times.pop_front();
    }
    self.mesh_times.push_back(time_us);
    self.last_mesh_time_us = time_us;
  }

  fn min(&self) -> u64 {
    self.mesh_times.iter().copied().min().unwrap_or(0)
  }

  fn max(&self) -> u64 {
    self.mesh_times.iter().copied().max().unwrap_or(0)
  }

  fn avg(&self) -> u64 {
    if self.mesh_times.is_empty() {
      return 0;
    }
    self.mesh_times.iter().sum::<u64>() / self.mesh_times.len() as u64
  }

  fn median(&self) -> u64 {
    if self.mesh_times.is_empty() {
      return 0;
    }
    let mut sorted: Vec<u64> = self.mesh_times.iter().copied().collect();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
      (sorted[mid - 1] + sorted[mid]) / 2
    } else {
      sorted[mid]
    }
  }
}

/// Task queue state resource
#[derive(Resource, Default)]
struct TaskQueueState {
  stage: MeshingStage,
}

// =============================================================================
// Metaballs SDF
// =============================================================================

/// Metaball configuration
struct Metaball {
  center: Vec3,
  radius: f32,
}

/// Generate metaballs SDF for a specific chunk
fn generate_metaballs_sdf_for_chunk(
  time: f32,
  chunk_pos: IVec3,
  volume: &mut [SdfSample; SAMPLE_SIZE_CB],
  materials: &mut [MaterialId; SAMPLE_SIZE_CB],
) {
  // Scene center is at the middle of all 8 chunks
  let scene_center = Vec3::splat(CHUNK_WORLD_SIZE); // Center of 2x2x2 arrangement

  // Chunk world offset
  let chunk_offset = chunk_world_offset(chunk_pos);

  // Larger metaballs that span multiple chunks
  let metaballs = [
    Metaball {
      center: scene_center
        + Vec3::new(
          (time * 0.8).sin() * 20.0,
          (time * 0.6).cos() * 15.0,
          (time * 0.7).sin() * 18.0,
        ),
      radius: 12.0,
    },
    Metaball {
      center: scene_center
        + Vec3::new(
          (time * 0.6 + 2.0).cos() * 18.0,
          (time * 0.9).sin() * 12.0,
          (time * 0.5 + 1.0).cos() * 20.0,
        ),
      radius: 10.0,
    },
    Metaball {
      center: scene_center
        + Vec3::new(
          (time * 1.0 + 4.0).sin() * 15.0,
          (time * 0.4 + 3.0).cos() * 18.0,
          (time * 0.8).sin() * 12.0,
        ),
      radius: 9.0,
    },
    Metaball {
      center: scene_center
        + Vec3::new(
          (time * 0.5 + 1.5).cos() * 12.0,
          (time * 1.1 + 2.5).sin() * 15.0,
          (time * 0.6 + 0.5).cos() * 16.0,
        ),
      radius: 8.0,
    },
    // Additional metaballs for more interesting shapes
    Metaball {
      center: scene_center
        + Vec3::new(
          (time * 0.9 + 3.0).sin() * 22.0,
          (time * 0.7 + 1.0).cos() * 10.0,
          (time * 1.2).sin() * 14.0,
        ),
      radius: 7.0,
    },
    Metaball {
      center: scene_center
        + Vec3::new(
          (time * 0.4 + 5.0).cos() * 16.0,
          (time * 1.0 + 4.0).sin() * 20.0,
          (time * 0.3 + 2.0).cos() * 18.0,
        ),
      radius: 8.5,
    },
  ];

  for x in 0..SAMPLE_SIZE {
    for y in 0..SAMPLE_SIZE {
      for z in 0..SAMPLE_SIZE {
        // Convert sample position to world position
        // Account for apron: sample 0 is at -1, sample 1 is at 0, etc.
        let local_pos = Vec3::new(
          (x as f32 - 1.0) * VOXEL_SIZE,
          (y as f32 - 1.0) * VOXEL_SIZE,
          (z as f32 - 1.0) * VOXEL_SIZE,
        );
        let world_pos = chunk_offset + local_pos;

        let mut sdf = f32::MAX;
        let mut closest_ball = 0usize;

        for (i, ball) in metaballs.iter().enumerate() {
          let d = (world_pos - ball.center).length() - ball.radius;
          // Smooth union with k=4 for blobby metaball effect
          let k = 4.0;
          let h = (0.5 + 0.5 * (sdf - d) / k).clamp(0.0, 1.0);
          let new_sdf = sdf * (1.0 - h) + d * h - k * h * (1.0 - h);

          if d < sdf {
            closest_ball = i;
          }
          sdf = new_sdf;
        }

        let idx = coord_to_index(x, y, z);
        volume[idx] = sdf_conversion::to_storage(sdf);
        materials[idx] = (closest_ball % 4) as u8;
      }
    }
  }
}

// =============================================================================
// Setup
// =============================================================================

fn setup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  camera_query: Query<Entity, With<crate::MainCamera>>,
) {
  // Configure existing main camera with OrbitCamera
  let orbit = OrbitCamera::default();
  let transform = orbit_transform(&orbit);
  if let Ok(camera_entity) = camera_query.single() {
    commands.entity(camera_entity).insert((transform, orbit));
  }

  // Directional light
  commands.spawn((
    DirectionalLight {
      illuminance: 15000.0,
      shadows_enabled: true,
      ..default()
    },
    Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
    SceneEntity,
  ));

  // Ambient light
  commands.insert_resource(AmbientLight {
    color: Color::WHITE,
    brightness: 200.0,
    ..default()
  });

  // Create 8 chunk mesh entities (2x2x2)
  let chunk_material = materials.add(StandardMaterial {
    base_color: Color::srgb(0.3, 0.5, 0.8),
    perceptual_roughness: 0.5,
    metallic: 0.1,
    cull_mode: None,
    ..default()
  });

  let mut chunk_index = 0;
  for cx in 0..CHUNKS_PER_AXIS as i32 {
    for cy in 0..CHUNKS_PER_AXIS as i32 {
      for cz in 0..CHUNKS_PER_AXIS as i32 {
        let chunk_pos = IVec3::new(cx, cy, cz);
        let world_offset = chunk_world_offset(chunk_pos);

        let empty_mesh = Mesh::new(PrimitiveTopology::TriangleList, default());

        commands.spawn((
          Mesh3d(meshes.add(empty_mesh)),
          MeshMaterial3d(chunk_material.clone()),
          Transform::from_translation(world_offset),
          ChunkMesh {
            index: chunk_index,
            chunk_pos,
          },
          SceneEntity,
        ));

        chunk_index += 1;
      }
    }
  }

  // Ground plane
  let ground_center = CHUNK_WORLD_SIZE; // Center of the chunk array
  commands.spawn((
    Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(100.0)))),
    MeshMaterial3d(materials.add(StandardMaterial {
      base_color: Color::srgb(0.15, 0.15, 0.15),
      perceptual_roughness: 0.9,
      metallic: 0.0,
      ..default()
    })),
    Transform::from_translation(Vec3::new(ground_center, -2.0, ground_center)),
    SceneEntity,
  ));

  info!(
    "[Metaballs] Initialized {} chunks in 2x2x2 arrangement",
    TOTAL_CHUNKS
  );
}

// =============================================================================
// Normal Settings UI
// =============================================================================

fn render_normal_settings_ui(mut contexts: EguiContexts, mut settings: ResMut<NormalSettings>) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };
  egui::Window::new("Normal Settings")
    .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
    .resizable(false)
    .show(ctx, |ui| {
      ui.set_min_width(200.0);

      // Normal mode selection
      ui.label("Normal Mode:");
      let mut changed = false;

      changed |= ui
        .radio_value(
          &mut settings.mode,
          NormalModeSelection::Gradient,
          "Gradient (fast)",
        )
        .changed();
      changed |= ui
        .radio_value(
          &mut settings.mode,
          NormalModeSelection::Geometry,
          "Geometry (accurate)",
        )
        .changed();
      changed |= ui
        .radio_value(
          &mut settings.mode,
          NormalModeSelection::Blended,
          "Blended (best)",
        )
        .changed();

      // Blended mode settings
      if settings.mode == NormalModeSelection::Blended {
        ui.separator();
        ui.label("Blend Settings:");

        ui.horizontal(|ui| {
          ui.label("Distance:");
          changed |= ui
            .add(egui::Slider::new(&mut settings.blend_distance, 1.0..=10.0))
            .changed();
        });
      }

      ui.separator();

      // Animation controls
      ui.checkbox(&mut settings.paused, "Pause animation");

      if settings.paused {
        ui.horizontal(|ui| {
          ui.label("Time:");
          if ui
            .add(egui::Slider::new(&mut settings.animation_time, 0.0..=20.0))
            .changed()
          {
            changed = true;
          }
        });
      }

      // Force remesh button
      if ui.button("Force Remesh").clicked() {
        changed = true;
      }

      if changed {
        settings.force_remesh = true;
      }

      ui.separator();

      // Mode description
      let desc = match settings.mode {
        NormalModeSelection::Gradient => {
          "Uses 8 corner samples per cell.\nFast but may have edge artifacts."
        }
        NormalModeSelection::Geometry => {
          "Computed from triangle faces.\nAccurate interior, gaps at chunk edges."
        }
        NormalModeSelection::Blended => "Geometry inside, gradient at edges.\nBest visual quality.",
      };
      ui.label(egui::RichText::new(desc).weak().small());
    });
}

// =============================================================================
// Mesh Generation
// =============================================================================

/// Enqueue mesh requests into the task queue
fn enqueue_mesh_requests(
  time: Res<Time>,
  mut queue: ResMut<TaskQueueState>,
  mut settings: ResMut<NormalSettings>,
) {
  // Only enqueue if we don't have pending work
  if queue.stage.pending_count() > 0 || queue.stage.completed_count() > 0 {
    return;
  }

  // Update animation time
  let t = if settings.paused {
    settings.animation_time
  } else {
    let elapsed = time.elapsed_secs();
    settings.animation_time = elapsed;
    elapsed
  };

  // Clear force_remesh flag
  settings.force_remesh = false;

  // Build mesh config with current normal mode
  let config = MeshConfig::new().with_normal_mode(settings.to_normal_mode());

  // Enqueue all 8 chunks
  for cx in 0..CHUNKS_PER_AXIS as i32 {
    for cy in 0..CHUNKS_PER_AXIS as i32 {
      for cz in 0..CHUNKS_PER_AXIS as i32 {
        let chunk_pos = IVec3::new(cx, cy, cz);

        // Generate SDF for this chunk
        let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
        let mut materials = Box::new([0u8; SAMPLE_SIZE_CB]);
        generate_metaballs_sdf_for_chunk(t, chunk_pos, &mut volume, &mut materials);

        // Enqueue the request
        queue.stage.enqueue(volume, materials, config.clone());
      }
    }
  }
}

/// Process completed meshes from the task queue
fn process_mesh_completions(
  mut queue: ResMut<TaskQueueState>,
  mut stats: ResMut<MeshingStats>,
  mut meshes: ResMut<Assets<Mesh>>,
  chunk_query: Query<(&Mesh3d, &ChunkMesh)>,
) {
  // Tick the stage to process pending work
  queue.stage.tick();

  // Update pending count for display
  stats.pending_count = queue.stage.pending_count();

  // Drain completions
  let completions: Vec<_> = queue.stage.drain_completions().into_iter().collect();
  if completions.is_empty() {
    return;
  }

  stats.chunks_meshed = completions.len();

  // Apply completions to chunk meshes
  let mut total_vertices = 0;
  let mut total_triangles = 0;

  // Match completions to chunks by order (they're enqueued in order)
  for (completion_idx, completion) in completions.into_iter().enumerate() {
    // Record raw meshing time per chunk
    stats.record(completion.mesh_time_us);
    total_vertices += completion.output.vertices.len();
    total_triangles += completion.output.indices.len() / 3;

    // Find the chunk mesh with matching index
    for (mesh_handle, chunk) in chunk_query.iter() {
      if chunk.index == completion_idx {
        if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
          let positions: Vec<[f32; 3]> = completion
            .output
            .vertices
            .iter()
            .map(|v| v.position)
            .collect();
          let normals: Vec<[f32; 3]> = completion
            .output
            .vertices
            .iter()
            .map(|v| v.normal)
            .collect();

          mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            VertexAttributeValues::Float32x3(positions),
          );
          mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            VertexAttributeValues::Float32x3(normals),
          );
          mesh.insert_indices(Indices::U32(completion.output.indices));
        }
        break;
      }
    }
  }

  stats.last_vertex_count = total_vertices;
  stats.last_triangle_count = total_triangles;
}

// =============================================================================
// Performance UI
// =============================================================================

/// Update performance entries for the egui_perf window
fn update_perf_entries(
  stats: Res<MeshingStats>,
  settings: Res<NormalSettings>,
  mut entries: ResMut<PerfEntries>,
) {
  entries.clear();

  let thread_count = rayon::current_num_threads();

  entries.add_time_us("Mesh avg", stats.avg());
  entries.add_time_us("Mesh min", stats.min());
  entries.add_time_us("Mesh max", stats.max());
  entries.add_time_us("Mesh med", stats.median());
  entries.add_count("Vertices", stats.last_vertex_count);
  entries.add_count("Triangles", stats.last_triangle_count);
  entries.add("Chunks", format!("{}", TOTAL_CHUNKS));
  entries.add("Threads", format!("{}", thread_count));

  // Show current normal mode
  let mode_str = match settings.mode {
    NormalModeSelection::Gradient => "Gradient",
    NormalModeSelection::Geometry => "Geometry",
    NormalModeSelection::Blended => "Blended",
  };
  entries.add("Normal", mode_str.to_string());
}

// =============================================================================
// Camera
// =============================================================================

fn orbit_transform(orbit: &OrbitCamera) -> Transform {
  let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
  let position = orbit.focus + rotation * Vec3::new(0.0, 0.0, orbit.radius);
  Transform::from_translation(position).looking_at(orbit.focus, Vec3::Y)
}

fn orbit_camera(
  mouse_button: Res<ButtonInput<MouseButton>>,
  mouse_motion: Res<AccumulatedMouseMotion>,
  mouse_scroll: Res<AccumulatedMouseScroll>,
  mut query: Query<(&mut OrbitCamera, &mut Transform)>,
) {
  let Ok((mut orbit, mut transform)) = query.single_mut() else {
    return;
  };

  if mouse_button.pressed(MouseButton::Right) {
    let delta = mouse_motion.delta;
    orbit.yaw -= delta.x * 0.005;
    orbit.pitch -= delta.y * 0.005;
    orbit.pitch = orbit.pitch.clamp(-1.5, 1.5);
  }

  let scroll = mouse_scroll.delta.y;
  if scroll != 0.0 {
    orbit.radius -= scroll * orbit.radius * 0.1;
    orbit.radius = orbit.radius.clamp(5.0, 200.0);
  }

  *transform = orbit_transform(&orbit);
}
