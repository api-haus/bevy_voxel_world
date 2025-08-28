use bevy::math::Vec3;
use bevy::pbr::light_consts::lux;
use bevy::pbr::{AmbientLight, CascadeShadowConfigBuilder, DirectionalLight};
use bevy::prelude::*;

pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, setup_atmosphere);
	}
}

/// Sets up atmospheric scattering and skybox for realistic sky rendering
pub fn setup_atmosphere(mut commands: Commands, mut ambient: ResMut<AmbientLight>) {
	// Tune global ambient so shadowed areas aren't pitch black
	ambient.color = Color::srgb(0.75, 0.82, 0.95);
	ambient.brightness = 1200.0;

	// Configure properly scaled cascade shadow map for the voxel world
	let cascade_shadow_config = CascadeShadowConfigBuilder {
		first_cascade_far_bound: 30.0, // Adjusted for voxel world scale
		maximum_distance: 300.0,       // Adjusted for voxel world scale
		..default()
	}
	.build();

	// Sun directional light
	commands.spawn((
		DirectionalLight {
			shadows_enabled: true,
			// Use RAW_SUNLIGHT for proper atmospheric scattering
			illuminance: lux::RAW_SUNLIGHT,
			..default()
		},
		Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
		cascade_shadow_config,
	));
}
