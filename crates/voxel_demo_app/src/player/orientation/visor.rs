use bevy::prelude::*;

use crate::player::components::PlayerOrientation;

pub fn attach_player_visor(
	commands: &mut Commands,
	parent: Entity,
	meshes: &mut ResMut<Assets<Mesh>>,
	materials: &mut ResMut<Assets<StandardMaterial>>,
	height: f32,
	radius: f32,
	material: Handle<StandardMaterial>,
) {
	let visor_mesh = meshes.add(Mesh::from(Cylinder {
		radius: radius * 0.75,
		half_height: 0.3,
	}));

	commands.entity(parent).with_children(|c| {
		c.spawn((
			Name::new("PlayerVisor"),
			PlayerOrientation,
			Transform::from_translation(Vec3::new(0.0, height * 0.32, radius * 0.5))
				.with_scale(Vec3::new(1.0, 1.0, 0.5))
				.with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
			GlobalTransform::default(),
			Mesh3d(visor_mesh),
			MeshMaterial3d(material),
		));
	});
}
