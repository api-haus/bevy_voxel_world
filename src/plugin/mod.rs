use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use fast_surface_nets::SurfaceNetsBuffer;
use ilattice::prelude::{IVec3, UVec3};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::meshing::surface_nets::buffer_to_meshes_per_material;
use crate::voxels::storage::VoxelStorage;

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
            grid_dims: UVec3::new(2, 1, 2),
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

#[derive(Event)]
pub struct VoxelEditEvent;

#[derive(Event)]
pub struct RemeshReady {
    pub entity: Entity,
    pub buffer: SurfaceNetsBuffer,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct RemeshBudget {
    pub max_chunks_per_frame: usize,
    pub time_slice_ms: u64,
}

impl Default for RemeshBudget {
    fn default() -> Self {
        Self {
            max_chunks_per_frame: 4,
            time_slice_ms: 2,
        }
    }
}

#[derive(Resource, Default)]
struct RemeshQueue {
    inner: VecDeque<Entity>,
}

#[derive(Resource)]
struct RemeshResultChannel {
    tx: Sender<RemeshReady>,
    rx: Arc<Mutex<Receiver<RemeshReady>>>,
}

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
                RemeshResultChannel {
                    tx,
                    rx: Arc::new(Mutex::new(rx)),
                }
            })
            .add_systems(
                Startup,
                (
                    spawn_volume_chunks,
                    setup_voxel_material,
                    seed_debug_sphere_sdf,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
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
    handle: Handle<StandardMaterial>,
}

fn setup_voxel_material(mut materials: ResMut<Assets<StandardMaterial>>, mut commands: Commands) {
    let handle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.82, 0.74),
        perceptual_roughness: 0.8,
        metallic: 0.0,
        ..Default::default()
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

fn seed_debug_sphere_sdf(
    desc: Res<VoxelVolumeDesc>,
    mut queue: ResMut<RemeshQueue>,
    mut q_chunks: Query<(Entity, &mut VoxelStorage, &VoxelChunk)>,
) {
    // Compute a sphere centered in the volume, radius proportional to the smallest side
    let vol_shape = IVec3::new(
        (desc.grid_dims.x * desc.chunk_core_dims.x) as i32,
        (desc.grid_dims.y * desc.chunk_core_dims.y) as i32,
        (desc.grid_dims.z * desc.chunk_core_dims.z) as i32,
    );
    let center = desc.origin_cell + vol_shape / 2;
    let min_side = vol_shape.x.min(vol_shape.y).min(vol_shape.z) as f32;
    let radius = 0.35 * min_side;

    for (entity, mut storage, chunk) in q_chunks.iter_mut() {
        let s = storage.dims.sample;
        let min = sample_min(&desc, chunk.chunk_coords);
        for z in 0..s.z {
            for y in 0..s.y {
                for x in 0..s.x {
                    let p = IVec3::new(x as i32, y as i32, z as i32) + min;
                    let dx = (p.x - center.x) as f32;
                    let dy = (p.y - center.y) as f32;
                    let dz = (p.z - center.z) as f32;
                    let d = (dx * dx + dy * dy + dz * dz).sqrt() - radius;
                    *storage.sdf_mut_at(x, y, z) = d;
                }
            }
        }
        queue.inner.push_back(entity);
    }
}

fn drain_queue_and_spawn_jobs(
    budget: Res<RemeshBudget>,
    mut queue: ResMut<RemeshQueue>,
    channels: Res<RemeshResultChannel>,
    q_storage: Query<&VoxelStorage>,
) {
    let start = Instant::now();
    let time_slice = Duration::from_millis(budget.time_slice_ms);

    let mut processed = 0usize;
    while processed < budget.max_chunks_per_frame && start.elapsed() <= time_slice {
        let Some(entity) = queue.inner.pop_front() else {
            break;
        };
        processed += 1;

        let Ok(storage) = q_storage.get(entity) else {
            continue;
        };
        let s = storage.dims.sample;
        if !(s.x == 18 && s.y == 18 && s.z == 18) {
            continue;
        }

        // Copy SDF to move into the rayon task
        let sdf: Vec<f32> = storage.sdf.iter().copied().collect();
        let tx = channels.tx.clone();

        rayon::spawn(move || {
            // Early skip
            let mut any_pos = false;
            let mut any_neg = false;
            for &v in &sdf {
                if v <= 0.0 {
                    any_neg = true;
                } else {
                    any_pos = true;
                }
                if any_pos && any_neg {
                    break;
                }
            }
            if !(any_pos && any_neg) {
                return;
            }

            let mut buffer = SurfaceNetsBuffer::default();
            fast_surface_nets::surface_nets(
                &sdf,
                &fast_surface_nets::ndshape::ConstShape3u32::<18, 18, 18>,
                [0; 3],
                [17, 17, 17],
                &mut buffer,
            );

            if buffer.positions.is_empty() {
                return;
            }

            let _ = tx.send(RemeshReady { entity, buffer });
        });
    }
}

fn pump_remesh_results(channels: Res<RemeshResultChannel>, mut evw: EventWriter<RemeshReady>) {
    loop {
        let Ok(guard) = channels.rx.lock() else { break };
        match guard.try_recv() {
            Ok(result) => {
                drop(guard);
                evw.write(result);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }
}

fn apply_remeshes(
    desc: Res<VoxelVolumeDesc>,
    render_mat: Res<VoxelRenderMaterial>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
    mut evr: EventReader<RemeshReady>,
    mut q_chunk_tf: Query<(&VoxelChunk, &mut Transform)>,
) {
    for ev in evr.read() {
        // Build mesh(es) (single material for now)
        let meshes_vec = buffer_to_meshes_per_material(&ev.buffer, None);
        if meshes_vec.is_empty() {
            continue;
        }
        let mesh_handle = meshes.add(meshes_vec.into_iter().next().unwrap());

        if let Ok((chunk, mut transform)) = q_chunk_tf.get_mut(ev.entity) {
            // Position the chunk by its sample min in volume-local space
            let min = sample_min(&desc, chunk.chunk_coords);
            transform.translation = Vec3::new(min.x as f32, min.y as f32, min.z as f32);

            commands.entity(ev.entity).insert((
                Mesh3d(mesh_handle),
                MeshMaterial3d(render_mat.handle.clone()),
            ));
        }
    }
}

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
}
