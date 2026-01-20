//! Unity FFI bridge for voxel_plugin v0.3.
//!
//! Rust owns all chunk orchestration - C# only passes viewer position and
//! applies presentation events. Key features:
//! - Proper LOD min/max controls (0..31)
//! - Rust pushes pre-calculated world positions and scales
//! - FastNoise2-rs integration as the terrain sampler
//!
//! # Architecture
//!
//! ```text
//! C# (Unity)                           Rust (voxel_unity)
//! ┌───────────────────┐                ┌─────────────────────────┐
//! │ NativeVoxelWorld  │                │ WorldState              │
//! │                   │                │  - config: OctreeConfig │
//! │ Update():         │  voxel_world_  │  - leaves: HashSet<Node>│
//! │   viewerPos ──────┼──update()────► │  - sampler: FastNoise2  │
//! │                   │                │  - pipeline: AsyncRef   │
//! │                   │ ◄──────────────│  - pending_batches      │
//! │ ApplyBatch():     │  Presentation  │                         │
//! │   Create/Remove   │  Batch         │ update():               │
//! │   GameObjects     │                │  1. Update viewer pos   │
//! └───────────────────┘                │  2. Poll pipeline       │
//!                                      │  3. Start refinement    │
//!                                      │  4. Return events       │
//!                                      └─────────────────────────┘
//! ```

use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use glam::DVec3;

use voxel_plugin::{
    noise::FastNoise2Terrain,
    octree::{DAabb3, OctreeConfig, OctreeNode, RefinementBudget},
    pipeline::{AsyncRefinementPipeline, RefinementRequest, VolumeSampler},
    types::Vertex,
    world::WorldId,
    MetaballsSampler, NormalMode,
};

// =============================================================================
// FFI Types - Phase 1
// =============================================================================

/// Chunk key for identifying chunks across FFI boundary.
/// Matches C# FfiChunkKey exactly.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FfiChunkKey {
    pub grid_x: i32,
    pub grid_y: i32,
    pub grid_z: i32,
    pub lod: u8,
    pub _pad: [u8; 3],
}

impl From<OctreeNode> for FfiChunkKey {
    fn from(node: OctreeNode) -> Self {
        Self {
            grid_x: node.x,
            grid_y: node.y,
            grid_z: node.z,
            lod: node.lod as u8,
            _pad: [0; 3],
        }
    }
}

impl From<FfiChunkKey> for OctreeNode {
    fn from(key: FfiChunkKey) -> Self {
        OctreeNode::new(key.grid_x, key.grid_y, key.grid_z, key.lod as i32)
    }
}

/// Configuration for world creation passed from Unity.
/// v0.3: Now includes LOD range and noise configuration.
#[repr(C)]
pub struct FfiWorldConfig {
    /// Seed for random/noise generation
    pub seed: i32,
    /// Base voxel size in world units
    pub voxel_size: f32,
    /// Finest LOD level (0..31, typically 0)
    pub lod_min: u8,
    /// Coarsest LOD level (0..31)
    pub lod_max: u8,
    /// Padding for alignment
    pub _pad: [u8; 2],
    /// World half-extent: defines world bounds as ±half_extent from origin
    pub world_half_extent: f32,
    /// LOD exponent: scales distance thresholds
    pub lod_exponent: f32,
    /// FastNoise2 encoded string (null = default terrain)
    pub noise_encoded: *const c_char,
}

/// Chunk presentation data with pre-calculated world position and scale.
/// Pointers are valid until the next update call or world destroy.
#[repr(C)]
pub struct FfiChunkPresentation {
    pub key: FfiChunkKey,
    /// Pre-calculated world position X
    pub world_pos_x: f64,
    /// Pre-calculated world position Y
    pub world_pos_y: f64,
    /// Pre-calculated world position Z
    pub world_pos_z: f64,
    /// Scale = voxel_size * 2^lod (for mesh vertices in voxel units)
    pub scale: f64,
    /// Pointer to vertex data
    pub vertices_ptr: *const Vertex,
    /// Number of vertices
    pub vertices_count: u32,
    /// Pointer to index data (u16 since 32³ volume has at most 32,768 vertices)
    pub indices_ptr: *const u16,
    /// Number of indices
    pub indices_count: u32,
}

/// A transition group that must be applied atomically.
/// Contains both removals and additions that belong together for visual coherence.
#[repr(C)]
pub struct FfiTransitionGroup {
    /// Group key (parent node for subdivide/merge)
    pub group_key: FfiChunkKey,
    /// True if this is a collapse (merge), false if subdivide
    pub is_collapse: u8,
    pub _pad: [u8; 3],
    /// Nodes to remove (despawn) - apply FIRST
    pub to_remove: *const FfiChunkKey,
    pub to_remove_count: u32,
    /// Chunks to add (spawn) - apply SECOND, in same frame
    pub to_add: *const FfiChunkPresentation,
    pub to_add_count: u32,
}

/// Batch of transition groups returned from update.
/// Each group must be applied atomically for visual coherence.
#[repr(C)]
pub struct FfiPresentationBatch {
    /// Transition groups to apply (each atomically)
    pub groups: *const FfiTransitionGroup,
    pub groups_count: u32,
    pub _pad: u32,
}

// SAFETY: FFI types contain raw pointers that are only valid within a single
// FFI call context. The WorldState owns all backing data, so pointers remain
// valid as long as the world is locked. These are not actually sent between
// threads - the Mutex guards ensure single-threaded access.
unsafe impl Send for FfiChunkPresentation {}
unsafe impl Sync for FfiChunkPresentation {}
unsafe impl Send for FfiTransitionGroup {}
unsafe impl Sync for FfiTransitionGroup {}
unsafe impl Send for FfiPresentationBatch {}
unsafe impl Sync for FfiPresentationBatch {}

// =============================================================================
// Legacy FFI Types (backward compatibility)
// =============================================================================

/// Legacy FFI mesh result for voxel_chunk_generate.
#[repr(C)]
pub struct FfiMeshResult {
    pub vertices_ptr: *const Vertex,
    pub vertices_count: u32,
    pub indices_ptr: *const u16,
    pub indices_count: u32,
}

/// Legacy world config for backward compatibility.
#[repr(C)]
pub struct FfiLegacyWorldConfig {
    pub seed: u32,
    pub voxel_size: f32,
    pub metaball_count: u32,
    pub metaball_extent: f32,
}

// =============================================================================
// Sampler Variants - Phase 2
// =============================================================================

/// Sampler variant for different terrain generation modes.
enum SamplerVariant {
    /// FastNoise2-based terrain (default)
    Terrain(FastNoise2Terrain),
    /// Legacy metaballs sampler
    Metaballs(MetaballsSampler),
}

impl VolumeSampler for SamplerVariant {
    fn sample_volume(
        &self,
        grid_offset: [i64; 3],
        voxel_size: f64,
        volume: &mut [i8; voxel_plugin::SAMPLE_SIZE_CB],
        materials: &mut [u8; voxel_plugin::SAMPLE_SIZE_CB],
    ) {
        match self {
            SamplerVariant::Terrain(t) => t.sample_volume(grid_offset, voxel_size, volume, materials),
            SamplerVariant::Metaballs(m) => m.sample_volume(grid_offset, voxel_size, volume, materials),
        }
    }
}

impl Clone for SamplerVariant {
    fn clone(&self) -> Self {
        match self {
            SamplerVariant::Terrain(t) => SamplerVariant::Terrain(t.clone()),
            SamplerVariant::Metaballs(m) => SamplerVariant::Metaballs(m.clone()),
        }
    }
}

// =============================================================================
// World State - Phase 2
// =============================================================================

/// Retained chunk mesh data for pointer validity across FFI boundary.
struct RetainedChunk {
    key: FfiChunkKey,
    world_pos: DVec3,
    scale: f64,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

/// Retained transition group data for pointer validity across FFI boundary.
struct RetainedTransitionGroup {
    group_key: FfiChunkKey,
    is_collapse: bool,
    /// Keys to remove (owned, for pointer stability)
    to_remove: Vec<FfiChunkKey>,
    /// Chunks to add (owned mesh data)
    to_add: Vec<RetainedChunk>,
    /// FFI presentations (built from to_add, points into to_add's data)
    presentations: Vec<FfiChunkPresentation>,
}

/// Internal state for a voxel world with Rust-driven orchestration.
struct WorldState {
    /// World identifier
    world_id: WorldId,
    /// Octree configuration (voxel size, LOD range, etc.)
    config: OctreeConfig,
    /// Volume sampler (FastNoise2 or metaballs)
    sampler: SamplerVariant,
    /// Current viewer position
    viewer_pos: DVec3,
    /// Current octree leaves
    leaves: HashSet<OctreeNode>,
    /// Async refinement + mesh pipeline
    pipeline: AsyncRefinementPipeline,
    /// Pending transition groups (retained for pointer validity)
    pending_groups: Vec<RetainedTransitionGroup>,
    /// FFI transition groups (built from pending_groups, points into their data)
    ffi_groups: Vec<FfiTransitionGroup>,
    /// Whether this is a new world needing initial population
    needs_initial_population: bool,
    /// Legacy: last generated mesh (for voxel_chunk_generate compatibility)
    last_mesh: Option<voxel_plugin::MeshOutput>,
}

/// Number of voxels per cell (interior cells, excluding boundary overlap).
const CHUNK_VOXELS: i64 = 28;

impl WorldState {
    /// Create a new world with FastNoise2 terrain.
    fn new_terrain(seed: i32, voxel_size: f64, lod_min: i32, lod_max: i32, world_half_extent: f64, lod_exponent: f64, encoded: Option<&str>) -> Self {
        let sampler = match encoded {
            Some(enc) => {
                // Leak the string to get 'static lifetime (acceptable for long-lived world)
                let static_str: &'static str = Box::leak(enc.to_string().into_boxed_str());
                SamplerVariant::Terrain(FastNoise2Terrain::with_encoded(static_str, seed))
            }
            None => SamplerVariant::Terrain(FastNoise2Terrain::new(seed)),
        };

        // Create world bounds from half-extent (centered at origin)
        // Node coordinates span negative to positive, so world_origin is ZERO
        let world_bounds = DAabb3::from_center_half_extents(
            DVec3::ZERO,
            DVec3::splat(world_half_extent),
        );

        let config = OctreeConfig {
            voxel_size,
            world_origin: DVec3::ZERO,
            min_lod: lod_min,
            max_lod: lod_max,
            lod_exponent,
            world_bounds: Some(world_bounds),
        };

        Self {
            world_id: WorldId::new(),
            config,
            sampler,
            viewer_pos: DVec3::ZERO,
            leaves: HashSet::new(),
            pipeline: AsyncRefinementPipeline::new(),
            pending_groups: Vec::new(),
            ffi_groups: Vec::new(),
            needs_initial_population: true,
            last_mesh: None,
        }
    }

    /// Create a new world with legacy metaballs sampler.
    fn new_metaballs(seed: u32, voxel_size: f32, metaball_count: usize, metaball_extent: f64) -> Self {
        let sampler = SamplerVariant::Metaballs(
            MetaballsSampler::random(seed, metaball_count, metaball_extent)
        );

        let config = OctreeConfig {
            voxel_size: voxel_size as f64,
            world_origin: DVec3::ZERO,
            min_lod: 0,
            max_lod: 8,
            lod_exponent: 1.0,
            world_bounds: None,
        };

        Self {
            world_id: WorldId::new(),
            config,
            sampler,
            viewer_pos: DVec3::ZERO,
            leaves: HashSet::new(),
            pipeline: AsyncRefinementPipeline::new(),
            pending_groups: Vec::new(),
            ffi_groups: Vec::new(),
            needs_initial_population: false, // Legacy mode uses manual chunk requests
            last_mesh: None,
        }
    }

    /// Calculate world position for a node (uses config for consistency with LOD calculations).
    fn node_world_pos(&self, node: &OctreeNode) -> DVec3 {
        self.config.get_node_min(node)
    }

    /// Calculate mesh scale for a node (voxel_size * 2^lod).
    fn node_scale(&self, node: &OctreeNode) -> f64 {
        self.config.get_voxel_size(node.lod)
    }

    /// Create initial leaves based on world bounds and suggested LOD.
    fn populate_initial_leaves(&mut self) {
        // Use config's suggested initial LOD based on world bounds
        let initial_lod = self.config.suggest_initial_lod();

        // Compute initial leaves from world bounds
        self.leaves = self.config.compute_initial_leaves(initial_lod)
            .into_iter()
            .collect();

        self.needs_initial_population = false;
    }

    /// Update world state with new viewer position.
    /// Returns true if events are ready.
    fn update(&mut self, viewer_pos: DVec3) -> bool {
        self.viewer_pos = viewer_pos;

        // Clear previous pending data
        self.pending_groups.clear();
        self.ffi_groups.clear();

        // Initial population if needed
        if self.needs_initial_population {
            self.populate_initial_leaves();
        }

        // Poll pipeline for completed results
        if let Some(result) = self.pipeline.poll_results() {
            // Process completed transitions into retained groups
            for transition in result.transitions {
                // Update leaves set
                for node in &transition.nodes_to_remove {
                    self.leaves.remove(node);
                }
                for node in &transition.nodes_to_add {
                    self.leaves.insert(*node);
                }

                // Build retained group with owned data
                let to_remove: Vec<FfiChunkKey> = transition
                    .nodes_to_remove
                    .iter()
                    .map(|n| (*n).into())
                    .collect();

                let to_add: Vec<RetainedChunk> = transition
                    .ready_chunks
                    .into_iter()
                    .map(|chunk| {
                        let node = chunk.node;
                        let world_pos = self.node_world_pos(&node);
                        let scale = self.node_scale(&node);
                        RetainedChunk {
                            key: node.into(),
                            world_pos,
                            scale,
                            vertices: chunk.output.vertices,
                            indices: chunk.output.indices,
                        }
                    })
                    .collect();

                self.pending_groups.push(RetainedTransitionGroup {
                    group_key: transition.group_key.into(),
                    is_collapse: transition.is_collapse,
                    to_remove,
                    to_add,
                    presentations: Vec::new(), // Will be built below
                });
            }

            // Build FFI presentations (must be done after all groups are stored for pointer stability)
            for group in &mut self.pending_groups {
                group.presentations = group
                    .to_add
                    .iter()
                    .map(|chunk| FfiChunkPresentation {
                        key: chunk.key,
                        world_pos_x: chunk.world_pos.x,
                        world_pos_y: chunk.world_pos.y,
                        world_pos_z: chunk.world_pos.z,
                        scale: chunk.scale,
                        vertices_ptr: chunk.vertices.as_ptr(),
                        vertices_count: chunk.vertices.len() as u32,
                        indices_ptr: chunk.indices.as_ptr(),
                        indices_count: chunk.indices.len() as u32,
                    })
                    .collect();
            }

            // Build FFI groups (points into pending_groups data)
            self.ffi_groups = self
                .pending_groups
                .iter()
                .map(|group| FfiTransitionGroup {
                    group_key: group.group_key,
                    is_collapse: if group.is_collapse { 1 } else { 0 },
                    _pad: [0; 3],
                    to_remove: if group.to_remove.is_empty() {
                        std::ptr::null()
                    } else {
                        group.to_remove.as_ptr()
                    },
                    to_remove_count: group.to_remove.len() as u32,
                    to_add: if group.presentations.is_empty() {
                        std::ptr::null()
                    } else {
                        group.presentations.as_ptr()
                    },
                    to_add_count: group.presentations.len() as u32,
                })
                .collect();

            return !self.ffi_groups.is_empty();
        }

        // If pipeline is idle and we have leaves, start new refinement
        if !self.pipeline.is_busy() && !self.leaves.is_empty() {
            let request = RefinementRequest {
                world_id: self.world_id,
                viewer_pos: self.viewer_pos,
                leaves: self.leaves.clone(),
                config: self.config.clone(),
                budget: RefinementBudget::DEFAULT,
                sampler: self.sampler.clone(),
            };
            self.pipeline.start(request);
        }

        false
    }
}

// =============================================================================
// Global World Storage
// =============================================================================

/// Global world storage with thread-safe access.
static WORLDS: Mutex<Option<HashMap<i32, WorldState>>> = Mutex::new(None);
static NEXT_WORLD_ID: AtomicI32 = AtomicI32::new(1);

/// Initialize the worlds HashMap if not already done.
fn ensure_worlds_initialized(guard: &mut Option<HashMap<i32, WorldState>>) {
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
}

// =============================================================================
// FFI Functions - Phase 3
// =============================================================================

/// Returns the library version as a packed u32: 0xMMmmpp (major.minor.patch).
#[no_mangle]
pub extern "C" fn voxel_version() -> u32 {
    0x000300 // v0.3.0
}

/// Create a new voxel world with v0.3 configuration.
///
/// # Safety
/// - `config` must point to a valid FfiWorldConfig struct.
///
/// # Returns
/// - Positive world_id on success
/// - -1 if config is null
/// - -2 if failed to acquire lock
#[no_mangle]
pub unsafe extern "C" fn voxel_world_create_v3(config: *const FfiWorldConfig) -> i32 {
    if config.is_null() {
        return -1;
    }

    let cfg = &*config;

    // Parse noise_encoded if provided
    let encoded = if cfg.noise_encoded.is_null() {
        None
    } else {
        match CStr::from_ptr(cfg.noise_encoded).to_str() {
            Ok(s) if !s.is_empty() => Some(s),
            _ => None,
        }
    };

    let state = WorldState::new_terrain(
        cfg.seed,
        cfg.voxel_size as f64,
        cfg.lod_min as i32,
        cfg.lod_max as i32,
        cfg.world_half_extent as f64,
        cfg.lod_exponent as f64,
        encoded,
    );

    let Ok(mut guard) = WORLDS.lock() else {
        return -2;
    };

    ensure_worlds_initialized(&mut guard);
    let worlds = guard.as_mut().unwrap();

    let world_id = NEXT_WORLD_ID.fetch_add(1, Ordering::SeqCst);
    worlds.insert(world_id, state);

    world_id
}

/// Update viewer position and poll for presentation events.
///
/// # Safety
/// - `out` must point to a valid FfiPresentationBatch struct.
///
/// # Parameters
/// - `world_id`: ID returned by voxel_world_create_v3
/// - `viewer_x/y/z`: Viewer position in world space
/// - `out`: Output struct to receive presentation batch
///
/// # Returns
/// - 0 = no events ready (pipeline still working)
/// - 1 = events ready (check out.groups)
/// - -1 if out is null
/// - -2 if failed to acquire lock
/// - -3 if world_id not found
#[no_mangle]
pub unsafe extern "C" fn voxel_world_update(
    world_id: i32,
    viewer_x: f64,
    viewer_y: f64,
    viewer_z: f64,
    out: *mut FfiPresentationBatch,
) -> i32 {
    if out.is_null() {
        return -1;
    }

    let Ok(mut guard) = WORLDS.lock() else {
        return -2;
    };

    let Some(ref mut worlds) = *guard else {
        return -3;
    };

    let Some(state) = worlds.get_mut(&world_id) else {
        return -3;
    };

    let viewer_pos = DVec3::new(viewer_x, viewer_y, viewer_z);
    let has_events = state.update(viewer_pos);

    if has_events {
        // Build output batch with pointers into state's retained FFI groups
        (*out) = FfiPresentationBatch {
            groups: if state.ffi_groups.is_empty() {
                std::ptr::null()
            } else {
                state.ffi_groups.as_ptr()
            },
            groups_count: state.ffi_groups.len() as u32,
            _pad: 0,
        };
        1
    } else {
        (*out) = FfiPresentationBatch {
            groups: std::ptr::null(),
            groups_count: 0,
            _pad: 0,
        };
        0
    }
}

/// Destroy a voxel world and free its resources.
///
/// # Returns
/// - 0 on success
/// - -2 if failed to acquire lock
/// - -3 if world_id not found
#[no_mangle]
pub extern "C" fn voxel_world_destroy(world_id: i32) -> i32 {
    let Ok(mut guard) = WORLDS.lock() else {
        return -2;
    };

    let Some(ref mut worlds) = *guard else {
        return -3;
    };

    if worlds.remove(&world_id).is_some() {
        0
    } else {
        -3
    }
}

// =============================================================================
// Legacy FFI Functions (backward compatibility with v0.2)
// =============================================================================

/// Create a new voxel world with legacy metaballs sampler.
/// Maintained for backward compatibility with existing Unity code.
///
/// # Safety
/// - `config` must point to a valid FfiLegacyWorldConfig struct.
#[no_mangle]
pub unsafe extern "C" fn voxel_world_create(config: *const FfiLegacyWorldConfig) -> i32 {
    if config.is_null() {
        return -1;
    }

    let cfg = &*config;

    let state = WorldState::new_metaballs(
        cfg.seed,
        cfg.voxel_size,
        cfg.metaball_count as usize,
        cfg.metaball_extent as f64,
    );

    let Ok(mut guard) = WORLDS.lock() else {
        return -2;
    };

    ensure_worlds_initialized(&mut guard);
    let worlds = guard.as_mut().unwrap();

    let world_id = NEXT_WORLD_ID.fetch_add(1, Ordering::SeqCst);
    worlds.insert(world_id, state);

    world_id
}

/// Generate mesh for a specific chunk (legacy API).
/// For backward compatibility with v0.2 - uses synchronous mesh generation.
///
/// # Safety
/// - `out` must point to a valid FfiMeshResult struct.
#[no_mangle]
pub unsafe extern "C" fn voxel_chunk_generate(
    world_id: i32,
    grid_x: i32,
    grid_y: i32,
    grid_z: i32,
    lod: i32,
    out: *mut FfiMeshResult,
) -> i32 {
    if out.is_null() {
        return -1;
    }

    let Ok(mut guard) = WORLDS.lock() else {
        return -2;
    };

    let Some(ref mut worlds) = *guard else {
        return -3;
    };

    let Some(state) = worlds.get_mut(&world_id) else {
        return -3;
    };

    // Synchronous mesh generation for legacy API
    let lod_scale = 1i64 << lod;
    let grid_offset = [
        (grid_x as i64) * CHUNK_VOXELS * lod_scale,
        (grid_y as i64) * CHUNK_VOXELS * lod_scale,
        (grid_z as i64) * CHUNK_VOXELS * lod_scale,
    ];

    let effective_voxel_size = state.config.voxel_size * (lod_scale as f64);

    // Sample volume
    let mut volume = Box::new([0i8; voxel_plugin::SAMPLE_SIZE_CB]);
    let mut materials = Box::new([0u8; voxel_plugin::SAMPLE_SIZE_CB]);

    state.sampler.sample_volume(grid_offset, effective_voxel_size, &mut *volume, &mut *materials);

    // Generate mesh
    let config = voxel_plugin::MeshConfig {
        voxel_size: effective_voxel_size as f32,
        neighbor_mask: 0,
        normal_mode: NormalMode::InterpolatedGradient,
        use_microsplat_encoding: false,
    };

    let output = voxel_plugin::surface_nets::generate(&*volume, &*materials, &config);

    (*out) = FfiMeshResult {
        vertices_ptr: output.vertices.as_ptr(),
        vertices_count: output.vertices.len() as u32,
        indices_ptr: output.indices.as_ptr(),
        indices_count: output.indices.len() as u32,
    };

    state.last_mesh = Some(output);

    0
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(voxel_version(), 0x000300);
    }

    #[test]
    fn test_ffi_chunk_key_conversion() {
        let node = OctreeNode::new(1, 2, 3, 4);
        let key: FfiChunkKey = node.into();

        assert_eq!(key.grid_x, 1);
        assert_eq!(key.grid_y, 2);
        assert_eq!(key.grid_z, 3);
        assert_eq!(key.lod, 4);

        let back: OctreeNode = key.into();
        assert_eq!(back, node);
    }

    #[test]
    fn test_legacy_world_lifecycle() {
        let config = FfiLegacyWorldConfig {
            seed: 42,
            voxel_size: 1.0,
            metaball_count: 5,
            metaball_extent: 30.0,
        };

        unsafe {
            let world_id = voxel_world_create(&config);
            assert!(world_id > 0, "Expected positive world_id, got {}", world_id);

            let mut result = FfiMeshResult {
                vertices_ptr: std::ptr::null(),
                vertices_count: 0,
                indices_ptr: std::ptr::null(),
                indices_count: 0,
            };

            let status = voxel_chunk_generate(world_id, 0, 0, 0, 0, &mut result);
            assert_eq!(status, 0, "Generate should succeed");

            let status = voxel_world_destroy(world_id);
            assert_eq!(status, 0, "Destroy should succeed");

            let status = voxel_world_destroy(world_id);
            assert_eq!(status, -3, "Double destroy should return -3");
        }
    }

    #[test]
    fn test_v3_world_create() {
        let config = FfiWorldConfig {
            seed: 42,
            voxel_size: 1.0,
            lod_min: 0,
            lod_max: 8,
            _pad: [0; 2],
            world_half_extent: 500.0,
            lod_exponent: 1.0,
            noise_encoded: std::ptr::null(),
        };

        unsafe {
            let world_id = voxel_world_create_v3(&config);
            assert!(world_id > 0, "Expected positive world_id, got {}", world_id);

            let status = voxel_world_destroy(world_id);
            assert_eq!(status, 0, "Destroy should succeed");
        }
    }

    #[test]
    fn test_v3_world_update() {
        let config = FfiWorldConfig {
            seed: 123,
            voxel_size: 1.0,
            lod_min: 0,
            lod_max: 4,
            _pad: [0; 2],
            world_half_extent: 100.0,
            lod_exponent: 1.0,
            noise_encoded: std::ptr::null(),
        };

        unsafe {
            let world_id = voxel_world_create_v3(&config);
            assert!(world_id > 0);

            let mut batch = FfiPresentationBatch {
                groups: std::ptr::null(),
                groups_count: 0,
                _pad: 0,
            };

            // First update - should start pipeline
            let status = voxel_world_update(world_id, 0.0, 0.0, 0.0, &mut batch);
            assert!(status >= 0, "Update should not fail");

            // Multiple updates - eventually pipeline should complete
            for _ in 0..100 {
                let status = voxel_world_update(world_id, 0.0, 0.0, 0.0, &mut batch);
                if status == 1 {
                    // Events ready - verify batch structure
                    assert!(batch.groups_count > 0 || batch.groups.is_null());
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            voxel_world_destroy(world_id);
        }
    }
}
