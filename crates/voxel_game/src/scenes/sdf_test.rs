//! SDF Test Scene
//!
//! Tests chunk tiling coherency with simple deterministic SDF shapes.
//! Use this to verify meshing pipeline works correctly without noise complexity.
//!
//! Controls:
//! - Right-click + drag: Look around
//! - WASD: Move camera
//! - Space/Shift: Move up/down
//! - Tab: Cycle through SDF shapes
//! - +/-: Adjust voxel size

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use rayon::prelude::*;
use voxel_bevy::fly_camera::{update_fly_camera, FlyCamera};
use voxel_bevy::input::{fly_camera_input_bundle, CameraInputContext};
use voxel_bevy::resources::LodMaterials;
use voxel_plugin::noise::is_homogeneous;
use voxel_plugin::octree::OctreeConfig;
use voxel_plugin::pipeline::{sample_volume_for_node, VolumeSampler};
use voxel_plugin::sdf_samplers::{BoxSampler, GroundPlaneSampler, SphereSampler, TiltedPlaneSampler};
use voxel_plugin::surface_nets;
use voxel_plugin::types::MeshConfig;
use voxel_plugin::OctreeNode;

use super::{Scene, SceneEntity};
use crate::MainCamera;

/// Re-use DVec3 from bevy
type DVec3 = bevy::math::DVec3;

/// Plugin for the SDF test scene
pub struct SdfTestPlugin;

impl Plugin for SdfTestPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<SdfTestSettings>()
      .add_message::<RebuildMeshesEvent>()
      .add_systems(OnEnter(Scene::SdfTest), setup)
      .add_systems(OnExit(Scene::SdfTest), cleanup)
      .add_systems(
        Update,
        (
          update_fly_camera.run_if(in_state(Scene::SdfTest)),
          handle_input.run_if(in_state(Scene::SdfTest)),
          rebuild_meshes.run_if(in_state(Scene::SdfTest)),
        ),
      )
      .add_systems(
        EguiPrimaryContextPass,
        ui_panel.run_if(in_state(Scene::SdfTest)),
      );
  }
}

/// SDF shape selection
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SdfShape {
  #[default]
  TiltedPlane,
  Sphere,
  GroundPlane,
  Box,
}

impl SdfShape {
  fn next(self) -> Self {
    match self {
      Self::TiltedPlane => Self::Sphere,
      Self::Sphere => Self::GroundPlane,
      Self::GroundPlane => Self::Box,
      Self::Box => Self::TiltedPlane,
    }
  }

  fn name(self) -> &'static str {
    match self {
      Self::TiltedPlane => "Tilted Plane (45°)",
      Self::Sphere => "Sphere",
      Self::GroundPlane => "Ground Plane",
      Self::Box => "Box",
    }
  }

  fn create_sampler(self) -> Box<dyn VolumeSampler> {
    match self {
      Self::TiltedPlane => Box::new(TiltedPlaneSampler::default()),
      Self::Sphere => Box::new(SphereSampler::new(40.0)),
      Self::GroundPlane => Box::new(GroundPlaneSampler::new(0.0)),
      Self::Box => Box::new(BoxSampler::new([30.0, 20.0, 25.0])),
    }
  }
}

/// Settings for the SDF test scene
#[derive(Resource)]
pub struct SdfTestSettings {
  pub shape: SdfShape,
  pub voxel_size: f32,
  pub grid_extent: i32,
  pub lod: i32,
  pub color_chunks: bool,
}

impl Default for SdfTestSettings {
  fn default() -> Self {
    Self {
      shape: SdfShape::TiltedPlane,
      voxel_size: 1.0,
      grid_extent: 2, // -2..=2 = 5x5x5 = 125 chunks
      lod: 0,
      color_chunks: true,
    }
  }
}

/// Event to trigger mesh rebuild
#[derive(Message)]
pub struct RebuildMeshesEvent;

/// Marker for chunk meshes in this scene
#[derive(Component)]
pub struct SdfTestChunk {
  pub node: OctreeNode,
}

// =============================================================================
// Setup
// =============================================================================

fn setup(
  mut commands: Commands,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut rebuild_events: MessageWriter<RebuildMeshesEvent>,
  camera_query: Query<Entity, With<MainCamera>>,
) {
  info!("[SdfTest] Setting up scene...");

  // Dark background
  commands.insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)));

  // Create per-LOD materials
  let lod_materials = create_lod_materials(&mut materials);
  commands.insert_resource(lod_materials);

  // Setup camera with input handling
  if let Ok(camera_entity) = camera_query.single() {
    commands.entity(camera_entity).insert((
      fly_camera_input_bundle(FlyCamera {
        speed: 50.0,
        mouse_sensitivity: 0.003,
        ..default()
      }),
      Transform::from_xyz(0.0, 50.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
  }

  // Add lights
  commands.spawn((
    DirectionalLight {
      illuminance: 10000.0,
      shadows_enabled: true,
      ..default()
    },
    Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
    SceneEntity,
  ));

  // Ambient light
  commands.insert_resource(GlobalAmbientLight {
    color: Color::WHITE,
    brightness: 200.0,
    ..default()
  });

  // Trigger initial mesh generation
  rebuild_events.write(RebuildMeshesEvent);

  info!("[SdfTest] Scene setup complete");
}

fn cleanup(mut commands: Commands, camera_query: Query<Entity, With<MainCamera>>) {
	if let Ok(camera_entity) = camera_query.single() {
		commands
			.entity(camera_entity)
			.remove::<FlyCamera>()
			.remove::<CameraInputContext>();
	}
}

// =============================================================================
// Input handling
// =============================================================================

fn handle_input(
  keyboard: Res<ButtonInput<KeyCode>>,
  mut settings: ResMut<SdfTestSettings>,
  mut rebuild_events: MessageWriter<RebuildMeshesEvent>,
) {
  let mut needs_rebuild = false;

  // Tab: cycle shapes
  if keyboard.just_pressed(KeyCode::Tab) {
    settings.shape = settings.shape.next();
    info!("[SdfTest] Shape: {}", settings.shape.name());
    needs_rebuild = true;
  }

  // +/-: adjust voxel size
  if keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd) {
    settings.voxel_size *= 2.0;
    settings.voxel_size = settings.voxel_size.min(8.0);
    info!("[SdfTest] Voxel size: {}", settings.voxel_size);
    needs_rebuild = true;
  }
  if keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract) {
    settings.voxel_size /= 2.0;
    settings.voxel_size = settings.voxel_size.max(0.125);
    info!("[SdfTest] Voxel size: {}", settings.voxel_size);
    needs_rebuild = true;
  }

  // C: toggle chunk colors
  if keyboard.just_pressed(KeyCode::KeyC) {
    settings.color_chunks = !settings.color_chunks;
    info!("[SdfTest] Chunk colors: {}", settings.color_chunks);
    needs_rebuild = true;
  }

  if needs_rebuild {
    rebuild_events.write(RebuildMeshesEvent);
  }
}

// =============================================================================
// Mesh generation
// =============================================================================

fn rebuild_meshes(
  mut commands: Commands,
  mut events: MessageReader<RebuildMeshesEvent>,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
  settings: Res<SdfTestSettings>,
  lod_materials: Res<LodMaterials>,
  existing_chunks: Query<Entity, With<SdfTestChunk>>,
) {
  if events.read().next().is_none() {
    return;
  }

  // Despawn existing chunks
  for entity in &existing_chunks {
    commands.entity(entity).despawn();
  }

  let sampler = settings.shape.create_sampler();
  let config = OctreeConfig {
    voxel_size: settings.voxel_size as f64,
    world_origin: DVec3::ZERO,
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
  };

  // Generate nodes in a grid
  let extent = settings.grid_extent;
  let lod = settings.lod;

  let nodes: Vec<OctreeNode> = (-extent..=extent)
    .flat_map(|x| {
      (-extent..=extent)
        .flat_map(move |y| (-extent..=extent).map(move |z| OctreeNode::new(x, y, z, lod)))
    })
    .collect();

  info!(
    "[SdfTest] Generating {} chunks at LOD {} with voxel_size={}",
    nodes.len(),
    lod,
    settings.voxel_size
  );

  // Generate meshes in parallel
  let mesh_results: Vec<_> = nodes
    .par_iter()
    .filter_map(|&node| {
      let sampled = sample_volume_for_node(&node, &*sampler, &config);

      // Skip homogeneous volumes
      if is_homogeneous(&sampled.volume) {
        return None;
      }

      let mesh_config = MeshConfig::default().with_voxel_size(settings.voxel_size);

      let output = surface_nets::generate(&sampled.volume, &sampled.materials, &mesh_config);

      if output.vertices.is_empty() {
        return None;
      }

      Some((node, output))
    })
    .collect();

  info!(
    "[SdfTest] Generated {} non-empty meshes",
    mesh_results.len()
  );

  // Spawn mesh entities
  for (node, output) in mesh_results {
    let mesh = create_bevy_mesh(&output);
    let mesh_handle = meshes.add(mesh);

    // World position of chunk
    let world_min = config.get_node_min(&node);

    let material = if settings.color_chunks {
      // Generate unique color per chunk based on position
      let color = chunk_color(node.x, node.y, node.z);
      materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.6,
        cull_mode: None,
        ..default()
      })
    } else {
      lod_materials.neutral.clone()
    };

    commands.spawn((
      Mesh3d(mesh_handle),
      MeshMaterial3d(material),
      Transform::from_translation(Vec3::new(
        world_min.x as f32,
        world_min.y as f32,
        world_min.z as f32,
      ))
      .with_scale(Vec3::splat(settings.voxel_size)),
      SdfTestChunk { node },
      SceneEntity,
    ));
  }
}

/// Generate a unique color for a chunk based on its grid position.
/// Uses golden ratio hue distribution for visually distinct adjacent colors.
fn chunk_color(x: i32, y: i32, z: i32) -> Color {
  const GOLDEN_RATIO: f32 = 0.618033988749895;

  // Hash position to get a unique seed
  let hash = ((x.wrapping_mul(73856093)) ^ (y.wrapping_mul(19349663)) ^ (z.wrapping_mul(83492791)))
    as u32;

  // Use golden ratio for good hue distribution
  let hue = (hash as f32 * GOLDEN_RATIO) % 1.0;

  // Vary saturation/lightness slightly based on position for more distinction
  let saturation = 0.7 + 0.2 * ((x.abs() % 3) as f32 / 2.0);
  let lightness = 0.45 + 0.1 * ((y.abs() % 3) as f32 / 2.0);

  Color::hsl(hue * 360.0, saturation, lightness)
}

fn create_bevy_mesh(output: &voxel_plugin::types::MeshOutput) -> Mesh {
  let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

  if output.is_empty() {
    return mesh;
  }

  let positions: Vec<[f32; 3]> = output.vertices.iter().map(|v| v.position).collect();
  let normals: Vec<[f32; 3]> = output.vertices.iter().map(|v| v.normal).collect();

  mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
  mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
  mesh.insert_indices(Indices::U32(output.indices.clone()));

  mesh
}

/// Create per-LOD colored materials
fn create_lod_materials(materials: &mut Assets<StandardMaterial>) -> LodMaterials {
  const GOLDEN_RATIO: f32 = 0.618033988749895;
  let mut hue = 0.1_f32;

  let colored: Vec<Handle<StandardMaterial>> = (0..8)
    .map(|_| {
      let color = Color::hsl(hue * 360.0, 0.8, 0.5);
      hue = (hue + GOLDEN_RATIO) % 1.0;
      materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.6,
        cull_mode: None,
        ..default()
      })
    })
    .collect();

  let neutral = materials.add(StandardMaterial {
    base_color: Color::srgb(0.7, 0.7, 0.7),
    perceptual_roughness: 0.6,
    cull_mode: None,
    ..default()
  });

  LodMaterials {
    materials: colored,
    neutral,
  }
}

// =============================================================================
// UI
// =============================================================================

fn ui_panel(
  mut contexts: EguiContexts,
  mut settings: ResMut<SdfTestSettings>,
  mut rebuild_events: MessageWriter<RebuildMeshesEvent>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  egui::Window::new("SDF Test")
    .default_pos([10.0, 10.0])
    .show(ctx, |ui| {
      ui.heading("Shape");

      let shapes = [
        SdfShape::TiltedPlane,
        SdfShape::Sphere,
        SdfShape::GroundPlane,
        SdfShape::Box,
      ];

      for shape in shapes {
        if ui
          .selectable_label(settings.shape == shape, shape.name())
          .clicked()
        {
          settings.shape = shape;
          rebuild_events.write(RebuildMeshesEvent);
        }
      }

      ui.separator();
      ui.heading("Settings");

      ui.horizontal(|ui| {
        ui.label("Voxel Size:");
        let sizes = [0.125, 0.25, 0.5, 1.0, 2.0, 4.0];
        for size in sizes {
          if ui
            .selectable_label((settings.voxel_size - size).abs() < 0.01, format!("{}", size))
            .clicked()
          {
            settings.voxel_size = size;
            rebuild_events.write(RebuildMeshesEvent);
          }
        }
      });

      ui.horizontal(|ui| {
        ui.label("Grid Extent:");
        if ui
          .add(egui::Slider::new(&mut settings.grid_extent, 1..=4))
          .changed()
        {
          rebuild_events.write(RebuildMeshesEvent);
        }
      });

      if ui
        .checkbox(&mut settings.color_chunks, "Color Chunks")
        .changed()
      {
        rebuild_events.write(RebuildMeshesEvent);
      }

      ui.separator();
      ui.label("Controls:");
      ui.label("  Tab - Cycle shapes");
      ui.label("  +/- - Voxel size");
      ui.label("  C - Toggle chunk colors");
      ui.label("  WASD - Move camera");
      ui.label("  Right-click + drag - Look");
    });
}
