#![cfg(test)]
use bevy::asset::AssetPlugin;
use bevy::pbr::{ExtendedMaterial, Mesh3d, StandardMaterial};
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use ilattice::prelude::{IVec3 as ILVec3, UVec3};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{RemeshQueue, VoxelChunk, VoxelRenderMaterial, VoxelVolumeDesc};
use crate::voxel_plugin::voxels::storage::{AIR_ID, VoxelStorage};

fn build_min_app() -> App {
	let mut app = App::new();
	app
		.add_plugins(MinimalPlugins)
		.add_plugins(AssetPlugin::default())
		.init_resource::<VoxTestState>()
		.init_resource::<VoxelVolumeDesc>()
		.init_resource::<RemeshQueue>()
		.init_resource::<super::scheduler::RemeshBudget>()
		.init_resource::<Assets<Mesh>>()
		.init_resource::<Assets<ExtendedMaterial<StandardMaterial, super::TriplanarExtension>>>();
	app.add_event::<super::RemeshReady>();
	app
}

#[derive(Resource, Default)]
struct VoxTestState(Option<Entity>);

#[test]
fn meshing_applies_mesh_on_enqueued_chunk() {
	let mut app = build_min_app();

	// Small volume: 1 chunk of 16^3 core with apron (18^3 samples)
	app.world.resource_mut::<VoxelVolumeDesc>().chunk_core_dims = UVec3::new(16, 16, 16);

	// Spawn one chunk entity with storage and default transform
	let entity = app
		.world
		.spawn((
			Name::new("TestChunk"),
			VoxelChunk {
				chunk_coords: ILVec3::new(0, 0, 0),
			},
			VoxelStorage::new(UVec3::new(16, 16, 16)),
			Transform::default(),
			GlobalTransform::default(),
			Visibility::Visible,
			InheritedVisibility::VISIBLE,
			ViewVisibility::default(),
		))
		.id();
	app.world.resource_mut::<VoxTestState>().0 = Some(entity);

	// Fill storage with a simple solid: sphere at center radius ~6
	{
		let mut storage = app.world.get_mut::<VoxelStorage>(entity).unwrap();
		let s = storage.dims.sample;
		let cx = (s.x as f32) * 0.5;
		let cy = (s.y as f32) * 0.5;
		let cz = (s.z as f32) * 0.5;
		for z in 0..s.z {
			for y in 0..s.y {
				for x in 0..s.x {
					let dx = x as f32 - cx;
					let dy = y as f32 - cy;
					let dz = z as f32 - cz;
					let dist = (dx * dx + dy * dy + dz * dz).sqrt();
					let sdf = dist - 6.0;
					*storage.sdf_mut_at(x, y, z) = sdf;
					*storage.mat_mut_at(x, y, z) = if sdf <= 0.0 { 1 } else { AIR_ID };
				}
			}
		}
	}

	// Enqueue for meshing
	app
		.world
		.resource_mut::<RemeshQueue>()
		.inner
		.push_back(entity);

	// Inject a dummy render material to allow apply system to run
	let handle = app
		.world
		.resource_mut::<Assets<ExtendedMaterial<StandardMaterial, super::TriplanarExtension>>>()
		.add(ExtendedMaterial {
			base: StandardMaterial::default(),
			extension: super::TriplanarExtension::default(),
		});
	app.world.insert_resource(VoxelRenderMaterial { handle });

	// Create remesh result channel like the plugin does
	let (tx, rx) = std::sync::mpsc::channel();
	app
		.world
		.insert_resource(super::scheduler::RemeshResultChannel {
			tx,
			rx: Arc::new(Mutex::new(rx)),
		});

	// Run only the scheduler and apply systems directly
	// drain_queue_and_spawn_jobs spawns rayon tasks; advance a few iterations to allow completion
	let mut has_mesh = false;
	for _ in 0..50 {
		app
			.world
			.run_system_once(super::scheduler::drain_queue_and_spawn_jobs);
		app
			.world
			.run_system_once(super::scheduler::pump_remesh_results);
		app.world.run_system_once(super::apply_mesh::apply_remeshes);
		if app.world.get::<Mesh3d>(entity).is_some() {
			has_mesh = true;
			break;
		}
		std::thread::sleep(Duration::from_millis(5));
	}

	// Assert a Mesh3d is present
	assert!(has_mesh, "Expected Mesh3d on chunk after meshing");
}
