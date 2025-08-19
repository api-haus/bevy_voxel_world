use crate::voxels::storage::VoxelStorage;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::Vec3;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;
use fast_surface_nets::ndshape::ConstShape3u32;
use fast_surface_nets::{SurfaceNetsBuffer, surface_nets};

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

/// Convert a Surface Nets buffer into one or more meshes split by material.
/// Single-material skeleton for now; `_vertex_materials` is reserved for future splitting.
pub fn buffer_to_meshes_per_material(
    buffer: &SurfaceNetsBuffer,
    _vertex_materials: Option<&[u8]>,
) -> Vec<bevy::render::mesh::Mesh> {
    if buffer.positions.is_empty() || buffer.indices.is_empty() {
        return Vec::new();
    }

    let mut mesh = bevy::render::mesh::Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        bevy::render::mesh::Mesh::ATTRIBUTE_POSITION,
        buffer.positions.clone(),
    );
    if !buffer.normals.is_empty() {
        mesh.insert_attribute(
            bevy::render::mesh::Mesh::ATTRIBUTE_NORMAL,
            buffer.normals.clone(),
        );
    }
    mesh.insert_indices(Indices::U32(buffer.indices.clone()));

    vec![mesh]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ilattice::prelude::UVec3;

    #[test]
    fn empty_and_solid_skip() {
        let mut storage = VoxelStorage::new(UVec3::new(16, 16, 16));
        storage.fill_default(1.0, 0); // all positive
        assert!(remesh_chunk_fixed::<18, 18, 18>(&storage).is_none());

        storage.fill_default(-1.0, 1); // all negative
        assert!(remesh_chunk_fixed::<18, 18, 18>(&storage).is_none());
    }

    #[test]
    fn simple_interface_meshes() {
        let mut storage = VoxelStorage::new(UVec3::new(16, 16, 16));
        storage.fill_default(1.0, 0);
        // Create a plane at z = 9 within the padded 18-sized grid: set <=0 below plane
        for z in 0..9u32 {
            for y in 0..18u32 {
                for x in 0..18u32 {
                    *storage.sdf_mut_at(x, y, z) = -1.0;
                }
            }
        }

        let buffer = remesh_chunk_fixed::<18, 18, 18>(&storage).expect("mesh expected");
        assert!(!buffer.positions.is_empty());
        assert!(!buffer.indices.is_empty());
    }

    #[test]
    fn gradient_sanity_for_plane() {
        // f(x,y,z) = x => grad = (1,0,0)
        let f = |p: Vec3| p.x;
        let g = central_gradient(f, Vec3::new(0.0, 0.0, 0.0), 1.0);
        let err = (g - Vec3::X).length();
        assert!(err < 1e-5, "gradient {:?} too far from expected", g);
    }

    #[test]
    fn dispatch_supported_vs_unsupported() {
        // 16^3 core -> 18^3 sample (supported)
        let mut s16 = VoxelStorage::new(UVec3::new(16, 16, 16));
        s16.fill_default(1.0, 0);
        let sz = s16.dims.sample;
        for z in 0..(sz.z / 2) {
            for y in 0..sz.y {
                for x in 0..sz.x {
                    *s16.sdf_mut_at(x, y, z) = -1.0;
                }
            }
        }
        assert!(remesh_chunk_dispatch(&s16).is_some());

        // 32^3 core -> 34^3 sample (now supported)
        let mut s32 = VoxelStorage::new(UVec3::new(32, 32, 32));
        s32.fill_default(1.0, 0);
        let sz = s32.dims.sample;
        for z in 0..(sz.z / 2) {
            for y in 0..sz.y {
                for x in 0..sz.x {
                    *s32.sdf_mut_at(x, y, z) = -1.0;
                }
            }
        }
        assert!(remesh_chunk_dispatch(&s32).is_some());

        // 8^3 core -> 10^3 sample (unsupported)
        let mut s8 = VoxelStorage::new(UVec3::new(8, 8, 8));
        s8.fill_default(1.0, 0);
        let sz = s8.dims.sample;
        for z in 0..(sz.z / 2) {
            for y in 0..sz.y {
                for x in 0..sz.x {
                    *s8.sdf_mut_at(x, y, z) = -1.0;
                }
            }
        }
        assert!(remesh_chunk_dispatch(&s8).is_none());
    }

    #[test]
    fn buffer_to_meshes_single_bucket_sanity() {
        let mut storage = VoxelStorage::new(UVec3::new(16, 16, 16));
        storage.fill_default(1.0, 0);
        let sz = storage.dims.sample;
        for z in 0..(sz.z / 2) {
            for y in 0..sz.y {
                for x in 0..sz.x {
                    *storage.sdf_mut_at(x, y, z) = -1.0;
                }
            }
        }
        let buffer = remesh_chunk_dispatch(&storage).expect("mesh expected");
        let meshes = buffer_to_meshes_per_material(&buffer, None);
        assert_eq!(meshes.len(), 1);
    }
}
