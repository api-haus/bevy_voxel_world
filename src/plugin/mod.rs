use avian3d::prelude::{Collider, RigidBody};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::render::mesh::MeshAabb;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use fast_surface_nets::SurfaceNetsBuffer;
use ilattice::prelude::{IVec3, UVec3};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::meshing::surface_nets::buffer_to_meshes_per_material;
use crate::voxels::storage::VoxelStorage;
use rand::{Rng, SeedableRng, rngs::StdRng};
use rayon::prelude::*;

mod apply_mesh;
mod scheduler;
mod telemetry;
use apply_mesh::apply_remeshes;
pub(crate) use scheduler::{
    RemeshBudget, RemeshQueue, RemeshResultChannel, drain_queue_and_spawn_jobs, pump_remesh_results,
};
pub(crate) use telemetry::VoxelTelemetry;
use telemetry::update_telemetry_begin;

#[derive(Resource, Debug, Clone, Copy)]
pub struct VoxelVolumeDesc {
    pub chunk_core_dims: UVec3,
    pub grid_dims: UVec3,
    pub origin_cell: IVec3,
}

impl Default for VoxelVolumeDesc {
    fn default() -> Self {
        Self {
            chunk_core_dims: UVec3::new(16, 16, 16),
            grid_dims: UVec3::new(16, 16, 16),
            origin_cell: IVec3::new(0, 0, 0),
        }
    }
}

#[derive(Component)]
pub struct VoxelVolume {
    pub chunk_core_dims: UVec3,
    pub grid_dims: UVec3,
    pub origin_cell: IVec3,
}

#[derive(Component)]
pub struct VoxelChunk {
    pub chunk_coords: IVec3,
}

#[derive(Clone, Copy, Debug)]
pub enum EditOp {
    Destroy,
    Place,
}

#[derive(Event, Clone, Copy, Debug)]
pub struct VoxelEditEvent {
    pub center_world: Vec3,
    pub radius: f32,
    pub op: EditOp,
}

#[derive(Event)]
pub struct RemeshReady {
    pub entity: Entity,
    pub buffer: SurfaceNetsBuffer,
}

// Scheduler types moved to `scheduler.rs`

// Telemetry moved to `telemetry.rs`

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum VoxelSet {
    Authoring,
    Editing,
    Schedule,
    ApplyMeshes,
    Physics,
}

pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelVolumeDesc>()
            .init_resource::<RemeshBudget>()
            .init_resource::<RemeshQueue>()
            .init_resource::<VoxelTelemetry>()
            .add_event::<VoxelEditEvent>()
            .add_event::<RemeshReady>()
            .configure_sets(
                Update,
                (
                    VoxelSet::Authoring,
                    VoxelSet::Editing,
                    VoxelSet::Schedule,
                    VoxelSet::ApplyMeshes,
                    VoxelSet::Physics,
                ),
            )
            .insert_resource({
                let (tx, rx) = channel();
                scheduler::RemeshResultChannel {
                    tx,
                    rx: Arc::new(Mutex::new(rx)),
                }
            })
            .add_systems(
                Startup,
                (
                    spawn_volume_chunks,
                    setup_voxel_material,
                    seed_random_spheres_sdf,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    apply_edit_events.in_set(VoxelSet::Editing),
                    update_telemetry_begin
                        .in_set(VoxelSet::Schedule)
                        .before(drain_queue_and_spawn_jobs),
                    drain_queue_and_spawn_jobs.in_set(VoxelSet::Schedule),
                    pump_remesh_results.in_set(VoxelSet::Schedule),
                    apply_remeshes.in_set(VoxelSet::ApplyMeshes),
                ),
            );
    }
}

fn spawn_volume_chunks(mut commands: Commands, desc: Res<VoxelVolumeDesc>) {
    let volume_entity = commands
        .spawn((
            Name::new("VoxelVolume"),
            VoxelVolume {
                chunk_core_dims: desc.chunk_core_dims,
                grid_dims: desc.grid_dims,
                origin_cell: desc.origin_cell,
            },
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
        ))
        .id();

    let grid = desc.grid_dims;
    for z in 0..grid.z as i32 {
        for y in 0..grid.y as i32 {
            for x in 0..grid.x as i32 {
                let chunk_coords = IVec3::new(x, y, z);
                let storage = VoxelStorage::new(desc.chunk_core_dims);

                let child = commands
                    .spawn((
                        Name::new(format!("VoxelChunk {:?}", chunk_coords)),
                        VoxelChunk { chunk_coords },
                        storage,
                        Transform::default(),
                        GlobalTransform::default(),
                        Visibility::Visible,
                        InheritedVisibility::VISIBLE,
                        ViewVisibility::default(),
                    ))
                    .id();
                commands.entity(volume_entity).add_child(child);
            }
        }
    }
}

#[derive(Resource, Clone)]
struct VoxelRenderMaterial {
    handle: Handle<ExtendedMaterial<StandardMaterial, TriplanarExtension>>,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TriplanarExtension {
    // Use binding slots starting at 100 to avoid collisions with StandardMaterial
    #[texture(100)]
    #[sampler(101)]
    pub albedo_map: Option<Handle<Image>>,
    #[uniform(102)]
    pub tiling_scale: f32,
}

impl Default for TriplanarExtension {
    fn default() -> Self {
        Self {
            albedo_map: None,
            tiling_scale: 0.08,
        }
    }
}

impl MaterialExtension for TriplanarExtension {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/triplanar_pbr.wgsl".into())
    }
    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/triplanar_pbr.wgsl".into())
    }
}

fn setup_voxel_material(
    mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, TriplanarExtension>>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    // Load a default diffuse texture from the attached stylized textures pack
    let albedo: Handle<Image> =
        asset_server.load("free_stylized_textures/ground_01_1k/ground_01_color_1k.png");
    let handle = materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: None,
            perceptual_roughness: 0.8,
            metallic: 0.0,
            ..Default::default()
        },
        extension: TriplanarExtension {
            albedo_map: Some(albedo),
            tiling_scale: 0.08,
        },
    });
    commands.insert_resource(VoxelRenderMaterial { handle });
}

fn sample_min(desc: &VoxelVolumeDesc, chunk_coords: IVec3) -> IVec3 {
    let core = desc.chunk_core_dims;
    let offset = IVec3::new(
        (core.x as i32) * chunk_coords.x,
        (core.y as i32) * chunk_coords.y,
        (core.z as i32) * chunk_coords.z,
    );
    desc.origin_cell + offset - IVec3::ONE
}

fn seed_random_spheres_sdf(
    desc: Res<VoxelVolumeDesc>,
    mut queue: ResMut<RemeshQueue>,
    mut q_chunks: Query<(Entity, &mut VoxelStorage, &VoxelChunk)>,
) {
    let mut rng: StdRng = SeedableRng::from_seed([7; 32]);

    // Pre-generate a set of random spheres in world/sample space covering the whole volume
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

    // Collect chunk tasks to compute off-thread
    #[derive(Clone, Copy)]
    struct ChunkTask {
        entity: Entity,
        sample_min: IVec3,
        sample_dims: UVec3,
    }
    let tasks: Vec<ChunkTask> = q_chunks
        .iter_mut()
        .map(|(e, storage, chunk)| {
            let sample_min = sample_min(&desc, chunk.chunk_coords);
            ChunkTask {
                entity: e,
                sample_min,
                sample_dims: storage.dims.sample,
            }
        })
        .collect();

    // Compute SDF arrays per chunk in parallel, culling spheres by AABB
    let results: Vec<(Entity, Vec<f32>)> = tasks
        .par_iter()
        .map(|task| {
            let sx = task.sample_dims.x;
            let sy = task.sample_dims.y;
            let sz = task.sample_dims.z;
            let len = (sx * sy * sz) as usize;
            let mut sdf = vec![f32::INFINITY; len];

            let chunk_min = task.sample_min;
            let chunk_max = chunk_min + IVec3::new(sx as i32 - 1, sy as i32 - 1, sz as i32 - 1);

            // Filter spheres that intersect this chunk's sample AABB
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
                        let idx = crate::core::index::linear_index(x, y, z, task.sample_dims);
                        let mut dmin = sdf[idx];
                        for s in &intersecting {
                            // Optional per-voxel AABB skip
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

    // Write results back to ECS storages and enqueue
    for (entity, sdf) in results.into_iter() {
        if let Ok((e, mut storage, _chunk)) = q_chunks.get_mut(entity) {
            // Copy values
            storage.sdf.copy_from_slice(&sdf);
            queue.inner.push_back(e);
        }
    }
}

fn sphere_aabb_intersects(center: Vec3, radius: f32, min: IVec3, max: IVec3) -> bool {
    let mut d2 = 0.0f32;
    let c = center;
    let clamp = |v: f32, lo: f32, hi: f32| v.max(lo).min(hi);
    let px = clamp(c.x, min.x as f32, max.x as f32);
    let py = clamp(c.y, min.y as f32, max.y as f32);
    let pz = clamp(c.z, min.z as f32, max.z as f32);
    d2 += (c.x - px) * (c.x - px);
    d2 += (c.y - py) * (c.y - py);
    d2 += (c.z - pz) * (c.z - pz);
    d2 <= radius * radius
}

fn apply_edit_events(
    desc: Res<VoxelVolumeDesc>,
    mut queue: ResMut<RemeshQueue>,
    mut evr: EventReader<VoxelEditEvent>,
    mut q_chunks: Query<(Entity, &mut VoxelStorage, &VoxelChunk)>,
) {
    for ev in evr.read() {
        let center = ev.center_world;
        let radius = ev.radius;
        for (entity, mut storage, chunk) in q_chunks.iter_mut() {
            let s = storage.dims.sample;
            let min = sample_min(&desc, chunk.chunk_coords);
            let max = min + IVec3::new(s.x as i32 - 1, s.y as i32 - 1, s.z as i32 - 1);
            if !sphere_aabb_intersects(center, radius, min, max) {
                continue;
            }
            let mut changed = false;
            for z in 0..s.z {
                for y in 0..s.y {
                    for x in 0..s.x {
                        let p = Vec3::new(
                            (min.x + x as i32) as f32,
                            (min.y + y as i32) as f32,
                            (min.z + z as i32) as f32,
                        );
                        let b = p.distance(center) - radius; // sphere SDF (negative inside)
                        let idx = crate::core::index::linear_index(x, y, z, s);
                        let s_old = storage.sdf[idx];
                        let s_new = match ev.op {
                            EditOp::Destroy => s_old.max(-b),
                            EditOp::Place => s_old.min(b),
                        };
                        if s_new != s_old {
                            match ev.op {
                                EditOp::Destroy => {
                                    if s_old < 0.0 && s_new >= 0.0 {
                                        storage.mat[idx] = crate::voxels::storage::AIR_ID;
                                    }
                                }
                                EditOp::Place => {
                                    if s_old >= 0.0 && s_new < 0.0 {
                                        // TODO: select material; default 1 for now
                                        storage.mat[idx] = 1;
                                    }
                                }
                            }
                            storage.sdf[idx] = s_new;
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                queue.inner.push_back(entity);
            }
        }
    }
}

// scheduler::drain_queue_and_spawn_jobs and scheduler::pump_remesh_results moved to module

// apply_mesh::apply_remeshes moved to module

// Telemetry begin moved to telemetry::update_telemetry_begin

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::render::mesh::Mesh;
    use bevy::render::render_resource::Shader;

    #[test]
    fn spawns_volume_and_chunks() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            // Prevent background jobs in tests to avoid threading teardown races
            .insert_resource(RemeshBudget {
                max_chunks_per_frame: 0,
                time_slice_ms: 0,
            })
            // Provide asset containers required by startup systems
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<Shader>::default())
            .insert_resource(VoxelVolumeDesc {
                chunk_core_dims: UVec3::new(8, 8, 8),
                grid_dims: UVec3::new(2, 1, 2),
                origin_cell: IVec3::ZERO,
            })
            .add_plugins(VoxelPlugin);

        // Run Startup schedule once
        app.update();

        // One volume
        let world = app.world_mut();
        let volumes = world.query::<&VoxelVolume>().iter(world).count();
        assert_eq!(volumes, 1);

        // Number of chunks equals product of grid dims
        let chunks = world.query::<&VoxelChunk>().iter(world).count();
        assert_eq!(chunks, 2 * 1 * 2);
    }

    #[test]
    fn telemetry_increments_on_applied_mesh() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .insert_resource(RemeshBudget {
                max_chunks_per_frame: 0,
                time_slice_ms: 0,
            })
            .insert_resource(Assets::<StandardMaterial>::default())
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<Shader>::default())
            .insert_resource(Assets::<
                ExtendedMaterial<StandardMaterial, TriplanarExtension>,
            >::default())
            .insert_resource(VoxelVolumeDesc::default())
            .add_plugins(VoxelPlugin);

        // Provide a render material handle
        {
            let mut mats = app
                .world_mut()
                .resource_mut::<Assets<ExtendedMaterial<StandardMaterial, TriplanarExtension>>>();
            let handle = mats.add(ExtendedMaterial {
                base: StandardMaterial::default(),
                extension: TriplanarExtension::default(),
            });
            app.world_mut()
                .insert_resource(VoxelRenderMaterial { handle });
        }

        // Spawn a chunk entity with a Transform so apply_remeshes can position it
        let e = {
            let mut world = app.world_mut();
            world
                .spawn((
                    VoxelChunk {
                        chunk_coords: IVec3::ZERO,
                    },
                    Transform::default(),
                    GlobalTransform::default(),
                ))
                .id()
        };

        // Send a synthetic remesh event with a tiny triangle
        {
            let mut evs = app.world_mut().resource_mut::<Events<RemeshReady>>();
            let mut buffer = SurfaceNetsBuffer::default();
            buffer.positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
            buffer.normals = vec![[0.0, 0.0, 1.0]; 3];
            buffer.indices = vec![0, 1, 2];
            evs.send(RemeshReady { entity: e, buffer });
        }

        // Run systems once; apply_remeshes should consume the event and update telemetry
        app.update();

        let telemetry = app.world().resource::<super::VoxelTelemetry>();
        assert_eq!(telemetry.meshed_this_frame, 1);
        assert!(telemetry.total_meshed >= 1);
    }
}
