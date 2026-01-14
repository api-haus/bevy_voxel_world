//! Consistency test for refinement + presentation pipeline.
//!
//! Simulates a chaotic camera path and verifies that presented nodes
//! always remain consistent with world.leaves.

use std::collections::HashSet;

use simdnoise::NoiseBuilder;

use crate::constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB};
use crate::octree::{refine, OctreeConfig, OctreeNode, RefinementBudget, RefinementInput};
use crate::pipeline::process::process_transitions;
use crate::pipeline::types::VolumeSampler;
use crate::types::{sdf_conversion, MaterialId, SdfSample};
use crate::world::WorldId;

// =============================================================================
// TerrainWithCaves sampler - exact copy from voxel_bevy::noise
// =============================================================================

/// Terrain sampler that produces a height-based surface with worm tunnel caves.
/// Exact copy of voxel_bevy::noise::TerrainWithCaves for test independence.
#[derive(Clone)]
struct TerrainWithCavesTestSampler {
    seed: i32,
    terrain_frequency: f32,
    terrain_amplitude: f32,
    terrain_base_height: f32,
    terrain_octaves: u8,
    cave_frequency: f32,
    cave_threshold: f32,
    cave_octaves: u8,
    warp_frequency: f32,
    warp_strength: f32,
}

impl TerrainWithCavesTestSampler {
    fn new(seed: i32) -> Self {
        Self {
            seed,
            terrain_frequency: 0.002,
            terrain_amplitude: 50.0,
            terrain_base_height: 0.0,
            terrain_octaves: 5,
            cave_frequency: 0.015,
            cave_threshold: 0.3,
            cave_octaves: 3,
            warp_frequency: 0.008,
            warp_strength: 15.0,
        }
    }
}

impl VolumeSampler for TerrainWithCavesTestSampler {
    fn sample_volume(
        &self,
        sample_start: [f64; 3],
        voxel_size: f64,
        volume: &mut [SdfSample; SAMPLE_SIZE_CB],
        materials: &mut [MaterialId; SAMPLE_SIZE_CB],
    ) {
        const SIZE: usize = SAMPLE_SIZE;
        let vs = voxel_size as f32;

        let start_x = sample_start[0] as f32;
        let start_y = sample_start[1] as f32;
        let start_z = sample_start[2] as f32;

        // Heightmap
        let heightmap = NoiseBuilder::fbm_2d_offset(start_x / vs, SIZE, start_z / vs, SIZE)
            .with_seed(self.seed)
            .with_freq(vs * self.terrain_frequency)
            .with_octaves(self.terrain_octaves)
            .with_gain(0.5)
            .with_lacunarity(2.0)
            .generate()
            .0;

        // Domain warp
        let warp_x = NoiseBuilder::fbm_3d_offset(
            start_x / vs,
            SIZE,
            start_y / vs,
            SIZE,
            start_z / vs,
            SIZE,
        )
        .with_seed(self.seed + 100)
        .with_freq(vs * self.warp_frequency)
        .with_octaves(2)
        .with_gain(0.5)
        .with_lacunarity(2.0)
        .generate()
        .0;

        let warp_y = NoiseBuilder::fbm_3d_offset(
            start_x / vs,
            SIZE,
            start_y / vs,
            SIZE,
            start_z / vs,
            SIZE,
        )
        .with_seed(self.seed + 200)
        .with_freq(vs * self.warp_frequency)
        .with_octaves(2)
        .with_gain(0.5)
        .with_lacunarity(2.0)
        .generate()
        .0;

        let warp_z = NoiseBuilder::fbm_3d_offset(
            start_x / vs,
            SIZE,
            start_y / vs,
            SIZE,
            start_z / vs,
            SIZE,
        )
        .with_seed(self.seed + 300)
        .with_freq(vs * self.warp_frequency)
        .with_octaves(2)
        .with_gain(0.5)
        .with_lacunarity(2.0)
        .generate()
        .0;

        // Cave noise
        let cave_noise = NoiseBuilder::fbm_3d_offset(
            start_x / vs,
            SIZE,
            start_y / vs,
            SIZE,
            start_z / vs,
            SIZE,
        )
        .with_seed(self.seed + 1000)
        .with_freq(vs * self.cave_frequency)
        .with_octaves(self.cave_octaves)
        .with_gain(0.5)
        .with_lacunarity(2.0)
        .generate()
        .0;

        // Combine
        for idx in 0..SAMPLE_SIZE_CB {
            let x = idx / (SIZE * SIZE);
            let yz = idx % (SIZE * SIZE);
            let y = yz / SIZE;
            let z = yz % SIZE;

            let sn3d_idx = z * SIZE * SIZE + y * SIZE + x;
            let sn2d_idx = z * SIZE + x;

            let world_y = start_y + y as f32 * vs;
            let height = self.terrain_base_height + heightmap[sn2d_idx] * self.terrain_amplitude;
            let terrain_sdf = world_y - height;

            let warp_magnitude = (warp_x[sn3d_idx].powi(2)
                + warp_y[sn3d_idx].powi(2)
                + warp_z[sn3d_idx].powi(2))
            .sqrt();
            let local_threshold = self.cave_threshold + warp_magnitude * 0.1;
            let cave_sdf = cave_noise[sn3d_idx] - local_threshold;

            let final_sdf = terrain_sdf.max(cave_sdf);
            volume[idx] = sdf_conversion::to_storage(final_sdf);
            materials[idx] = 0;
        }
    }
}

// =============================================================================
// Mock presentation layer
// =============================================================================

/// Mock presentation layer that tracks which nodes are "visible" (have entities).
struct MockPresentationLayer {
    /// Nodes that have been presented (spawned as entities).
    presented: HashSet<OctreeNode>,
    /// History of consistency violations for debugging.
    violations: Vec<ConsistencyViolation>,
}

#[derive(Debug, Clone)]
struct ConsistencyViolation {
    frame: usize,
    violation_type: ViolationType,
    node: OctreeNode,
}

#[derive(Debug, Clone)]
enum ViolationType {
    /// Node is presented but not in world.leaves (orphan entity)
    OrphanPresented,
    /// Node spawned from ready_chunks but not in world.leaves
    SpawnedOrphan,
    /// Duplicate spawn attempt
    DuplicateSpawn,
}

impl MockPresentationLayer {
    fn new() -> Self {
        Self {
            presented: HashSet::new(),
            violations: Vec::new(),
        }
    }

    /// Apply transitions: despawn nodes_to_remove, spawn from ready_chunks.
    fn apply_transitions(
        &mut self,
        frame: usize,
        world_leaves: &HashSet<OctreeNode>,
        nodes_to_remove: &[OctreeNode],
        nodes_to_spawn: &[OctreeNode],
    ) {
        // Despawn
        for node in nodes_to_remove {
            self.presented.remove(node);
        }

        // Spawn (with consistency checks)
        for node in nodes_to_spawn {
            // Check: is this node in world.leaves?
            if !world_leaves.contains(node) {
                self.violations.push(ConsistencyViolation {
                    frame,
                    violation_type: ViolationType::SpawnedOrphan,
                    node: *node,
                });
            }

            // Check: is this a duplicate?
            if self.presented.contains(node) {
                self.violations.push(ConsistencyViolation {
                    frame,
                    violation_type: ViolationType::DuplicateSpawn,
                    node: *node,
                });
            }

            self.presented.insert(*node);
        }
    }

    /// Check for orphans: presented nodes not in world.leaves.
    fn check_orphans(&mut self, frame: usize, world_leaves: &HashSet<OctreeNode>) {
        for node in &self.presented {
            if !world_leaves.contains(node) {
                self.violations.push(ConsistencyViolation {
                    frame,
                    violation_type: ViolationType::OrphanPresented,
                    node: *node,
                });
            }
        }
    }

    fn has_violations(&self) -> bool {
        !self.violations.is_empty()
    }

    fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

/// Test sampler that creates a surface at y=0.
#[derive(Clone)]
struct TestTerrainSampler;

impl VolumeSampler for TestTerrainSampler {
    fn sample_volume(
        &self,
        sample_start: [f64; 3],
        voxel_size: f64,
        volume: &mut [i8; SAMPLE_SIZE_CB],
        materials: &mut [u8; SAMPLE_SIZE_CB],
    ) {
        for x in 0..32 {
            for y in 0..32 {
                for z in 0..32 {
                    let idx = x * 32 * 32 + y * 32 + z;
                    let world_y = sample_start[1] + (y as f64) * voxel_size;
                    // Positive = air, negative = solid
                    volume[idx] = if world_y < 0.0 { -127 } else { 127 };
                    materials[idx] = 1;
                }
            }
        }
    }
}

/// Generate a chaotic camera path within bounds.
fn chaotic_camera_path(frame: usize, bounds: f64) -> [f64; 3] {
    let t = frame as f64 * 0.1;

    // Lissajous-like curve with varying frequencies
    let x = (t * 0.7).sin() * (t * 0.3).cos() * bounds * 0.8;
    let y = (t * 0.5).sin() * bounds * 0.3 + bounds * 0.2; // Stay above ground
    let z = (t * 0.9).cos() * (t * 0.4).sin() * bounds * 0.8;

    [x, y, z]
}

#[test]
fn test_refinement_presentation_consistency() {
    let config = OctreeConfig {
        voxel_size: 1.0,
        world_origin: glam::DVec3::new(-100.0, -50.0, -100.0),
        min_lod: 0,
        max_lod: 5,
        lod_exponent: 1.5,
    };

    let sampler = TestTerrainSampler;
    let world_id = WorldId::new();

    // Initialize world.leaves with a coarse grid
    let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
    let initial_lod = 4;
    for x in -2..=2 {
        for y in -2..=2 {
            for z in -2..=2 {
                world_leaves.insert(OctreeNode::new(x, y, z, initial_lod));
            }
        }
    }

    // Initialize presentation layer with initial meshes
    let mut presentation = MockPresentationLayer::new();

    // Initial setup: mesh all leaves and present non-empty ones
    let initial_nodes: Vec<_> = world_leaves.iter().copied().collect();
    for node in &initial_nodes {
        // Simulate meshing - only present if would have geometry
        let node_min = config.get_node_min(node);
        if node_min.y < 10.0 && node_min.y > -50.0 {
            // Near surface, would have mesh
            presentation.presented.insert(*node);
        }
    }

    println!("Initial: {} leaves, {} presented", world_leaves.len(), presentation.presented.len());

    // Run simulation for many frames
    const NUM_FRAMES: usize = 500;
    let bounds = 80.0;

    for frame in 0..NUM_FRAMES {
        let [vx, vy, vz] = chaotic_camera_path(frame, bounds);
        let viewer_pos = glam::DVec3::new(vx, vy, vz);

        // Run refinement
        let input = RefinementInput {
            viewer_pos,
            config: config.clone(),
            prev_leaves: world_leaves.clone(),
            budget: RefinementBudget::DEFAULT,
        };

        let output = refine(input);

        if output.transition_groups.is_empty() {
            // No changes needed, but still check consistency
            presentation.check_orphans(frame, &world_leaves);
            continue;
        }

        // Collect nodes to remove and add
        let mut nodes_to_remove: Vec<OctreeNode> = Vec::new();
        let mut nodes_to_add: Vec<OctreeNode> = Vec::new();

        for group in &output.transition_groups {
            nodes_to_remove.extend(group.nodes_to_remove.iter().copied());
            nodes_to_add.extend(group.nodes_to_add.iter().copied());
        }

        // Update world.leaves (this happens in start_refinement)
        for node in &nodes_to_remove {
            world_leaves.remove(node);
        }
        for node in &nodes_to_add {
            world_leaves.insert(*node);
        }

        // Process through pipeline to get ready_chunks
        let ready_chunks = process_transitions(
            world_id,
            &output.transition_groups,
            &sampler,
            &world_leaves,
            &config,
        );

        // Extract spawned nodes from ready_chunks
        let spawned_nodes: Vec<OctreeNode> = ready_chunks.iter().map(|c| c.node).collect();

        // Apply to presentation layer (this happens in poll_async_refinement)
        presentation.apply_transitions(frame, &world_leaves, &nodes_to_remove, &spawned_nodes);

        // Check for orphans after this frame
        presentation.check_orphans(frame, &world_leaves);

        // Early exit if we have violations
        if presentation.has_violations() {
            println!("Frame {}: Found {} violations!", frame, presentation.violation_count());
            for v in &presentation.violations {
                println!("  {:?}", v);
            }
            break;
        }
    }

    // Final report
    println!(
        "After {} frames: {} leaves, {} presented, {} violations",
        NUM_FRAMES,
        world_leaves.len(),
        presentation.presented.len(),
        presentation.violation_count()
    );

    // The test passes if there are no violations
    assert!(
        !presentation.has_violations(),
        "Found {} consistency violations: {:?}",
        presentation.violation_count(),
        &presentation.violations[..presentation.violations.len().min(10)]
    );
}

#[test]
fn test_rapid_zoom_in_out() {
    // Specifically test rapid zoom in/out which is most likely to cause issues
    let config = OctreeConfig {
        voxel_size: 1.0,
        world_origin: glam::DVec3::new(-50.0, -25.0, -50.0),
        min_lod: 0,
        max_lod: 4,
        lod_exponent: 1.5,
    };

    let sampler = TestTerrainSampler;
    let world_id = WorldId::new();

    let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
    world_leaves.insert(OctreeNode::new(0, 0, 0, 3));

    let mut presentation = MockPresentationLayer::new();
    presentation.presented.insert(OctreeNode::new(0, 0, 0, 3));

    // Alternate between very close and very far
    let positions = [
        glam::DVec3::new(0.0, 5.0, 0.0),   // Close - should subdivide
        glam::DVec3::new(0.0, 500.0, 0.0), // Far - should merge
        glam::DVec3::new(0.0, 5.0, 0.0),   // Close again
        glam::DVec3::new(0.0, 500.0, 0.0), // Far again
        glam::DVec3::new(0.0, 5.0, 0.0),
        glam::DVec3::new(0.0, 500.0, 0.0),
    ];

    for (frame, &viewer_pos) in positions.iter().enumerate() {
        // Run refinement until stable at this position
        for _iteration in 0..10 {
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

            let mut nodes_to_remove: Vec<OctreeNode> = Vec::new();
            let mut nodes_to_add: Vec<OctreeNode> = Vec::new();

            for group in &output.transition_groups {
                nodes_to_remove.extend(group.nodes_to_remove.iter().copied());
                nodes_to_add.extend(group.nodes_to_add.iter().copied());
            }

            for node in &nodes_to_remove {
                world_leaves.remove(node);
            }
            for node in &nodes_to_add {
                world_leaves.insert(*node);
            }

            let ready_chunks = process_transitions(
                world_id,
                &output.transition_groups,
                &sampler,
                &world_leaves,
                &config,
            );

            let spawned_nodes: Vec<OctreeNode> = ready_chunks.iter().map(|c| c.node).collect();
            presentation.apply_transitions(frame, &world_leaves, &nodes_to_remove, &spawned_nodes);
            presentation.check_orphans(frame, &world_leaves);
        }

        println!(
            "Position {}: {} leaves, {} presented",
            frame,
            world_leaves.len(),
            presentation.presented.len()
        );
    }

    assert!(
        !presentation.has_violations(),
        "Rapid zoom violations: {:?}",
        &presentation.violations
    );
}

/// Test that simulates the ASYNC timing gap more accurately.
///
/// In the real system:
/// - Frame N: world.leaves updated, async starts
/// - Frame N+1 to N+K: world.leaves may be queried for OTHER purposes
/// - Frame N+K: ready_chunks arrive, entities spawned
///
/// The key question: can anything in between cause a desync?
#[test]
fn test_async_timing_gap_simulation() {
    let config = OctreeConfig {
        voxel_size: 1.0,
        world_origin: glam::DVec3::new(-50.0, -25.0, -50.0),
        min_lod: 0,
        max_lod: 4,
        lod_exponent: 1.5,
    };

    let sampler = TestTerrainSampler;
    let world_id = WorldId::new();

    // Initial state
    let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
    let initial_node = OctreeNode::new(0, 0, 0, 3);
    world_leaves.insert(initial_node);

    let mut presentation = MockPresentationLayer::new();
    presentation.presented.insert(initial_node);

    // Simulate async refinement with timing gap
    struct PendingRefinement {
        nodes_to_remove: Vec<OctreeNode>,
        nodes_to_spawn: Vec<OctreeNode>,
    }

    let mut pending: Option<PendingRefinement> = None;

    // Frame 0: Start refinement (zoom in)
    let viewer_pos = glam::DVec3::new(0.0, 5.0, 0.0);
    {
        let input = RefinementInput {
            viewer_pos,
            config: config.clone(),
            prev_leaves: world_leaves.clone(),
            budget: RefinementBudget::UNLIMITED,
        };

        let output = refine(input);

        if !output.transition_groups.is_empty() {
            let mut nodes_to_remove = Vec::new();
            let mut nodes_to_add = Vec::new();

            for group in &output.transition_groups {
                nodes_to_remove.extend(group.nodes_to_remove.iter().copied());
                nodes_to_add.extend(group.nodes_to_add.iter().copied());
            }

            // UPDATE world.leaves IMMEDIATELY (like start_refinement does)
            for node in &nodes_to_remove {
                world_leaves.remove(node);
            }
            for node in &nodes_to_add {
                world_leaves.insert(*node);
            }

            // Process to get ready_chunks (this would happen on background thread)
            let ready_chunks = process_transitions(
                world_id,
                &output.transition_groups,
                &sampler,
                &world_leaves,
                &config,
            );

            // Store pending (would be stored in AsyncRefinementState)
            pending = Some(PendingRefinement {
                nodes_to_remove,
                nodes_to_spawn: ready_chunks.iter().map(|c| c.node).collect(),
            });
        }
    }

    println!("After start: world.leaves={}, presented={}", world_leaves.len(), presentation.presented.len());

    // Frame 1-4: Simulate frames passing
    // During this gap, presentation IS inconsistent with world.leaves - this is EXPECTED for async
    // The old parent is still "presented" but was removed from world.leaves
    // We do NOT check for orphans here - that would be a false positive

    // Frame 5: Poll completes, apply to presentation
    if let Some(p) = pending.take() {
        presentation.apply_transitions(5, &world_leaves, &p.nodes_to_remove, &p.nodes_to_spawn);
    }

    println!("After poll: world.leaves={}, presented={}", world_leaves.len(), presentation.presented.len());

    // NOW check for orphans - AFTER poll has completed, consistency should be restored
    presentation.check_orphans(100, &world_leaves);

    // Check that ALL presented nodes are in world.leaves
    for node in &presentation.presented {
        assert!(
            world_leaves.contains(node),
            "Orphan presented node AFTER poll: {:?}",
            node
        );
    }

    assert!(
        !presentation.has_violations(),
        "Post-poll violations: {:?}",
        &presentation.violations
    );
}

/// Test that verifies the EXACT sequence of nodes_to_mesh matches what gets spawned.
#[test]
fn test_nodes_to_mesh_matches_ready_chunks() {
    use crate::octree::TransitionType;

    let config = OctreeConfig::default();
    let sampler = TestTerrainSampler;
    let world_id = WorldId::new();

    // Create a subdivide transition
    let parent = OctreeNode::new(0, 0, 0, 2);
    let transition = crate::octree::TransitionGroup::new_subdivide(parent).unwrap();

    // What nodes_to_mesh would be for this transition
    let nodes_to_mesh: Vec<OctreeNode> = match transition.transition_type {
        TransitionType::Subdivide => transition.nodes_to_add.iter().copied().collect(),
        TransitionType::Merge => vec![transition.group_key],
    };

    println!("nodes_to_mesh: {:?}", nodes_to_mesh);
    assert_eq!(nodes_to_mesh.len(), 8, "Subdivide should mesh 8 children");

    // Set up leaves as the children (post-transition state)
    let leaves: HashSet<OctreeNode> = nodes_to_mesh.iter().copied().collect();

    // Process
    let ready_chunks = process_transitions(
        world_id,
        &[transition],
        &sampler,
        &leaves,
        &config,
    );

    // Every node in ready_chunks should be from nodes_to_mesh
    for chunk in &ready_chunks {
        assert!(
            nodes_to_mesh.contains(&chunk.node),
            "ready_chunk node {:?} not in nodes_to_mesh",
            chunk.node
        );
    }

    // Every node in ready_chunks should be in leaves
    for chunk in &ready_chunks {
        assert!(
            leaves.contains(&chunk.node),
            "ready_chunk node {:?} not in leaves",
            chunk.node
        );
    }

    println!("ready_chunks: {} nodes", ready_chunks.len());
}

// =============================================================================
// END-TO-END TEST: Exact match of noise_lod scene
// =============================================================================

/// End-to-end consistency test matching noise_lod scene EXACTLY.
///
/// Config: voxel_size=1.0, origin=(-500,-100,-500), lod 0-6, exponent 1.5
/// Sampler: TerrainWithCaves(seed=1337)
/// Camera: Chaotic path within world bounds
///
/// This test runs the FULL pipeline:
/// 1. Refinement (compute transitions)
/// 2. Update world.leaves (synchronous)
/// 3. Presample + Meshing (via process_transitions)
/// 4. Presentation (apply to mock layer)
/// 5. Consistency check (no orphans after poll)
#[test]
fn test_end_to_end_noise_lod_scene() {
    // EXACT config from noise_lod.rs
    let config = OctreeConfig {
        voxel_size: 1.0,
        world_origin: glam::DVec3::new(-500.0, -100.0, -500.0),
        min_lod: 0,
        max_lod: 6,
        lod_exponent: 1.5,
    };

    // EXACT sampler from noise_lod.rs
    let sampler = TerrainWithCavesTestSampler::new(1337);
    let world_id = WorldId::new();

    // Initialize with coarse grid (like initial setup in noise_lod)
    // Start at LOD 5 to give room for subdivision
    let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
    let initial_lod = 5;
    for x in -4..=4 {
        for y in -2..=2 {
            for z in -4..=4 {
                world_leaves.insert(OctreeNode::new(x, y, z, initial_lod));
            }
        }
    }

    // Initialize presentation with nodes that have surfaces
    let mut presentation = MockPresentationLayer::new();

    // Pre-mesh initial nodes and present those with geometry
    let initial_chunks = {
        let initial_nodes: Vec<_> = world_leaves.iter().copied().collect();
        let mut groups = Vec::new();
        for node in &initial_nodes {
            // Create fake "appeared" transitions for initial meshing
            groups.push(crate::octree::TransitionGroup {
                transition_type: crate::octree::TransitionType::Subdivide,
                group_key: node.get_parent(config.max_lod).unwrap_or(*node),
                nodes_to_add: smallvec::smallvec![*node],
                nodes_to_remove: smallvec::SmallVec::new(),
            });
        }
        process_transitions(world_id, &groups, &sampler, &world_leaves, &config)
    };

    for chunk in &initial_chunks {
        presentation.presented.insert(chunk.node);
    }

    println!(
        "Initial: {} leaves, {} presented (with mesh)",
        world_leaves.len(),
        presentation.presented.len()
    );

    // Simulate async state (pending refinement)
    struct PendingRefinement {
        nodes_to_remove: Vec<OctreeNode>,
        ready_chunks_nodes: Vec<OctreeNode>,
    }
    let mut pending: Option<PendingRefinement> = None;

    // Run simulation for many frames
    const NUM_FRAMES: usize = 1000;

    // Camera path that stays within world bounds (-500 to 500 in XZ, -100 to 100 in Y)
    let camera_path = |frame: usize| -> glam::DVec3 {
        let t = frame as f64 * 0.05;

        // Lissajous curve within bounds
        let x = (t * 0.7).sin() * (t * 0.23).cos() * 400.0;
        let y = (t * 0.31).sin() * 80.0; // Stay within -100 to +100
        let z = (t * 0.53).cos() * (t * 0.17).sin() * 400.0;

        // Add rapid zoom component (height oscillation)
        let zoom = ((t * 2.0).sin() * 0.5 + 0.5) * 50.0 + 10.0;
        glam::DVec3::new(x, y + zoom, z)
    };

    let mut total_transitions = 0;
    let mut frames_with_work = 0;

    for frame in 0..NUM_FRAMES {
        let viewer_pos = camera_path(frame);

        // =====================================================================
        // STEP 1: If busy (pending != None), skip refinement start
        // =====================================================================
        if pending.is_none() {
            let input = RefinementInput {
                viewer_pos,
                config: config.clone(),
                prev_leaves: world_leaves.clone(),
                budget: RefinementBudget::DEFAULT,
            };

            let output = refine(input);

            if !output.transition_groups.is_empty() {
                frames_with_work += 1;
                total_transitions += output.transition_groups.len();

                // Collect nodes to remove and add
                let mut nodes_to_remove: Vec<OctreeNode> = Vec::new();
                let mut nodes_to_add: Vec<OctreeNode> = Vec::new();

                for group in &output.transition_groups {
                    nodes_to_remove.extend(group.nodes_to_remove.iter().copied());
                    nodes_to_add.extend(group.nodes_to_add.iter().copied());
                }

                // =====================================================================
                // STEP 2: Update world.leaves IMMEDIATELY (like start_refinement)
                // =====================================================================
                for node in &nodes_to_remove {
                    world_leaves.remove(node);
                }
                for node in &nodes_to_add {
                    world_leaves.insert(*node);
                }

                // =====================================================================
                // STEP 3: Process through full pipeline (presample + meshing)
                // =====================================================================
                let ready_chunks = process_transitions(
                    world_id,
                    &output.transition_groups,
                    &sampler,
                    &world_leaves,
                    &config,
                );

                // Store pending (simulates async gap)
                pending = Some(PendingRefinement {
                    nodes_to_remove,
                    ready_chunks_nodes: ready_chunks.iter().map(|c| c.node).collect(),
                });
            }
        }

        // =====================================================================
        // STEP 4: Simulate poll completing (every frame for simplicity)
        // In real system this happens after async completes
        // =====================================================================
        if let Some(p) = pending.take() {
            // Apply to presentation layer
            presentation.apply_transitions(
                frame,
                &world_leaves,
                &p.nodes_to_remove,
                &p.ready_chunks_nodes,
            );

            // Check for orphans AFTER applying this frame's transitions
            presentation.check_orphans(frame, &world_leaves);
        }

        // Early exit on violations
        if presentation.has_violations() {
            println!(
                "\n=== VIOLATION at frame {} ===",
                frame
            );
            println!("Viewer: {:?}", viewer_pos);
            println!("World leaves: {}", world_leaves.len());
            println!("Presented: {}", presentation.presented.len());
            println!("Violations ({}):", presentation.violations.len());
            for v in presentation.violations.iter().take(20) {
                println!("  {:?}", v);
            }
            break;
        }

        // Progress report every 100 frames
        if frame % 100 == 0 && frame > 0 {
            println!(
                "Frame {}: {} leaves, {} presented, {} transitions total",
                frame,
                world_leaves.len(),
                presentation.presented.len(),
                total_transitions
            );
        }
    }

    // Final report
    println!(
        "\n=== FINAL REPORT ===\nFrames: {}\nFrames with work: {}\nTotal transitions: {}\nFinal leaves: {}\nFinal presented: {}\nViolations: {}",
        NUM_FRAMES,
        frames_with_work,
        total_transitions,
        world_leaves.len(),
        presentation.presented.len(),
        presentation.violation_count()
    );

    // The test passes if there are no violations
    assert!(
        !presentation.has_violations(),
        "End-to-end test found {} violations. First 10: {:?}",
        presentation.violation_count(),
        &presentation.violations[..presentation.violations.len().min(10)]
    );
}

/// Stress test: Rapid zoom in/out with full noise_lod config.
#[test]
fn test_stress_rapid_zoom_noise_lod() {
    let config = OctreeConfig {
        voxel_size: 1.0,
        world_origin: glam::DVec3::new(-500.0, -100.0, -500.0),
        min_lod: 0,
        max_lod: 6,
        lod_exponent: 1.5,
    };

    let sampler = TerrainWithCavesTestSampler::new(1337);
    let world_id = WorldId::new();

    // Start with single coarse node
    let mut world_leaves: HashSet<OctreeNode> = HashSet::new();
    world_leaves.insert(OctreeNode::new(0, 0, 0, 5));

    let mut presentation = MockPresentationLayer::new();
    presentation.presented.insert(OctreeNode::new(0, 0, 0, 5));

    // Alternate between very close and very far - 50 cycles
    for cycle in 0..50 {
        for &distance in &[5.0, 1000.0] {
            let viewer_pos = glam::DVec3::new(0.0, distance, 0.0);

            // Refine until stable at this position
            for _iter in 0..20 {
                let input = RefinementInput {
                    viewer_pos,
                    config: config.clone(),
                    prev_leaves: world_leaves.clone(),
                    budget: RefinementBudget::UNLIMITED, // No budget limit for stress test
                };

                let output = refine(input);

                if output.transition_groups.is_empty() {
                    break;
                }

                let mut nodes_to_remove = Vec::new();
                let mut nodes_to_add = Vec::new();

                for group in &output.transition_groups {
                    nodes_to_remove.extend(group.nodes_to_remove.iter().copied());
                    nodes_to_add.extend(group.nodes_to_add.iter().copied());
                }

                for node in &nodes_to_remove {
                    world_leaves.remove(node);
                }
                for node in &nodes_to_add {
                    world_leaves.insert(*node);
                }

                let ready_chunks = process_transitions(
                    world_id,
                    &output.transition_groups,
                    &sampler,
                    &world_leaves,
                    &config,
                );

                let spawned: Vec<_> = ready_chunks.iter().map(|c| c.node).collect();
                presentation.apply_transitions(cycle * 100, &world_leaves, &nodes_to_remove, &spawned);
                presentation.check_orphans(cycle * 100, &world_leaves);

                if presentation.has_violations() {
                    println!("Violation at cycle {} distance {}", cycle, distance);
                    for v in &presentation.violations {
                        println!("  {:?}", v);
                    }
                    panic!("Stress test violation");
                }
            }
        }

        if cycle % 10 == 0 {
            println!(
                "Cycle {}: {} leaves, {} presented",
                cycle,
                world_leaves.len(),
                presentation.presented.len()
            );
        }
    }

    assert!(
        !presentation.has_violations(),
        "Stress test violations: {:?}",
        &presentation.violations
    );

    println!(
        "Stress test passed: {} leaves, {} presented",
        world_leaves.len(),
        presentation.presented.len()
    );
}
