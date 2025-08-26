use bevy::core_pipeline::{bloom::Bloom, tonemapping::Tonemapping};
use bevy::pbr::{Atmosphere, AtmosphereSettings};
use bevy::prelude::*;
use bevy::render::camera::Exposure;

#[allow(dead_code)]
pub struct CameraPlugin;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[allow(dead_code)]
pub enum CameraSet {
	Startup,
}

impl Plugin for CameraPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(
			Startup,
			(ensure_render_camera, setup_camera_rendering).chain(),
		);
	}
}

/// Ensure there is a real render camera entity present.
#[allow(dead_code)]
pub fn ensure_render_camera(mut commands: Commands, q: Query<Entity, With<Camera3d>>) {
	if q.is_empty() {
		commands.spawn((
			Camera3d::default(),
			Transform::default(),
			GlobalTransform::default(),
		));
	}
}

/// Sets up camera rendering components for atmospheric scattering and post-processing
#[allow(dead_code)]
pub fn setup_camera_rendering(mut commands: Commands, camera_query: Query<Entity, With<Camera3d>>) {
	// Find the fly camera entity and add rendering components to it
	for camera_entity in camera_query.iter() {
		commands.entity(camera_entity).insert((
			// HDR is required for atmospheric scattering
			Camera {
				hdr: true,
				..default()
			},
			// Enable atmospheric scattering for the camera
			Atmosphere::EARTH,
			// Adjust atmosphere settings for voxel world scale (1 unit = 1 meter)
			AtmosphereSettings {
				aerial_view_lut_max_distance: 3.2e5, // 320km view distance
				scene_units_to_m: 1.0,               // 1 unit = 1 meter
				..Default::default()
			},
			// Proper exposure for sunlight
			Exposure::SUNLIGHT,
			// Tone mapping for better visual quality
			Tonemapping::TonyMcMapface,
			// Natural bloom for sun and bright areas
			#[cfg(not(target_os = "ios"))]
			Bloom::NATURAL,
		));
	}
}
