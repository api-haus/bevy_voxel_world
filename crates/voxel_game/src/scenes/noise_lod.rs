//! Noise LOD Demo Scene
//!
//! Octree-based voxel terrain with FastNoise2.
//! Shows octree LOD refinement with procedural noise.
//! Scene loads asynchronously - initial coarse meshes appear first,
//! then detail is progressively refined.
//! Controls:
//! - Right-click + drag: Look around
//! - WASD: Move camera
//! - Space/Shift: Move up/down
//! - Ctrl: Sprint

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use rand::Rng;
use rayon::prelude::*;
use smallvec::SmallVec;
use voxel_bevy::components::{VoxelChunk, VoxelViewer};
use voxel_bevy::fly_camera::{update_fly_camera, FlyCamera};
use voxel_bevy::noise::{is_homogeneous, FastNoise2Terrain};
use voxel_bevy::resources::{ChunkEntityMap, LodMaterials};
use voxel_bevy::systems::entities::spawn_chunk_entity;
use voxel_bevy::systems::meshing::compute_neighbor_mask;
use voxel_bevy::world::{sync_world_transforms, VoxelWorldRoot, WorldChunkMap};
use voxel_plugin::constants::SAMPLE_SIZE_CB;
use voxel_plugin::octree::{
  refine, OctreeConfig, OctreeNode, RefinementBudget, RefinementInput, TransitionGroup,
  TransitionType,
};
use voxel_plugin::pipeline::AsyncPipeline;
use voxel_plugin::surface_nets;
use voxel_plugin::threading::TaskExecutor;
use voxel_plugin::types::MeshConfig;
use voxel_plugin::world::WorldId;

use super::{Scene, SceneEntity};

/// Re-use glam DVec3 from bevy
type DVec3 = bevy::math::DVec3;

/// Initial LOD for octree (coarse starting point, will refine from here).
const INITIAL_LOD: i32 = 4;

/// Plugin for the noise LOD scene
pub struct NoiseLodPlugin;

impl Plugin for NoiseLodPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<UiSettings>()
      .init_resource::<WorldChunkMap>()
      .init_resource::<AsyncRefinementState>()
      .add_message::<RebuildWorldEvent>()
      .add_message::<RefineWorldEvent>()
      .add_message::<InitialMeshGenEvent>()
      .add_systems(OnEnter(Scene::NoiseLod), setup)
      .add_systems(OnExit(Scene::NoiseLod), cleanup_camera)
      .add_systems(
        Update,
        (
          update_fly_camera.run_if(in_state(Scene::NoiseLod)),
          sync_world_transforms.run_if(in_state(Scene::NoiseLod)),
          toggle_lod_colors.run_if(in_state(Scene::NoiseLod)),
          rebuild_world.run_if(in_state(Scene::NoiseLod)),
          initial_mesh_gen.run_if(in_state(Scene::NoiseLod)),
          start_refinement.run_if(in_state(Scene::NoiseLod)),
          poll_async_refinement.run_if(in_state(Scene::NoiseLod)),
          continuous_refinement.run_if(in_state(Scene::NoiseLod)),
        ),
      )
      .add_systems(
        EguiPrimaryContextPass,
        (
          ui_controls.run_if(in_state(Scene::NoiseLod)),
          instructions_ui.run_if(in_state(Scene::NoiseLod)),
        ),
      );
  }
}

/// Remove FlyCamera and VoxelViewer components from main camera when leaving
/// scene
fn cleanup_camera(mut commands: Commands, camera_query: Query<Entity, With<crate::MainCamera>>) {
  if let Ok(camera_entity) = camera_query.single() {
    commands
      .entity(camera_entity)
      .remove::<FlyCamera>()
      .remove::<VoxelViewer>();
  }
}

/// Sampler source selection.
/// Currently only FastNoise2, but designed for future samplers (SDF primitives,
/// etc).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SamplerSource {
  /// FastNoise2 terrain with caves (native FFI or WASM JS bridge).
  #[default]
  FastNoise2,
}

impl SamplerSource {
  /// Get display name for UI.
  pub fn name(&self) -> &'static str {
    match self {
      Self::FastNoise2 => "FastNoise2",
    }
  }
}

/// UI settings resource for demo controls.
#[derive(Resource)]
struct UiSettings {
  lod_colors_enabled: bool,
  current_seed: i32,
  prev_lod_colors_enabled: bool,
  sampler_source: SamplerSource,
  prev_sampler_source: SamplerSource,
}

impl Default for UiSettings {
  fn default() -> Self {
    Self {
      lod_colors_enabled: true,
      current_seed: 1337,
      prev_lod_colors_enabled: true,
      sampler_source: SamplerSource::default(),
      prev_sampler_source: SamplerSource::default(),
    }
  }
}

/// Async refinement state resource.
/// Tracks in-flight refinement operations using the cross-platform
/// TaskExecutor.
#[derive(Resource)]
struct AsyncRefinementState {
  pipeline: AsyncPipeline,
  /// Pending transitions that need entity management after mesh generation.
  pending_transitions: Option<PendingRefinement>,
  /// Whether continuous refinement is enabled.
  continuous: bool,
  /// Frames since last refinement check (for throttling continuous mode).
  frames_since_check: u32,
}

/// Data needed to complete a refinement after async mesh generation.
struct PendingRefinement {
  world_id: WorldId,
  transitions: Vec<TransitionGroup>,
}

impl Default for AsyncRefinementState {
  fn default() -> Self {
    Self {
      pipeline: AsyncPipeline::with_executor(Arc::new(TaskExecutor::default_threads())),
      pending_transitions: None,
      continuous: false,
      frames_since_check: 0,
    }
  }
}

/// Message to trigger world rebuild with new seed.
#[derive(Message)]
struct RebuildWorldEvent {
  seed: i32,
  sampler_source: SamplerSource,
}

/// Create a volume sampler based on the selected noise source.
fn create_sampler(
  sampler_source: SamplerSource,
  seed: i32,
) -> Box<dyn voxel_plugin::pipeline::VolumeSampler> {
  match sampler_source {
    SamplerSource::FastNoise2 => Box::new(FastNoise2Terrain::new(seed)),
  }
}

/// Message to trigger LOD refinement at current viewer position.
#[derive(Message)]
struct RefineWorldEvent;

/// Message to trigger initial mesh generation for starting leaves.
#[derive(Message)]
struct InitialMeshGenEvent;

// =============================================================================
// Setup
// =============================================================================

fn setup(
  mut commands: Commands,
  mut materials: ResMut<Assets<StandardMaterial>>,
  mut initial_gen_events: MessageWriter<InitialMeshGenEvent>,
  camera_query: Query<Entity, With<crate::MainCamera>>,
) {
  info!("[NoiseLod] Setting up octree scene (async)...");

  // 1. Create terrain sampler
  let sampler = FastNoise2Terrain::new(1337);

  // 2. Create octree configuration
  let config = OctreeConfig {
    voxel_size: 1.0,
    world_origin: DVec3::new(-500.0, -100.0, -500.0),
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
  };

  // 3. Create VoxelWorldRoot with initial coarse leaves only
  let mut world_root = VoxelWorldRoot::new(config.clone(), Box::new(sampler));

  // Initialize with just a few coarse nodes - refinement will add detail
  for x in -1..=1 {
    for y in -1..=1 {
      for z in -1..=1 {
        world_root
          .world
          .leaves
          .insert(OctreeNode::new(x, y, z, INITIAL_LOD));
      }
    }
  }

  info!(
    "[NoiseLod] Initial leaves: {} (generating async)",
    world_root.world.leaves.len()
  );

  // 4. Create per-LOD materials
  let lod_materials = create_lod_materials(&mut materials);

  // 5. Spawn VoxelWorldRoot entity (no meshes yet - async will generate them)
  commands.spawn((world_root, Transform::default(), SceneEntity));

  // 6. Insert resources
  commands.insert_resource(ChunkEntityMap::default());
  commands.insert_resource(lod_materials);

  // 7. Setup camera and lights immediately
  setup_camera_and_lights(&mut commands, &camera_query);

  // 8. Trigger initial mesh generation for starting leaves
  initial_gen_events.write(InitialMeshGenEvent);

  info!("[NoiseLod] Scene setup complete - generating initial meshes...");
}

/// System to generate meshes for initial leaves (runs once at startup).
fn initial_mesh_gen(
  mut events: MessageReader<InitialMeshGenEvent>,
  mut async_state: ResMut<AsyncRefinementState>,
  world_roots: Query<&VoxelWorldRoot>,
  settings: Res<UiSettings>,
) {
  if events.read().next().is_none() {
    return;
  }

  // Don't start if already processing
  if async_state.pipeline.is_busy() || async_state.pending_transitions.is_some() {
    info!("[InitialGen] Pipeline busy, skipping");
    return;
  }

  let Ok(world_root) = world_roots.single() else {
    warn!("[InitialGen] VoxelWorldRoot not found");
    return;
  };

  let world_id = world_root.id();
  let config = world_root.config().clone();
  let leaves = world_root.world.leaves.as_set().clone();

  // Create a "fake" transition that adds all initial leaves
  // This tricks the async pipeline into generating meshes for them
  let initial_nodes: SmallVec<[OctreeNode; 8]> = leaves.iter().copied().collect();
  let transition = TransitionGroup {
    transition_type: TransitionType::Subdivide,
    group_key: OctreeNode::new(0, 0, 0, INITIAL_LOD + 1), // Dummy parent
    nodes_to_remove: SmallVec::new(),
    nodes_to_add: initial_nodes,
  };

  info!(
    "[InitialGen] Starting async generation for {} leaves",
    leaves.len()
  );

  // Create sampler and start async processing
  let sampler = FastNoise2Terrain::new(settings.current_seed);

  let started =
    async_state
      .pipeline
      .start(world_id, vec![transition.clone()], sampler, leaves, config);

  if started {
    // Store pending transitions for entity management after completion
    async_state.pending_transitions = Some(PendingRefinement {
      world_id,
      transitions: vec![transition],
    });

    // Enable continuous refinement after initial gen completes
    async_state.continuous = true;
  } else {
    warn!("[InitialGen] Failed to start pipeline");
  }
}

/// Create per-LOD colored materials
fn create_lod_materials(materials: &mut Assets<StandardMaterial>) -> LodMaterials {
  const GOLDEN_RATIO: f32 = 0.618033988749895;
  let mut hue = 0.6769420_f32;
  let colored: Vec<Handle<StandardMaterial>> = (0..32)
    .map(|i| {
      let saturation = if i % 2 == 0 { 0.9 } else { 0.7 };
      let brightness = if i % 4 < 2 { 1.0 } else { 0.85 };
      let color = Color::hsl(hue * 360.0, saturation, brightness * 0.5);
      hue = (hue + GOLDEN_RATIO * 0.5) % 1.0;
      materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.7,
        cull_mode: None,
        ..default()
      })
    })
    .collect();

  let neutral = materials.add(StandardMaterial {
    base_color: Color::srgb(0.6, 0.6, 0.6),
    perceptual_roughness: 0.7,
    cull_mode: None,
    ..default()
  });

  LodMaterials {
    materials: colored,
    neutral,
  }
}

/// Spawn a chunk entity with SceneEntity marker
fn spawn_chunk_entity_with_marker(
  commands: &mut Commands,
  meshes: &mut Assets<Mesh>,
  material: Handle<StandardMaterial>,
  chunk_map: &mut ChunkEntityMap,
  world_chunk_map: &mut WorldChunkMap,
  world_id: voxel_plugin::world::WorldId,
  node: OctreeNode,
  output: &voxel_plugin::MeshOutput,
  config: &OctreeConfig,
) {
  // Use the existing spawn function but add SceneEntity
  spawn_chunk_entity(
    commands,
    meshes,
    material,
    chunk_map,
    Some(world_chunk_map),
    world_id,
    node,
    output,
    config,
  );

  // Get the entity that was just spawned (last one in chunk_map)
  if let Some(&entity) = chunk_map.map.get(&node) {
    commands.entity(entity).insert(SceneEntity);
  }
}

/// Setup camera and lighting
fn setup_camera_and_lights(
  commands: &mut Commands,
  camera_query: &Query<Entity, With<crate::MainCamera>>,
) {
  // Configure existing main camera with FlyCamera and VoxelViewer
  if let Ok(camera_entity) = camera_query.single() {
    commands.entity(camera_entity).insert((
      Transform::from_translation(Vec3::new(0.0, 100.0, 100.0)).looking_at(Vec3::ZERO, Vec3::Y),
      FlyCamera {
        speed: 100.0,
        sensitivity: 0.003,
        yaw: 0.0,
        pitch: -0.3,
      },
      VoxelViewer,
    ));
  }

  // Directional light (sun)
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
  commands.insert_resource(AmbientLight {
    color: Color::srgb(0.6, 0.7, 0.8),
    brightness: 200.0,
    affects_lightmapped_meshes: false,
  });
}

// =============================================================================
// UI
// =============================================================================

/// Instructions text via egui
fn instructions_ui(mut contexts: EguiContexts) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  egui::Area::new(egui::Id::new("noise_lod_instructions"))
    .anchor(egui::Align2::LEFT_BOTTOM, [10.0, -10.0])
    .show(ctx, |ui| {
      ui.label(
        egui::RichText::new(
          "Right-click + drag to look | WASD to move | Space/Shift up/down | Ctrl to sprint",
        )
        .color(egui::Color32::from_rgb(200, 200, 200))
        .size(14.0),
      );
    });
}

/// UI controls panel
fn ui_controls(
  mut contexts: EguiContexts,
  mut settings: ResMut<UiSettings>,
  mut async_state: ResMut<AsyncRefinementState>,
  mut rebuild_events: MessageWriter<RebuildWorldEvent>,
  mut refine_events: MessageWriter<RefineWorldEvent>,
) {
  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };
  egui::Window::new("Controls")
    .anchor(egui::Align2::RIGHT_TOP, [-10.0, 60.0])
    .resizable(false)
    .show(ctx, |ui| {
      ui.checkbox(&mut settings.lod_colors_enabled, "LOD Colors");

      ui.separator();

      // Async refinement status
      let is_processing =
        async_state.pipeline.is_busy() || async_state.pending_transitions.is_some();
      ui.horizontal(|ui| {
        ui.label("Status:");
        if is_processing {
          ui.colored_label(egui::Color32::YELLOW, "Processing...");
        } else {
          ui.colored_label(egui::Color32::GREEN, "Idle");
        }
      });

      ui.horizontal(|ui| {
        ui.checkbox(&mut async_state.continuous, "Continuous");
        if ui
          .add_enabled(!is_processing, egui::Button::new("Refine LOD"))
          .clicked()
        {
          refine_events.write(RefineWorldEvent);
        }
      });

      ui.separator();

      ui.horizontal(|ui| {
        ui.label("Seed:");
        ui.add(egui::DragValue::new(&mut settings.current_seed));
      });

      if ui.button("Rebuild World").clicked() {
        rebuild_events.write(RebuildWorldEvent {
          seed: settings.current_seed,
          sampler_source: settings.sampler_source,
        });
      }

      if ui.button("Random Seed").clicked() {
        let new_seed = rand::rng().random::<i32>();
        settings.current_seed = new_seed;
        rebuild_events.write(RebuildWorldEvent {
          seed: new_seed,
          sampler_source: settings.sampler_source,
        });
      }

      ui.separator();

      // Noise source selection
      ui.horizontal(|ui| {
        ui.label("Noise:");
        egui::ComboBox::from_id_salt("sampler_source")
          .selected_text(settings.sampler_source.name())
          .show_ui(ui, |ui| {
            ui.selectable_value(
              &mut settings.sampler_source,
              SamplerSource::FastNoise2,
              "FastNoise2",
            );
          });
      });

      // Auto-rebuild if noise source changed
      if settings.sampler_source != settings.prev_sampler_source {
        settings.prev_sampler_source = settings.sampler_source;
        rebuild_events.write(RebuildWorldEvent {
          seed: settings.current_seed,
          sampler_source: settings.sampler_source,
        });
      }
    });
}

/// System to toggle LOD colors on/off
fn toggle_lod_colors(
  mut settings: ResMut<UiSettings>,
  lod_materials: Option<Res<LodMaterials>>,
  mut chunks: Query<(&VoxelChunk, &mut MeshMaterial3d<StandardMaterial>)>,
) {
  if settings.lod_colors_enabled == settings.prev_lod_colors_enabled {
    return;
  }
  settings.prev_lod_colors_enabled = settings.lod_colors_enabled;

  let Some(lod_materials) = lod_materials else {
    return;
  };

  for (chunk, mut material) in &mut chunks {
    material.0 = lod_materials.get(chunk.node.lod, settings.lod_colors_enabled);
  }
}

/// System to rebuild world when message is received
fn rebuild_world(
  mut commands: Commands,
  mut rebuild_events: MessageReader<RebuildWorldEvent>,
  mut meshes: ResMut<Assets<Mesh>>,
  chunks: Query<Entity, With<VoxelChunk>>,
  settings: Res<UiSettings>,
  lod_materials: Option<Res<LodMaterials>>,
  mut world_roots: Query<&mut VoxelWorldRoot>,
  mut chunk_map: Option<ResMut<ChunkEntityMap>>,
  mut world_chunk_map: ResMut<WorldChunkMap>,
) {
  let Some(event) = rebuild_events.read().last() else {
    return;
  };

  info!(
    "[NoiseLod] Rebuilding world with seed: {}, noise: {:?}",
    event.seed, event.sampler_source
  );

  // Despawn all existing chunks
  for entity in &chunks {
    commands.entity(entity).despawn();
  }

  // Clear chunk map
  if let Some(ref mut map) = chunk_map {
    map.map.clear();
  }

  let Some(lod_materials) = lod_materials else {
    warn!("LodMaterials not available");
    return;
  };

  // Get the VoxelWorldRoot
  let Ok(mut world_root) = world_roots.single_mut() else {
    warn!("VoxelWorldRoot not found");
    return;
  };

  let world_id = world_root.id();

  // Clear world chunks from WorldChunkMap
  world_chunk_map.remove_world(world_id);

  // Create new terrain sampler with the selected noise source
  // We need two copies: one for world_root (owned) and one for parallel sampling
  // (Arc)
  let sampler: Arc<dyn voxel_plugin::pipeline::VolumeSampler> = match event.sampler_source {
    SamplerSource::FastNoise2 => Arc::new(FastNoise2Terrain::new(event.seed)),
  };

  // Update the world's sampler with the new noise source
  world_root.world.sampler = create_sampler(event.sampler_source, event.seed);

  let config = world_root.config().clone();
  let leaf_nodes: Vec<_> = world_root.world.leaves.iter().copied().collect();
  let use_lod_colors = settings.lod_colors_enabled;

  // Parallel: sample noise and generate meshes
  let chunk_meshes: Vec<_> = leaf_nodes
    .par_iter()
    .filter_map(|node| {
      let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
      let mut mats = Box::new([0u8; SAMPLE_SIZE_CB]);

      let node_min = config.get_node_min(node);
      let voxel_size = config.get_voxel_size(node.lod);
      sampler.sample_volume(
        [node_min.x, node_min.y, node_min.z],
        voxel_size,
        &mut volume,
        &mut mats,
      );

      if is_homogeneous(&volume) {
        return None;
      }

      let neighbor_mask = compute_neighbor_mask(node, &world_root.world.leaves, &config);

      let mesh_config = MeshConfig::default()
        .with_voxel_size(voxel_size as f32)
        .with_neighbor_mask(neighbor_mask);

      let output = surface_nets::generate(&volume, &mats, &mesh_config);

      if output.is_empty() {
        return None;
      }

      Some((*node, output))
    })
    .collect();

  let mesh_count = chunk_meshes.len();
  let empty_count = leaf_nodes.len() - mesh_count;

  let mut local_chunk_map = ChunkEntityMap::default();
  let chunk_map_ref = if let Some(ref mut map) = chunk_map {
    &mut **map
  } else {
    &mut local_chunk_map
  };

  // Sequential: spawn entities
  for (node, output) in chunk_meshes {
    spawn_chunk_entity_with_marker(
      &mut commands,
      &mut meshes,
      lod_materials.get(node.lod, use_lod_colors),
      chunk_map_ref,
      &mut world_chunk_map,
      world_id,
      node,
      &output,
      &config,
    );
  }

  info!(
    "[NoiseLod] Rebuilt world: {} meshes, {} empty chunks",
    mesh_count, empty_count
  );
}

// =============================================================================
// Refinement - Async pipeline using rayon background threads
// =============================================================================

/// Throttle continuous refinement to this many frames between checks.
const CONTINUOUS_REFINEMENT_INTERVAL: u32 = 15;

/// System to start refinement when triggered by message.
///
/// This kicks off async mesh generation on background threads (via rayon).
/// The poll_async_refinement system handles completion.
fn start_refinement(
  mut refine_events: MessageReader<RefineWorldEvent>,
  mut async_state: ResMut<AsyncRefinementState>,
  viewers: Query<&GlobalTransform, With<VoxelViewer>>,
  mut world_roots: Query<&mut VoxelWorldRoot>,
  settings: Res<UiSettings>,
) {
  if refine_events.read().next().is_none() {
    return;
  }

  // Don't start new refinement if one is in progress
  if async_state.pipeline.is_busy() || async_state.pending_transitions.is_some() {
    info!("[Refine] Already processing, skipping");
    return;
  }

  let Ok(mut world_root) = world_roots.single_mut() else {
    warn!("VoxelWorldRoot not found");
    return;
  };

  // Get viewer position
  let viewer_pos = viewers
    .iter()
    .next()
    .map(|t| {
      let p = t.translation();
      DVec3::new(p.x as f64, p.y as f64, p.z as f64)
    })
    .unwrap_or(DVec3::new(0.0, 50.0, 0.0));

  let config = world_root.config().clone();

  // Run octree refinement (this is CPU-bound but fast)
  let input = RefinementInput {
    viewer_pos,
    config: config.clone(),
    prev_leaves: world_root.world.leaves.as_set().clone(),
    budget: RefinementBudget::DEFAULT,
  };

  let output = refine(input);

  if output.transition_groups.is_empty() {
    return;
  }

  let world_id = world_root.id();

  // Apply transitions to world state immediately
  for group in &output.transition_groups {
    for node in &group.nodes_to_remove {
      world_root.world.leaves.remove(node);
    }
    for node in &group.nodes_to_add {
      world_root.world.leaves.insert(*node);
    }
  }

  // Create sampler and start async processing
  let sampler = FastNoise2Terrain::new(settings.current_seed);
  let leaves = world_root.world.leaves.as_set().clone();
  let transitions = output.transition_groups.clone();

  // Start async mesh generation (non-blocking)
  // Check return value - should always succeed due to is_busy() guard at function
  // start, but be defensive to avoid pending_transitions/pipeline mismatch.
  let started = async_state
    .pipeline
    .start(world_id, transitions.clone(), sampler, leaves, config);

  if started {
    // Store pending transitions for entity management after completion
    async_state.pending_transitions = Some(PendingRefinement {
      world_id,
      transitions,
    });
  } else {
    // Should not happen - pipeline.is_busy() check at function start prevents this.
    // If it does happen, world.leaves is now inconsistent with tracked transitions.
    warn!("[Refine] pipeline.start() returned false unexpectedly - state may be inconsistent");
  }
}

/// System to poll for async refinement completion and spawn entities.
fn poll_async_refinement(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut async_state: ResMut<AsyncRefinementState>,
  world_roots: Query<&VoxelWorldRoot>,
  chunks: Query<(Entity, &VoxelChunk)>,
  settings: Res<UiSettings>,
  lod_materials: Option<Res<LodMaterials>>,
  mut chunk_map: Option<ResMut<ChunkEntityMap>>,
  mut world_chunk_map: ResMut<WorldChunkMap>,
) {
  // Poll for completion (non-blocking)
  let Some(ready_chunks) = async_state.pipeline.poll() else {
    return;
  };

  let Some(pending) = async_state.pending_transitions.take() else {
    return;
  };

  let Some(lod_materials) = lod_materials else {
    warn!("LodMaterials not available");
    return;
  };

  let Ok(world_root) = world_roots.single() else {
    warn!("VoxelWorldRoot not found");
    return;
  };

  let config = world_root.config().clone();
  let use_lod_colors = settings.lod_colors_enabled;

  // Build node -> entity map for quick lookup
  let node_to_entity: HashMap<OctreeNode, Entity> = chunks
    .iter()
    .filter(|(_, chunk)| chunk.world_id == pending.world_id)
    .map(|(entity, chunk)| (chunk.node, entity))
    .collect();

  // Despawn old nodes
  for group in &pending.transitions {
    for node in &group.nodes_to_remove {
      if let Some(&entity) = node_to_entity.get(node) {
        commands.entity(entity).despawn();
        if let Some(ref mut map) = chunk_map {
          map.map.remove(node);
        }
        world_chunk_map.remove(pending.world_id, node);
      }
    }
  }

  // Spawn new chunks - only spawn if node is still in world.leaves
  // This guards against stale ready_chunks from async timing gaps
  let mut local_chunk_map = ChunkEntityMap::default();
  let chunk_map_ref = if let Some(ref mut map) = chunk_map {
    &mut **map
  } else {
    &mut local_chunk_map
  };

  let mut skipped = 0;
  for ready in ready_chunks {
    // Guard: only spawn if node is still a leaf (prevents orphan entities)
    if !world_root.world.leaves.contains(&ready.node) {
      skipped += 1;
      continue;
    }

    let mesh_output = mesh_data_to_output(&ready.mesh_data);

    spawn_chunk_entity_with_marker(
      &mut commands,
      &mut meshes,
      lod_materials.get(ready.node.lod, use_lod_colors),
      chunk_map_ref,
      &mut world_chunk_map,
      pending.world_id,
      ready.node,
      &mesh_output,
      &config,
    );
  }

  if skipped > 0 {
    warn!(
      "[Refine] Skipped {} stale nodes (no longer leaves)",
      skipped
    );
  }
}

/// System for continuous automatic refinement based on viewer movement.
fn continuous_refinement(
  mut async_state: ResMut<AsyncRefinementState>,
  mut refine_events: MessageWriter<RefineWorldEvent>,
) {
  if !async_state.continuous {
    return;
  }

  // Don't trigger if already processing
  if async_state.pipeline.is_busy() || async_state.pending_transitions.is_some() {
    return;
  }

  // Throttle check frequency
  async_state.frames_since_check += 1;
  if async_state.frames_since_check < CONTINUOUS_REFINEMENT_INTERVAL {
    return;
  }
  async_state.frames_since_check = 0;

  // Trigger refinement check
  refine_events.write(RefineWorldEvent);
}

/// Convert MeshData (bytes) back to MeshOutput (typed).
/// This is needed because spawn_chunk_entity expects MeshOutput.
fn mesh_data_to_output(data: &voxel_plugin::pipeline::MeshData) -> voxel_plugin::MeshOutput {
  use voxel_plugin::types::Vertex;

  let vertices: Vec<Vertex> = if data.vertices.is_empty() {
    Vec::new()
  } else {
    let vertex_count = data.vertex_count as usize;
    let mut verts = Vec::with_capacity(vertex_count);
    unsafe {
      let ptr = data.vertices.as_ptr() as *const Vertex;
      verts.extend_from_slice(std::slice::from_raw_parts(ptr, vertex_count));
    }
    verts
  };

  let indices: Vec<u32> = if data.indices.is_empty() {
    Vec::new()
  } else {
    let index_count = data.index_count as usize;
    let mut idx = Vec::with_capacity(index_count);
    unsafe {
      let ptr = data.indices.as_ptr() as *const u32;
      idx.extend_from_slice(std::slice::from_raw_parts(ptr, index_count));
    }
    idx
  };

  voxel_plugin::MeshOutput {
    vertices,
    indices,
    displaced_positions: Vec::new(), // Not used when converting from MeshData
    bounds: data.bounds,
  }
}
