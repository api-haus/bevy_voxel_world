use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::render::mesh::MeshAabb;
use bevy::render::render_resource::AsBindGroup;
use fast_surface_nets::SurfaceNetsBuffer;
use ilattice::prelude::{IVec3, UVec3};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use rand::{Rng, SeedableRng};
use rayon::prelude::*;

mod apply_mesh;
mod editing;
mod materials;
mod scheduler;
mod telemetry;
mod volume_spawn;
mod authoring {
    pub(crate) use crate::authoring::components::{CsgOp, SdfBox, SdfSphere};
    #[cfg(all(debug_assertions, feature = "editor"))]
    pub(crate) use crate::authoring::scene_io::{
        demo_spawn_authoring, save_authoring_scene_system,
    };
    pub(crate) use crate::authoring::seed::seed_random_spheres_sdf;
}
use apply_mesh::apply_remeshes;
pub use editing::{EditOp, VoxelEditEvent};
pub use materials::TriplanarExtension;
pub(crate) use materials::{setup_voxel_material, VoxelRenderMaterial};
pub(crate) use scheduler::{
    drain_queue_and_spawn_jobs, pump_remesh_results, RemeshBudget, RemeshQueue,
};
pub(crate) use telemetry::VoxelTelemetry;
use telemetry::{publish_diagnostics, register_voxel_diagnostics, update_telemetry_begin};
use crate::plugin::telemetry::setup_voxel_screen_diagnostics;

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

// Editing types moved to editing.rs

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
            .init_resource::<scheduler::RemeshInFlightTimings>()
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
            });

        // Register diagnostics and custom Perf UI entries
        register_voxel_diagnostics(app);

        // Register authoring reflection types for scene I/O
        app.register_type::<authoring::CsgOp>()
            .register_type::<authoring::SdfSphere>()
            .register_type::<authoring::SdfBox>();

        app.add_systems(
            Startup,
            (
                volume_spawn::spawn_volume_chunks,
                setup_voxel_material,
                authoring::seed_random_spheres_sdf,
                setup_voxel_screen_diagnostics,
                #[cfg(all(debug_assertions, feature = "editor"))]
                authoring::demo_spawn_authoring,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                editing::apply_edit_events.in_set(VoxelSet::Editing),
                update_telemetry_begin
                    .in_set(VoxelSet::Schedule)
                    .before(drain_queue_and_spawn_jobs),
                drain_queue_and_spawn_jobs.in_set(VoxelSet::Schedule),
                pump_remesh_results.in_set(VoxelSet::Schedule),
                apply_remeshes.in_set(VoxelSet::ApplyMeshes),
                publish_diagnostics.in_set(VoxelSet::ApplyMeshes),
                #[cfg(all(debug_assertions, feature = "editor"))]
                authoring::save_authoring_scene_system.in_set(VoxelSet::Authoring),
            ),
        );
    }
}

// spawn_volume_chunks moved to volume_spawn.rs

pub(crate) fn sample_min(desc: &VoxelVolumeDesc, chunk_coords: IVec3) -> IVec3 {
    let core = desc.chunk_core_dims;
    let offset = IVec3::new(
        (core.x as i32) * chunk_coords.x,
        (core.y as i32) * chunk_coords.y,
        (core.z as i32) * chunk_coords.z,
    );
    desc.origin_cell + offset - IVec3::ONE
}

// seeding moved to authoring::seed

// Editing systems moved to editing.rs

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
