use crate::voxel_plugin::voxels::storage::VoxelStorage;
use bevy::prelude::Vec3;
use fast_surface_nets::ndshape::ConstShape3u32;
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};

/// A simple wrapper that runs Surface Nets on fixed chunk sizes.
/// Returns `None` if the chunk is entirely positive or entirely negative, including apron.
pub fn remesh_chunk_fixed<const SX: u32, const SY: u32, const SZ: u32>(
    storage: &VoxelStorage,
) -> Option<SurfaceNetsBuffer> {
    // Expect matching dims (including apron)
    debug_assert_eq!(storage.dims.sample.x, SX);
    debug_assert_eq!(storage.dims.sample.y, SY);
    debug_assert_eq!(storage.dims.sample.z, SZ);

    // Early skip if all positive or all negative
    let mut any_pos = false;
    let mut any_neg = false;
    for &s in storage.sdf.iter() {
        if s <= 0.0 {
            any_neg = true;
        } else {
            any_pos = true;
        }
        if any_pos && any_neg {
            break;
        }
    }
    if !(any_pos && any_neg) {
        return None;
    }

    // Run surface nets on the padded array
    let mut buffer = SurfaceNetsBuffer::default();
    surface_nets(
        &storage.sdf,
        &ConstShape3u32::<SX, SY, SZ>,
        [0; 3],
        [SX - 1, SY - 1, SZ - 1],
        &mut buffer,
    );

    if buffer.positions.is_empty() {
        None
    } else {
        Some(buffer)
    }
}

/// Dispatch to a supported fixed-size meshing implementation based on the storage sample dims.
/// Currently supports only 16^3 core (18^3 sample) chunks.
pub fn remesh_chunk_dispatch(storage: &VoxelStorage) -> Option<SurfaceNetsBuffer> {
    let s = storage.dims.sample;
    if s.x == 18 && s.y == 18 && s.z == 18 {
        return remesh_chunk_fixed::<18, 18, 18>(storage);
    }
    if s.x == 34 && s.y == 34 && s.z == 34 {
        return remesh_chunk_fixed::<34, 34, 34>(storage);
    }
    None
}

/// Central-difference gradient estimator for a scalar field `f` at point `p`.
/// Uses spacing `h` along each axis.
pub fn central_gradient<F>(f: F, p: Vec3, h: f32) -> Vec3
where
    F: Fn(Vec3) -> f32,
{
    let inv_2h = 0.5 / h;
    let dx = f(p + Vec3::X * h) - f(p - Vec3::X * h);
    let dy = f(p + Vec3::Y * h) - f(p - Vec3::Y * h);
    let dz = f(p + Vec3::Z * h) - f(p - Vec3::Z * h);
    Vec3::new(dx, dy, dz) * inv_2h
}
