//! Unity FFI bridge for voxel_plugin.
//!
//! Provides a C-compatible interface for voxel mesh generation from Unity via P/Invoke.
//! Rust handles volume generation internally using metaballs - Unity only requests meshes
//! by grid coordinates, keeping volume data inside Rust.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use voxel_plugin::{
    pipeline::VolumeSampler, surface_nets, MeshConfig, MeshOutput, MetaballsSampler, NormalMode,
    Vertex, SAMPLE_SIZE_CB,
};

// =============================================================================
// FFI Types
// =============================================================================

/// FFI-safe mesh result containing raw pointers to vertex and index data.
/// Pointers are valid until the next generate call for the same world,
/// or until the world is destroyed.
#[repr(C)]
pub struct FfiMeshResult {
    pub vertices_ptr: *const Vertex,
    pub vertices_count: u32,
    pub indices_ptr: *const u32,
    pub indices_count: u32,
}

/// Configuration for world creation passed from Unity.
#[repr(C)]
pub struct FfiWorldConfig {
    /// Seed for random generation (deterministic metaballs)
    pub seed: u32,
    /// Base voxel size in world units
    pub voxel_size: f32,
    /// Number of metaballs to generate
    pub metaball_count: u32,
    /// Extent of the metaball region (metaballs spawn within [-extent, extent])
    pub metaball_extent: f32,
}

// =============================================================================
// World State Management
// =============================================================================

/// Internal state for a voxel world.
struct WorldState {
    /// Base voxel size
    voxel_size: f32,
    /// Volume sampler (metaballs)
    sampler: MetaballsSampler,
    /// Last generated mesh (keeps pointers valid)
    last_mesh: Option<MeshOutput>,
}

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
// FFI Functions
// =============================================================================

/// Returns the library version as a packed u32: 0xMMmmpp (major.minor.patch).
#[no_mangle]
pub extern "C" fn voxel_version() -> u32 {
    0x000200 // v0.2.0
}

/// Create a new voxel world with internal metaballs sampler.
///
/// # Safety
/// - `config` must point to a valid FfiWorldConfig struct.
///
/// # Returns
/// - Positive world_id on success
/// - -1 if config is null
/// - -2 if failed to acquire lock
#[no_mangle]
pub unsafe extern "C" fn voxel_world_create(config: *const FfiWorldConfig) -> i32 {
    if config.is_null() {
        return -1;
    }

    let cfg = &*config;

    let sampler = MetaballsSampler::random(
        cfg.seed,
        cfg.metaball_count as usize,
        cfg.metaball_extent as f64,
    );

    let state = WorldState {
        voxel_size: cfg.voxel_size,
        sampler,
        last_mesh: None,
    };

    let Ok(mut guard) = WORLDS.lock() else {
        return -2;
    };

    ensure_worlds_initialized(&mut guard);
    let worlds = guard.as_mut().unwrap();

    let world_id = NEXT_WORLD_ID.fetch_add(1, Ordering::SeqCst);
    worlds.insert(world_id, state);

    world_id
}

/// Generate mesh for a specific chunk in the world.
///
/// # Safety
/// - `out` must point to a valid FfiMeshResult struct.
///
/// # Parameters
/// - `world_id`: ID returned by voxel_world_create
/// - `grid_x/y/z`: Chunk grid coordinates (each chunk is 28 voxels)
/// - `lod`: Level of detail (0 = finest)
/// - `out`: Output struct to receive mesh pointers
///
/// # Returns
/// - 0 on success (even if mesh is empty)
/// - -1 if out is null
/// - -2 if failed to acquire lock
/// - -3 if world_id not found
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

    // Each chunk spans 28 voxels (32 - 4 boundary overlap for surface nets)
    const CHUNK_VOXELS: i64 = 28;
    let lod_scale = 1i64 << lod;
    let grid_offset = [
        (grid_x as i64) * CHUNK_VOXELS * lod_scale,
        (grid_y as i64) * CHUNK_VOXELS * lod_scale,
        (grid_z as i64) * CHUNK_VOXELS * lod_scale,
    ];

    let effective_voxel_size = (state.voxel_size as f64) * (lod_scale as f64);

    // Sample volume
    let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
    let mut materials = Box::new([0u8; SAMPLE_SIZE_CB]);

    state
        .sampler
        .sample_volume(grid_offset, effective_voxel_size, &mut *volume, &mut *materials);

    // Generate mesh
    let config = MeshConfig {
        voxel_size: effective_voxel_size as f32,
        neighbor_mask: 0,
        normal_mode: NormalMode::InterpolatedGradient,
        use_microsplat_encoding: false,
    };

    let output = surface_nets::generate(&*volume, &*materials, &config);

    (*out) = FfiMeshResult {
        vertices_ptr: output.vertices.as_ptr(),
        vertices_count: output.vertices.len() as u32,
        indices_ptr: output.indices.as_ptr(),
        indices_count: output.indices.len() as u32,
    };

    state.last_mesh = Some(output);

    0
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
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(voxel_version(), 0x000200);
    }

    #[test]
    fn test_world_lifecycle() {
        let config = FfiWorldConfig {
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
    fn test_world_multiple_chunks() {
        let config = FfiWorldConfig {
            seed: 123,
            voxel_size: 1.0,
            metaball_count: 10,
            metaball_extent: 50.0,
        };

        unsafe {
            let world_id = voxel_world_create(&config);
            assert!(world_id > 0);

            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let mut result = FfiMeshResult {
                            vertices_ptr: std::ptr::null(),
                            vertices_count: 0,
                            indices_ptr: std::ptr::null(),
                            indices_count: 0,
                        };

                        let status = voxel_chunk_generate(world_id, x, y, z, 0, &mut result);
                        assert_eq!(status, 0, "Generate chunk ({},{},{}) should succeed", x, y, z);
                    }
                }
            }

            voxel_world_destroy(world_id);
        }
    }

    #[test]
    fn test_world_invalid_id() {
        let mut result = FfiMeshResult {
            vertices_ptr: std::ptr::null(),
            vertices_count: 0,
            indices_ptr: std::ptr::null(),
            indices_count: 0,
        };

        unsafe {
            let status = voxel_chunk_generate(99999, 0, 0, 0, 0, &mut result);
            assert_eq!(status, -3, "Invalid world_id should return -3");
        }
    }
}
