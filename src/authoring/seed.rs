use bevy::prelude::*;
use ilattice::prelude::{IVec3, UVec3};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rayon::prelude::*;

use crate::core::index::linear_index;
use crate::voxels::storage::VoxelStorage;

/// Random sphere field seeding used in the demo. Behavior identical to the previous inline version.
pub(crate) fn seed_random_spheres_sdf(
    desc: Res<crate::plugin::VoxelVolumeDesc>,
    mut queue: ResMut<crate::plugin::RemeshQueue>,
    mut q_chunks: Query<(Entity, &mut VoxelStorage, &crate::plugin::VoxelChunk)>,
) {
    let mut rng: StdRng = SeedableRng::from_seed([7; 32]);

    let vol_shape = IVec3::new(
        (desc.grid_dims.x * desc.chunk_core_dims.x) as i32,
        (desc.grid_dims.y * desc.chunk_core_dims.y) as i32,
        (desc.grid_dims.z * desc.chunk_core_dims.z) as i32,
    );
    let sphere_count = 400usize;
    #[derive(Clone, Copy)]
    struct Sphere {
        center: IVec3,
        radius: f32,
        aabb_min: IVec3,
        aabb_max: IVec3,
    }
    let spheres: Vec<Sphere> = (0..sphere_count)
        .map(|_| {
            let cx = rng.gen_range(0..vol_shape.x);
            let cy = rng.gen_range(0..vol_shape.y);
            let cz = rng.gen_range(0..vol_shape.z);
            let r = rng.gen_range(2.0f32..16.0f32);
            let center = IVec3::new(cx, cy, cz);
            let rr = r.ceil() as i32 + 1;
            let aabb_min = center - IVec3::splat(rr);
            let aabb_max = center + IVec3::splat(rr);
            Sphere {
                center,
                radius: r,
                aabb_min,
                aabb_max,
            }
        })
        .collect();

    #[derive(Clone, Copy)]
    struct ChunkTask {
        entity: Entity,
        sample_min: IVec3,
        sample_dims: UVec3,
    }
    let tasks: Vec<ChunkTask> = q_chunks
        .iter_mut()
        .map(|(e, storage, chunk)| {
            let sample_min = super::super::plugin::sample_min(&desc, chunk.chunk_coords);
            ChunkTask {
                entity: e,
                sample_min,
                sample_dims: storage.dims.sample,
            }
        })
        .collect();

    let results: Vec<(Entity, Vec<f32>)> = tasks
        .par_iter()
        .map(|task| {
            let sx = task.sample_dims.x;
            let sy = task.sample_dims.y;
            let sz = task.sample_dims.z;
            let len = (sx * sy * sz) as usize;
            let mut sdf = vec![f32::INFINITY; len];
            let chunk_min = task.sample_min;
            let chunk_max = IVec3::new(
                chunk_min.x + sx as i32 - 1,
                chunk_min.y + sy as i32 - 1,
                chunk_min.z + sz as i32 - 1,
            );

            let intersecting: Vec<_> = spheres
                .iter()
                .filter(|s| {
                    !(s.aabb_max.x < chunk_min.x
                        || s.aabb_min.x > chunk_max.x
                        || s.aabb_max.y < chunk_min.y
                        || s.aabb_min.y > chunk_max.y
                        || s.aabb_max.z < chunk_min.z
                        || s.aabb_min.z > chunk_max.z)
                })
                .copied()
                .collect();

            if intersecting.is_empty() {
                return (task.entity, sdf);
            }

            for z in 0..sz {
                for y in 0..sy {
                    for x in 0..sx {
                        let p = IVec3::new(x as i32, y as i32, z as i32) + chunk_min;
                        let idx = linear_index(x, y, z, task.sample_dims);
                        let mut dmin = sdf[idx];
                        for s in &intersecting {
                            if p.x < s.aabb_min.x
                                || p.x > s.aabb_max.x
                                || p.y < s.aabb_min.y
                                || p.y > s.aabb_max.y
                                || p.z < s.aabb_min.z
                                || p.z > s.aabb_max.z
                            {
                                continue;
                            }
                            let dx = (p.x - s.center.x) as f32;
                            let dy = (p.y - s.center.y) as f32;
                            let dz = (p.z - s.center.z) as f32;
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                            dmin = dmin.min(dist - s.radius);
                        }
                        sdf[idx] = dmin;
                    }
                }
            }

            (task.entity, sdf)
        })
        .collect();

    for (entity, sdf) in results.into_iter() {
        if let Ok((e, mut storage, _chunk)) = q_chunks.get_mut(entity) {
            storage.sdf.copy_from_slice(&sdf);
            queue.inner.push_back(e);
        }
    }
}
