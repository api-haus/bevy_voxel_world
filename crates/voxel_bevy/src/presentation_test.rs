//! Presentation layer consistency tests.
//!
//! Tests the exact logic from noise_lod's poll_async_refinement without Bevy
//! runtime. Simulates entity spawn/despawn tracking to detect orphans.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use bevy::math::DVec3;
use smallvec::{smallvec, SmallVec};
use voxel_plugin::octree::{
  refine, OctreeConfig, OctreeNode, RefinementBudget, RefinementInput, TransitionGroup,
};
use voxel_plugin::pipeline::process_transitions;
use voxel_plugin::world::WorldId;

use voxel_plugin::noise::FastNoise2Terrain;

// =============================================================================
// Mock Bevy Entity System
// =============================================================================

/// Fake entity ID for testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MockEntity(u64);

/// Simulates Bevy's entity/chunk tracking (ChunkEntityMap + WorldChunkMap).
struct MockBevyPresentation {
  /// Maps node -> entity (like ChunkEntityMap.map)
  node_to_entity: HashMap<OctreeNode, MockEntity>,
  /// Next entity ID to assign
  next_entity: u64,
  /// Violations detected
  violations: Vec<PresentationViolation>,
}

#[derive(Debug, Clone)]
struct PresentationViolation {
  frame: usize,
  kind: ViolationKind,
  node: OctreeNode,
}

#[derive(Debug, Clone)]
enum ViolationKind {
  /// Entity exists but node not in world.leaves (orphan)
  OrphanEntity,
  /// Spawned entity for node not in world.leaves
  SpawnedOrphan,
  /// Despawned entity that didn't exist
  DespawnedNonexistent,
  /// Spawned duplicate entity for same node
  DuplicateSpawn,
}

impl MockBevyPresentation {
  fn new() -> Self {
    Self {
      node_to_entity: HashMap::new(),
      next_entity: 1,
      violations: Vec::new(),
    }
  }

  /// Initial spawn for setup phase (like setup_scene).
  fn initial_spawn(&mut self, nodes: &[OctreeNode]) {
    for node in nodes {
      let entity = MockEntity(self.next_entity);
      self.next_entity += 1;
      self.node_to_entity.insert(*node, entity);
    }
  }

  /// Apply poll_async_refinement logic EXACTLY as noise_lod.rs does it.
  ///
  /// This mirrors the exact code from noise_lod.rs:
  /// 1. Despawn old nodes from pending.transitions.nodes_to_remove
  /// 2. Spawn new chunks from ready_chunks (WITH guard: only if still in
  ///    leaves)
  fn apply_poll_result(
    &mut self,
    frame: usize,
    world_leaves: &HashSet<OctreeNode>,
    transitions: &[TransitionGroup],
    ready_chunk_nodes: &[OctreeNode],
  ) {
    // Step 1: Despawn old nodes
    for group in transitions {
      for node in &group.nodes_to_remove {
        // Successfully despawned (or wasn't present - empty chunk)
        self.node_to_entity.remove(node);
      }
    }

    // Step 2: Spawn new chunks WITH guard (mirrors the fix in noise_lod.rs)
    for node in ready_chunk_nodes {
      // Guard: only spawn if node is still a leaf (prevents orphan entities)
      if !world_leaves.contains(node) {
        // Skip stale node - this is the fix for the "stuck tiles" bug
        continue;
      }

      // Check: duplicate spawn?
      if self.node_to_entity.contains_key(node) {
        self.violations.push(PresentationViolation {
          frame,
          kind: ViolationKind::DuplicateSpawn,
          node: *node,
        });
      }

      // Spawn entity
      let entity = MockEntity(self.next_entity);
      self.next_entity += 1;
      self.node_to_entity.insert(*node, entity);
    }
  }

  /// Apply poll result WITHOUT the spawn guard - demonstrates ghost cell bug.
  /// This is what would happen if we didn't check world.leaves before spawning.
  fn apply_poll_result_no_guard(
    &mut self,
    frame: usize,
    world_leaves: &HashSet<OctreeNode>,
    transitions: &[TransitionGroup],
    ready_chunk_nodes: &[OctreeNode],
  ) {
    // Step 1: Despawn old nodes
    for group in transitions {
      for node in &group.nodes_to_remove {
        self.node_to_entity.remove(node);
      }
    }

    // Step 2: Spawn new chunks WITHOUT guard - this causes ghost cells!
    for node in ready_chunk_nodes {
      // NO GUARD - spawn even if node is no longer in leaves
      // This is the bug we're demonstrating

      // Record if this would be a ghost cell
      if !world_leaves.contains(node) {
        self.violations.push(PresentationViolation {
          frame,
          kind: ViolationKind::SpawnedOrphan,
          node: *node,
        });
      }

      // Check: duplicate spawn?
      if self.node_to_entity.contains_key(node) {
        self.violations.push(PresentationViolation {
          frame,
          kind: ViolationKind::DuplicateSpawn,
          node: *node,
        });
      }

      // Spawn entity anyway (demonstrating the bug)
      let entity = MockEntity(self.next_entity);
      self.next_entity += 1;
      self.node_to_entity.insert(*node, entity);
    }
  }

  /// Check for orphan entities (entities for nodes not in world.leaves).
  fn check_orphans(&mut self, frame: usize, world_leaves: &HashSet<OctreeNode>) {
    for (node, _entity) in &self.node_to_entity {
      if !world_leaves.contains(node) {
        self.violations.push(PresentationViolation {
          frame,
          kind: ViolationKind::OrphanEntity,
          node: *node,
        });
      }
    }
  }

  fn has_violations(&self) -> bool {
    !self.violations.is_empty()
  }

  fn entity_count(&self) -> usize {
    self.node_to_entity.len()
  }
}

// =============================================================================
// Tests
// =============================================================================

/// Test the Bevy presentation layer logic with exact noise_lod configuration.
#[test]
fn test_bevy_presentation_consistency() {
  let config = OctreeConfig {
    voxel_size: 1.0,
    world_origin: DVec3::new(-500.0, -100.0, -500.0),
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
    world_bounds: None,
  };

  let sampler = FastNoise2Terrain::new(1337);
  let world_id = WorldId::new();

  // Initialize world.leaves with coarse grid
  let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
  let initial_lod = 5;
  for x in -4..=4 {
    for y in -2..=2 {
      for z in -4..=4 {
        world_leaves.insert(OctreeNode::new(x, y, z, initial_lod));
      }
    }
  }

  // Initial meshing to determine which nodes have geometry
  let initial_nodes: Vec<_> = world_leaves.iter().copied().collect();
  let mut groups = Vec::new();
  for node in &initial_nodes {
    groups.push(TransitionGroup {
      transition_type: voxel_plugin::octree::TransitionType::Subdivide,
      group_key: node.get_parent(config.max_lod).unwrap_or(*node),
      nodes_to_add: smallvec![*node],
      nodes_to_remove: SmallVec::new(),
    });
  }
  let initial_chunks = process_transitions(world_id, &groups, &sampler, &world_leaves, &config);

  // Initialize mock presentation with nodes that have meshes
  let mut presentation = MockBevyPresentation::new();
  let initial_mesh_nodes: Vec<_> = initial_chunks.iter().map(|c| c.node).collect();
  presentation.initial_spawn(&initial_mesh_nodes);

  println!(
    "Initial: {} leaves, {} entities (with mesh)",
    world_leaves.len(),
    presentation.entity_count()
  );

  // Simulate pending refinement state
  struct PendingRefinement {
    transitions: Vec<TransitionGroup>,
    ready_chunk_nodes: Vec<OctreeNode>,
  }
  let mut pending: Option<PendingRefinement> = None;

  // Camera path
  let camera_path = |frame: usize| -> DVec3 {
    let t = frame as f64 * 0.05;
    let x = (t * 0.7).sin() * (t * 0.23).cos() * 400.0;
    let y = (t * 0.31).sin() * 80.0;
    let z = (t * 0.53).cos() * (t * 0.17).sin() * 400.0;
    let zoom = ((t * 2.0).sin() * 0.5 + 0.5) * 50.0 + 10.0;
    DVec3::new(x, y + zoom, z)
  };

  const NUM_FRAMES: usize = 1000;
  const MAX_TEST_DURATION: Duration = Duration::from_secs(40);
  let start = Instant::now();
  let mut total_transitions = 0;

  for frame in 0..NUM_FRAMES {
    if start.elapsed() > MAX_TEST_DURATION {
      println!(
        "Test timeout after {} frames ({:?})",
        frame,
        start.elapsed()
      );
      break;
    }
    let viewer_pos = camera_path(frame);

    // Mirrors start_refinement logic
    if pending.is_none() {
      let input = RefinementInput {
        viewer_pos,
        config: config.clone(),
        prev_leaves: world_leaves.clone(),
        budget: RefinementBudget::DEFAULT,
      };

      let output = refine(input);

      if !output.transition_groups.is_empty() {
        total_transitions += output.transition_groups.len();

        // Update world.leaves IMMEDIATELY (like start_refinement)
        for group in &output.transition_groups {
          for node in &group.nodes_to_remove {
            world_leaves.remove(node);
          }
          for node in &group.nodes_to_add {
            world_leaves.insert(*node);
          }
        }

        // Process through pipeline
        let ready_chunks = process_transitions(
          world_id,
          &output.transition_groups,
          &sampler,
          &world_leaves,
          &config,
        );

        pending = Some(PendingRefinement {
          transitions: output.transition_groups,
          ready_chunk_nodes: ready_chunks.iter().map(|c| c.node).collect(),
        });
      }
    }

    // Mirrors poll_async_refinement logic
    if let Some(p) = pending.take() {
      presentation.apply_poll_result(frame, &world_leaves, &p.transitions, &p.ready_chunk_nodes);

      // Check for orphans after applying
      presentation.check_orphans(frame, &world_leaves);
    }

    if presentation.has_violations() {
      println!("\n=== BEVY VIOLATION at frame {} ===", frame);
      println!("Viewer: {:?}", viewer_pos);
      println!("World leaves: {}", world_leaves.len());
      println!("Entities: {}", presentation.entity_count());
      for v in presentation.violations.iter().take(20) {
        println!("  {:?}", v);
      }
      break;
    }

    if frame % 100 == 0 && frame > 0 {
      println!(
        "Frame {}: {} leaves, {} entities, {} transitions",
        frame,
        world_leaves.len(),
        presentation.entity_count(),
        total_transitions
      );
    }
  }

  println!(
    "\n=== BEVY FINAL ===\nFrames: {}\nTransitions: {}\nLeaves: {}\nEntities: {}\nViolations: {}",
    NUM_FRAMES,
    total_transitions,
    world_leaves.len(),
    presentation.entity_count(),
    presentation.violations.len()
  );

  assert!(
    !presentation.has_violations(),
    "Bevy presentation violations: {:?}",
    &presentation.violations[..presentation.violations.len().min(10)]
  );
}

/// Stress test with extreme zoom transitions.
#[test]
fn test_bevy_presentation_rapid_zoom() {
  let config = OctreeConfig {
    voxel_size: 1.0,
    world_origin: DVec3::new(-500.0, -100.0, -500.0),
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
    world_bounds: None,
  };

  let sampler = FastNoise2Terrain::new(1337);
  let world_id = WorldId::new();

  let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
  world_leaves.insert(OctreeNode::new(0, 0, 0, 5));

  let mut presentation = MockBevyPresentation::new();
  presentation.initial_spawn(&[OctreeNode::new(0, 0, 0, 5)]);

  const MAX_TEST_DURATION: Duration = Duration::from_secs(40);
  let start = Instant::now();

  for cycle in 0..50 {
    if start.elapsed() > MAX_TEST_DURATION {
      println!(
        "Test timeout after {} cycles ({:?})",
        cycle,
        start.elapsed()
      );
      break;
    }
    for &distance in &[5.0, 1000.0] {
      let viewer_pos = DVec3::new(0.0, distance, 0.0);

      for _iter in 0..20 {
        let input = RefinementInput {
          viewer_pos,
          config: config.clone(),
          prev_leaves: world_leaves.clone(),
          budget: RefinementBudget::UNLIMITED,
        };

        let output = refine(input);
        if output.transition_groups.is_empty() {
          break;
        }

        for group in &output.transition_groups {
          for node in &group.nodes_to_remove {
            world_leaves.remove(node);
          }
          for node in &group.nodes_to_add {
            world_leaves.insert(*node);
          }
        }

        let ready_chunks = process_transitions(
          world_id,
          &output.transition_groups,
          &sampler,
          &world_leaves,
          &config,
        );

        presentation.apply_poll_result(
          cycle * 100,
          &world_leaves,
          &output.transition_groups,
          &ready_chunks.iter().map(|c| c.node).collect::<Vec<_>>(),
        );

        presentation.check_orphans(cycle * 100, &world_leaves);

        if presentation.has_violations() {
          println!("Violation at cycle {} distance {}", cycle, distance);
          for v in &presentation.violations {
            println!("  {:?}", v);
          }
          panic!("Bevy stress test violation");
        }
      }
    }

    if cycle % 10 == 0 {
      println!(
        "Cycle {}: {} leaves, {} entities",
        cycle,
        world_leaves.len(),
        presentation.entity_count()
      );
    }
  }

  assert!(
    !presentation.has_violations(),
    "Bevy stress test violations: {:?}",
    &presentation.violations
  );

  println!(
    "Bevy stress test passed: {} leaves, {} entities",
    world_leaves.len(),
    presentation.entity_count()
  );
}

/// Test with variable async delay (simulate real frame timing).
#[test]
fn test_bevy_presentation_async_delay() {
  let config = OctreeConfig {
    voxel_size: 1.0,
    world_origin: DVec3::new(-500.0, -100.0, -500.0),
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
    world_bounds: None,
  };

  let sampler = FastNoise2Terrain::new(1337);
  let world_id = WorldId::new();

  let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
  for x in -2..=2 {
    for y in -1..=1 {
      for z in -2..=2 {
        world_leaves.insert(OctreeNode::new(x, y, z, 5));
      }
    }
  }

  let mut presentation = MockBevyPresentation::new();
  let initial: Vec<_> = world_leaves.iter().copied().collect();
  presentation.initial_spawn(&initial);

  struct PendingRefinement {
    transitions: Vec<TransitionGroup>,
    ready_chunk_nodes: Vec<OctreeNode>,
    complete_at_frame: usize,
  }
  let mut pending: Option<PendingRefinement> = None;

  let camera_path = |frame: usize| -> DVec3 {
    let t = frame as f64 * 0.1;
    let x = (t * 0.5).sin() * 300.0;
    let y = 50.0 + (t * 0.3).sin() * 30.0;
    let z = (t * 0.7).cos() * 300.0;
    DVec3::new(x, y, z)
  };

  const NUM_FRAMES: usize = 500;
  const MAX_TEST_DURATION: Duration = Duration::from_secs(40);
  let start = Instant::now();

  for frame in 0..NUM_FRAMES {
    if start.elapsed() > MAX_TEST_DURATION {
      println!(
        "Test timeout after {} frames ({:?})",
        frame,
        start.elapsed()
      );
      break;
    }
    let viewer_pos = camera_path(frame);

    // Start refinement if not busy
    if pending.is_none() {
      let input = RefinementInput {
        viewer_pos,
        config: config.clone(),
        prev_leaves: world_leaves.clone(),
        budget: RefinementBudget::DEFAULT,
      };

      let output = refine(input);

      if !output.transition_groups.is_empty() {
        // Update leaves immediately
        for group in &output.transition_groups {
          for node in &group.nodes_to_remove {
            world_leaves.remove(node);
          }
          for node in &group.nodes_to_add {
            world_leaves.insert(*node);
          }
        }

        let ready_chunks = process_transitions(
          world_id,
          &output.transition_groups,
          &sampler,
          &world_leaves,
          &config,
        );

        // Variable delay: 1-5 frames
        let delay = 1 + (frame % 5);

        pending = Some(PendingRefinement {
          transitions: output.transition_groups,
          ready_chunk_nodes: ready_chunks.iter().map(|c| c.node).collect(),
          complete_at_frame: frame + delay,
        });
      }
    }

    // Poll only when "async" completes
    if let Some(ref p) = pending {
      if frame >= p.complete_at_frame {
        let p = pending.take().unwrap();
        presentation.apply_poll_result(frame, &world_leaves, &p.transitions, &p.ready_chunk_nodes);
        presentation.check_orphans(frame, &world_leaves);
      }
    }

    if presentation.has_violations() {
      println!("Async delay violation at frame {}", frame);
      for v in presentation.violations.iter().take(10) {
        println!("  {:?}", v);
      }
      break;
    }
  }

  assert!(
    !presentation.has_violations(),
    "Async delay test violations: {:?}",
    &presentation.violations[..presentation.violations.len().min(10)]
  );

  println!(
    "Async delay test passed: {} leaves, {} entities",
    world_leaves.len(),
    presentation.entity_count()
  );
}

/// Test that CONFIRMS ghost cells occur with async timing gaps.
///
/// This test simulates the race condition that causes ghost cells:
/// 1. Refinement 1: subdivide, update leaves, capture transitions to process
///    "async"
/// 2. Refinement 2: merge back (while "async" is pending), update leaves again
/// 3. Process the transitions from step 1 - nodes are now stale!
///
/// Without the spawn guard, this creates ghost entities.
#[test]
fn test_ghost_cells_with_async_gap() {
  let config = OctreeConfig {
    voxel_size: 1.0,
    world_origin: DVec3::new(-500.0, -100.0, -500.0),
    min_lod: 0,
    max_lod: 6,
    lod_exponent: 1.5,
    world_bounds: None,
  };

  let sampler = FastNoise2Terrain::new(1337);
  let world_id = WorldId::new();

  // Start with a single coarse node
  let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
  world_leaves.insert(OctreeNode::new(0, 0, 0, 5));

  let mut presentation = MockBevyPresentation::new();
  presentation.initial_spawn(&[OctreeNode::new(0, 0, 0, 5)]);

  println!(
    "Initial state: {} leaves, {} entities",
    world_leaves.len(),
    presentation.entity_count()
  );

  // === STEP 1: Zoom in (subdivide) ===
  let close_pos = DVec3::new(0.0, 5.0, 0.0);
  let mut subdivide_transitions = Vec::new();

  for _iter in 0..20 {
    let input = RefinementInput {
      viewer_pos: close_pos,
      config: config.clone(),
      prev_leaves: world_leaves.clone(),
      budget: RefinementBudget::UNLIMITED,
    };
    let output = refine(input);
    if output.transition_groups.is_empty() {
      break;
    }
    subdivide_transitions.extend(output.transition_groups.clone());

    // Update leaves (this happens immediately in start_refinement)
    for group in &output.transition_groups {
      for node in &group.nodes_to_remove {
        world_leaves.remove(node);
      }
      for node in &group.nodes_to_add {
        world_leaves.insert(*node);
      }
    }
  }

  let leaves_after_subdivide = world_leaves.clone();
  println!("After subdivide: {} leaves", world_leaves.len());

  // Process the subdivide transitions (simulate async completing)
  let ready_chunks = process_transitions(
    world_id,
    &subdivide_transitions,
    &sampler,
    &leaves_after_subdivide,
    &config,
  );
  let subdivide_ready_nodes: Vec<_> = ready_chunks.iter().map(|c| c.node).collect();
  println!(
    "Subdivide produced {} ready chunks",
    subdivide_ready_nodes.len()
  );

  // === STEP 2: BEFORE spawning, zoom back out (merge) ===
  // This simulates: async is in-flight, but camera moved, triggering new
  // refinement
  let far_pos = DVec3::new(0.0, 1000.0, 0.0);

  for _iter in 0..20 {
    let input = RefinementInput {
      viewer_pos: far_pos,
      config: config.clone(),
      prev_leaves: world_leaves.clone(),
      budget: RefinementBudget::UNLIMITED,
    };
    let output = refine(input);
    if output.transition_groups.is_empty() {
      break;
    }

    // Update leaves (the "second refinement" while async is pending)
    for group in &output.transition_groups {
      for node in &group.nodes_to_remove {
        world_leaves.remove(node);
      }
      for node in &group.nodes_to_add {
        world_leaves.insert(*node);
      }
    }
  }

  println!("After merge: {} leaves", world_leaves.len());

  // === STEP 3: NOW the "async" from step 1 completes ===
  // But world.leaves has changed! The ready_nodes are now stale.

  // Count how many nodes are stale (would become ghosts without guard)
  let stale_count = subdivide_ready_nodes
    .iter()
    .filter(|n| !world_leaves.contains(n))
    .count();

  println!(
    "Stale nodes (would be ghosts without guard): {}",
    stale_count
  );

  // Apply WITHOUT guard - this creates ghost cells
  presentation.apply_poll_result_no_guard(
    0,
    &world_leaves,
    &subdivide_transitions,
    &subdivide_ready_nodes,
  );

  // Check for orphans
  presentation.check_orphans(0, &world_leaves);
  let ghost_cells_detected = presentation.violations.len();

  println!("\n=== GHOST CELL CONFIRMATION TEST ===");
  println!("Ghost cells detected: {}", ghost_cells_detected);
  println!(
    "Final leaves: {}, entities: {}",
    world_leaves.len(),
    presentation.entity_count()
  );

  if ghost_cells_detected > 0 {
    println!("\nCONFIRMED: Ghost cells occur when async results are applied");
    println!("after world.leaves has changed!");
    println!("The spawn guard checking world.leaves.contains() is ESSENTIAL.\n");

    // Show some violations
    println!("Sample violations:");
    for v in presentation.violations.iter().take(5) {
      println!("  {:?}", v);
    }
  }

  // This test passes if we detect ghost cells
  assert!(
    ghost_cells_detected > 0,
    "Expected ghost cells from async timing gap. Stale count was {}. This test demonstrates why \
     the spawn guard is necessary.",
    stale_count
  );
}
