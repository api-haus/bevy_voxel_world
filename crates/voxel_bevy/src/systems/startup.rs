//! Startup system for octree scene initialization.

use bevy::prelude::*;
use rayon::prelude::*;
use voxel_plugin::noise::{is_homogeneous, FastNoise2Terrain};
use voxel_plugin::octree::{refine, OctreeConfig, OctreeLeaves, RefinementBudget, RefinementInput};
use voxel_plugin::pipeline::sample_volume_for_node;
use voxel_plugin::surface_nets;
use voxel_plugin::types::MeshConfig;
use voxel_plugin::world::WorldId;

// Re-use DVec3 from bevy
type DVec3 = bevy::math::DVec3;

use crate::input::fly_camera_input_bundle;
use crate::resources::{ChunkEntityMap, LodMaterials, OctreeLodState};
use crate::systems::entities::spawn_chunk_entity;
use crate::systems::meshing::compute_neighbor_mask;
use crate::FlyCamera;

/// Initial LOD for octree (will refine from here).
const INITIAL_LOD: i32 = 4;

/// Maximum refinement iterations at startup.
const MAX_STARTUP_ITERATIONS: usize = 10;

/// Startup system: pre-compute octree refinement and spawn all meshes.
pub fn setup_octree_scene(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  info!("Setting up octree scene...");

  // 1. Create terrain sampler (FastNoise2)
  let sampler = FastNoise2Terrain::new(1337);

  // 2. Create octree configuration
  let config = OctreeConfig {
    voxel_size: 1.0,
    world_origin: DVec3::new(-500.0, -100.0, -500.0), // Center world around origin
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5, // Controls LOD distance thresholds
  };

  // 3. Initialize octree with a grid at INITIAL_LOD
  let mut leaves = OctreeLeaves::new();

  // Create a 3x3x3 grid of initial nodes to cover more area
  for x in -1..=1 {
    for y in -1..=1 {
      for z in -1..=1 {
        leaves.insert(voxel_plugin::octree::OctreeNode::new(x, y, z, INITIAL_LOD));
      }
    }
  }

  info!("Initial leaves: {}", leaves.len());

  // 4. Pre-compute refinement until stable
  let viewer_pos = DVec3::new(0.0, 50.0, 0.0);

  for iteration in 0..MAX_STARTUP_ITERATIONS {
    let input = RefinementInput {
      viewer_pos,
      config: config.clone(),
      prev_leaves: leaves.as_set().clone(),
      budget: RefinementBudget::UNLIMITED, // Large budget for startup
    };

    let output = refine(input);

    if output.transition_groups.is_empty() {
      info!("Refinement stable after {} iterations", iteration);
      break;
    }

    // Apply transitions to leaves
    for group in &output.transition_groups {
      for node in &group.nodes_to_remove {
        leaves.remove(node);
      }
      for node in &group.nodes_to_add {
        leaves.insert(*node);
      }
    }

    info!(
      "Iteration {}: {} transitions, {} leaves",
      iteration,
      output.transition_groups.len(),
      leaves.len()
    );
  }

  info!("Final leaf count: {}", leaves.len());

  // 5. Create per-LOD materials for visualization
  // Golden ratio hue distribution with alternating saturation/brightness
  let lod_materials = {
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
          cull_mode: None, // Double-sided for debugging
          ..default()
        })
      })
      .collect();

    // Neutral gray material for when LOD colors are disabled
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
  };

  // 6. Generate meshes for all leaves (parallel noise + meshing)
  let mut chunk_map = ChunkEntityMap::default();

  // Collect leaves first
  let leaf_nodes: Vec<_> = leaves.iter().copied().collect();

  // Parallel: sample noise and generate meshes
  let chunk_meshes: Vec<_> = leaf_nodes
    .par_iter()
    .filter_map(|node| {
      // Use centralized sampling helper (handles apron offset)
      let sampled = sample_volume_for_node(node, &sampler, &config);

      if is_homogeneous(&sampled.volume) {
        return None;
      }

      let neighbor_mask = compute_neighbor_mask(node, &leaves, &config);
      let voxel_size = config.get_voxel_size(node.lod);

      let mesh_config = MeshConfig::default()
        .with_voxel_size(voxel_size as f32)
        .with_neighbor_mask(neighbor_mask);

      let output = surface_nets::generate(&sampled.volume, &sampled.materials, &mesh_config);

      if output.is_empty() {
        return None;
      }

      Some((*node, output))
    })
    .collect();

  let mesh_count = chunk_meshes.len();
  let empty_count = leaf_nodes.len() - mesh_count;

  // Create a default WorldId for the main world
  let world_id = WorldId::new();

  // Sequential: spawn entities (Commands isn't thread-safe)
  for (node, output) in chunk_meshes {
    spawn_chunk_entity(
      &mut commands,
      &mut meshes,
      lod_materials.get(node.lod, true),
      &mut chunk_map,
      None, // No WorldChunkMap in legacy startup
      world_id,
      node,
      &output,
      &config,
    );
  }

  info!(
    "Spawned {} mesh entities ({} empty chunks)",
    mesh_count, empty_count
  );

  // 7. Insert resources
  commands.insert_resource(OctreeLodState { leaves, config });
  commands.insert_resource(chunk_map);
  commands.insert_resource(lod_materials);

  // 8. Setup camera and lights
  setup_camera_and_lights(&mut commands);

  info!("Octree scene setup complete!");
}

/// Setup camera and lighting for the scene.
fn setup_camera_and_lights(commands: &mut Commands) {
	// Fly camera with input handling
	commands.spawn((
		Camera3d::default(),
		Transform::from_translation(Vec3::new(0.0, 100.0, 100.0)).looking_at(Vec3::ZERO, Vec3::Y),
		fly_camera_input_bundle(FlyCamera {
			speed: 100.0,
			mouse_sensitivity: 0.003,
			gamepad_sensitivity: 2.0,
			yaw: 0.0,
			pitch: -0.3,
		}),
	));

  // Directional light (sun)
  commands.spawn((
    DirectionalLight {
      illuminance: 10000.0,
      shadows_enabled: true,
      ..default()
    },
    Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
  ));

  // Ambient light
  commands.insert_resource(GlobalAmbientLight {
    color: Color::srgb(0.6, 0.7, 0.8),
    brightness: 200.0,
    affects_lightmapped_meshes: false,
  });
}
