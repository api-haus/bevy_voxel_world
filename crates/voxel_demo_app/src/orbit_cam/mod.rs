use bevy::input::gamepad::GamepadAxis;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy_voxel_plugin::plugin::VoxelVolumeDesc;

pub struct OrbitCamPlugin;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum OrbitCamSet {
	Input,
	Follow,
	Apply,
}

impl Plugin for OrbitCamPlugin {
	fn build(&self, app: &mut App) {
		app
			.configure_sets(
				Update,
				(OrbitCamSet::Input, OrbitCamSet::Follow, OrbitCamSet::Apply).chain(),
			)
			.add_systems(Startup, setup)
			.add_systems(
				Update,
				(
					mouse_orbit.in_set(OrbitCamSet::Input),
					follow_player.in_set(OrbitCamSet::Follow),
					apply_to_camera.in_set(OrbitCamSet::Apply),
				),
			);
	}
}

#[derive(Component, Default)]
pub struct OrbitCam {
	pub yaw: f32,
	pub pitch: f32,
	pub radius: f32,
	pub target: Vec3,
}

// No actions: this camera reads raw mouse + right stick input

pub fn setup(mut commands: Commands, desc: Res<VoxelVolumeDesc>) {
	// Center and default radius
	let total_dims = Vec3::new(
		(desc.grid_dims.x * desc.chunk_core_dims.x) as f32,
		(desc.grid_dims.y * desc.chunk_core_dims.y) as f32,
		(desc.grid_dims.z * desc.chunk_core_dims.z) as f32,
	);
	let center = Vec3::new(total_dims.x * 0.5, total_dims.y * 0.5, total_dims.z * 0.5);
	let radius = 12.0;

	commands.spawn((
		Transform::from_translation(center + Vec3::new(0.0, 0.0, radius)).looking_at(center, Vec3::Y),
		GlobalTransform::default(),
		OrbitCam {
			yaw: 0.0,
			pitch: 0.2,
			radius,
			target: center,
		},
	));
}

pub fn mouse_orbit(
	mut q_cam: Query<(&mut Transform, &mut OrbitCam)>,
	mut mouse_motion: EventReader<MouseMotion>,
	axes: Option<Res<Axis<GamepadAxis>>>,
) {
	let mut look = Vec2::ZERO;
	for ev in mouse_motion.read() {
		look += ev.delta;
	}
	// Sample aggregated right stick axes if present (Bevy 0.16 unit variants)
	let (gx, gy) = if let Some(axes) = axes {
		(
			axes.get(GamepadAxis::RightStickX).unwrap_or(0.0),
			axes.get(GamepadAxis::RightStickY).unwrap_or(0.0),
		)
	} else {
		(0.0, 0.0)
	};
	look += Vec2::new(gx, -gy) * 8.0;
	if look == Vec2::ZERO {
		return;
	}

	for (mut transform, mut orbit) in q_cam.iter_mut() {
		orbit.yaw -= look.x * 0.003;
		orbit.pitch -= look.y * 0.003;
		orbit.pitch = orbit.pitch.clamp(-1.4, 1.4);

		let dir = Quat::from_rotation_y(orbit.yaw) * Quat::from_rotation_x(orbit.pitch) * Vec3::Z;
		let cam_pos = orbit.target + dir * orbit.radius;
		*transform = Transform::from_translation(cam_pos).looking_at(orbit.target, Vec3::Y);
	}
}

// Zoom/dolly disabled

use crate::player::Player;

/// Keep orbit camera target synced to the player position.
pub fn follow_player(
	mut q_cam: Query<(&mut Transform, &mut OrbitCam)>,
	q_player: Query<&GlobalTransform, With<Player>>,
) {
	let Ok(player_xf) = q_player.get_single() else {
		return;
	};
	let p = player_xf.translation();
	for (mut transform, mut orbit) in q_cam.iter_mut() {
		orbit.target = p;
		let dir = Quat::from_rotation_y(orbit.yaw) * Quat::from_rotation_x(orbit.pitch) * Vec3::Z;
		let cam_pos = orbit.target + dir * orbit.radius;
		*transform = Transform::from_translation(cam_pos).looking_at(orbit.target, Vec3::Y);
	}
}

/// Apply virtual orbit camera transform to the real render camera each frame (before rendering).
pub fn apply_to_camera(
	q_orbit: Query<&Transform, With<OrbitCam>>,
	mut q_real: Query<&mut Transform, (With<Camera3d>, Without<OrbitCam>)>,
) {
	let Ok(orbit_tf) = q_orbit.get_single() else {
		return;
	};
	for mut cam_tf in q_real.iter_mut() {
		*cam_tf = *orbit_tf;
	}
}
