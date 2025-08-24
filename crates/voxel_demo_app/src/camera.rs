use bevy::core_pipeline::{bloom::Bloom, tonemapping::Tonemapping};
use bevy::pbr::{Atmosphere, AtmosphereSettings};
use bevy::prelude::*;
use bevy::render::camera::Exposure;

use crate::fly_cam::FlyCam;

/// Sets up camera rendering components for atmospheric scattering and post-processing
pub fn setup_camera_rendering(
	mut commands: Commands,
	camera_query: Query<Entity, (With<Camera3d>, With<FlyCam>)>,
) {
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
