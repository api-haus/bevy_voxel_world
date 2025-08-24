use bevy::math::Vec3;
use bevy::pbr::light_consts::lux;
use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLight};
use bevy::prelude::{default, Commands, Transform};

/// Sets up atmospheric scattering and skybox for realistic sky rendering
pub fn setup_atmosphere(mut commands: Commands) {
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
