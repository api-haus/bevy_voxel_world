use avian3d::prelude as avian;
use bevy::prelude::*;
use bevy_tnua::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::player::abilities::PlayerAbility;
use crate::player::actions::{PlayerAction, default_input_map};
use crate::player::components::{Player, PlayerConfig, PlayerDimensions};
use crate::player::input::PlayerInput;
use crate::player::orientation::visor::attach_player_visor;

pub fn setup_player(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
	desc: Res<bevy_voxel_plugin::plugin::VoxelVolumeDesc>,
) {
	let radius = 0.4;
	let height = 1.6;
	let total_dims = Vec3::new(
		(desc.grid_dims.x * desc.chunk_core_dims.x) as f32,
		(desc.grid_dims.y * desc.chunk_core_dims.y) as f32,
		(desc.grid_dims.z * desc.chunk_core_dims.z) as f32,
	);
	let start = Vec3::new(total_dims.x * 0.5, total_dims.y - 1.0, total_dims.z * 0.5);

	let mesh = meshes.add(Mesh::from(Capsule3d::new(radius, height - 2.0 * radius)));
	let character_mat = materials.add(StandardMaterial {
		base_color: Color::srgb(0.2, 0.7, 0.9),
		..Default::default()
	});

	let entity = commands
		.spawn((
			Name::new("Player"),
			Player::default(),
			avian::RigidBody::Dynamic,
			avian::Collider::capsule(height * 0.5, radius),
			avian::Restitution::new(0.0),
			avian::Friction::new(0.9),
			Transform::from_translation(start),
			GlobalTransform::default(),
			Mesh3d(mesh),
			MeshMaterial3d(character_mat.clone()),
		))
		.id();

	commands.entity(entity).insert((
		// Avoid tipping over
		avian::LockedAxes::new().lock_rotation_x().lock_rotation_z(),
		TnuaController::default(),
		// Proximity sensor allows querying grounding and enables one-way platform logic.
		bevy_tnua::TnuaProximitySensor::default(),
		// Obstacle radar for wall/obstacle interactions (e.g., climb)
		bevy_tnua::TnuaObstacleRadar::new(radius + 0.1, height + 0.2),
		// Leafwing: prefer inserting components directly over the deprecated bundle
		ActionState::<PlayerAction>::default(),
		default_input_map(),
		PlayerAbility::abilities_bundle(),
		PlayerDimensions { height, radius },
		PlayerInput::default(),
		PlayerConfig::default(),
		avian::GravityScale(1.0),
	));

	// Visor child to indicate player forward orientation
	attach_player_visor(
		&mut commands,
		entity,
		&mut meshes,
		&mut materials,
		height,
		radius,
		character_mat.clone(),
	);
}
