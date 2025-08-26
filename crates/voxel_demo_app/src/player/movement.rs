use bevy::prelude::*;
use bevy_tnua::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::player::actions::PlayerAction;
use crate::player::components::Player;

pub fn control_player(
	_time: Res<Time>,
	mut q_player: Query<
		(
			&mut TnuaController,
			&GlobalTransform,
			&ActionState<PlayerAction>,
			&mut Player,
		),
		With<Player>,
	>,
	q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<Player>)>,
	keyboard: Res<ButtonInput<KeyCode>>,
) {
	let cam_yaw_forward = || -> Vec3 {
		if let Some(xf) = q_cam.iter().next() {
			let f = xf.compute_transform().forward();
			Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
		} else {
			Vec3::Z
		}
	};

	for (mut ctrl, _player_xf, actions, mut player) in q_player.iter_mut() {
		let x = (actions.pressed(&PlayerAction::MoveRight) as i32
			- actions.pressed(&PlayerAction::MoveLeft) as i32) as f32;
		let y = (actions.pressed(&PlayerAction::MoveForward) as i32
			- actions.pressed(&PlayerAction::MoveBack) as i32) as f32;
		let move2d = Vec2::new(x, y).clamp_length_max(1.0);
		let boosting = actions.pressed(&PlayerAction::Boost);
		let jump_pressed = actions.pressed(&PlayerAction::Jump) || keyboard.pressed(KeyCode::Space);

		let speed = if boosting { 10.0 } else { 6.0 };
		let yaw_fwd = cam_yaw_forward();
		let yaw_right = yaw_fwd.cross(Vec3::Y).normalize_or_zero();
		let wish_dir = (yaw_right * move2d.x + yaw_fwd * move2d.y).normalize_or_zero();

		ctrl.basis(TnuaBuiltinWalk {
			desired_velocity: wish_dir * speed,
			// Center-of-mass height above ground at rest (capsule half-height + radius)
			float_height: 1.2,
			// Keep contact slightly above float height
			cling_distance: 0.4,
			// Stronger spring to stick to terrain; dampening to avoid bounce
			spring_strength: 800.0,
			spring_dampening: 1.4,
			// Snappier ground accel, limited air control
			acceleration: 80.0,
			air_acceleration: 6.0,
			// Extra gravity while falling improves slope adhesion
			free_fall_extra_gravity: 80.0,
			// We lock X/Z rotations; tilt correction not needed
			tilt_offset_angvel: 0.0,
			tilt_offset_angacl: 0.0,
			turning_angvel: 10.0,
			max_slope: std::f32::consts::PI,
			..Default::default()
		});

		if jump_pressed {
			ctrl.action(TnuaBuiltinJump {
				height: 3.5,
				allow_in_air: false,
				..Default::default()
			});
		}

		// Update player facing vector by slerping toward wish_dir (horizontal only)
		if wish_dir != Vec3::ZERO {
			let current = player.facing.normalize_or_zero();
			let target = wish_dir.normalize_or_zero();
			let smooth = 0.2; // slerp factor per tick; tune as desired
			let dot = current.dot(target).clamp(-1.0, 1.0);
			let angle = dot.acos();
			if angle > 1e-4 {
				// Rodrigues' rotation formula for slerp-like interpolation
				let axis = current.cross(target).normalize_or_zero();
				let step = angle * smooth;
				let rot = Quat::from_axis_angle(axis, step);
				let new_dir = (rot * current).normalize_or_zero();
				if new_dir != Vec3::ZERO {
					player.facing = new_dir;
				}
			} else {
				player.facing = target;
			}
		}
	}
}

/// Visualize player's local wish direction and resulting world velocity using Bevy gizmos.
pub fn visualize_player_movement(
	mut gizmos: Gizmos,
	q_player: Query<(&GlobalTransform, &ActionState<PlayerAction>), With<Player>>,
	q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<Player>)>,
) {
	let Ok((player_xf, actions)) = q_player.get_single() else {
		return;
	};
	let p = player_xf.translation();

	let cam_yaw_forward = || -> Vec3 {
		if let Some(xf) = q_cam.iter().next() {
			let f = xf.compute_transform().forward();
			Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
		} else {
			Vec3::Z
		}
	};

	let x = (actions.pressed(&PlayerAction::MoveRight) as i32
		- actions.pressed(&PlayerAction::MoveLeft) as i32) as f32;
	let y = (actions.pressed(&PlayerAction::MoveForward) as i32
		- actions.pressed(&PlayerAction::MoveBack) as i32) as f32;
	let move2d = Vec2::new(x, y).clamp_length_max(1.0);

	let yaw_fwd = cam_yaw_forward();
	let yaw_right = yaw_fwd.cross(Vec3::Y).normalize_or_zero();
	let wish_dir = (yaw_right * move2d.x + yaw_fwd * move2d.y).normalize_or_zero();

	if wish_dir != Vec3::ZERO {
		// Blue arrow: wish direction (unit)
		gizmos.arrow(p, p + wish_dir, Color::srgb(0.2, 0.6, 1.0));
	}
}

/// Visualize jump input state as an upward arrow over the player.
pub fn visualize_jump_input(
	mut gizmos: Gizmos,
	q_player: Query<(&GlobalTransform, &ActionState<PlayerAction>), With<Player>>,
) {
	let Ok((player_xf, actions)) = q_player.get_single() else {
		return;
	};
	let p = player_xf.translation();
	let pressed = actions.pressed(&PlayerAction::Jump);
	let just = actions.just_pressed(&PlayerAction::Jump);
	let color = if just {
		Color::srgb(1.0, 0.2, 0.2)
	} else if pressed {
		Color::srgb(1.0, 1.0, 0.2)
	} else {
		Color::srgb(0.5, 0.5, 0.5)
	};
	let len = if pressed { 1.6 } else { 0.8 };
	gizmos.arrow(p, p + Vec3::Y * len, color);
}

/// Visualize grounded state with a small sphere over the player: green when grounded, red when airborne.
pub fn visualize_grounded_state(
	mut gizmos: Gizmos,
	q_player: Query<(&GlobalTransform, &bevy_tnua::TnuaProximitySensor), With<Player>>,
) {
	let Ok((player_xf, sensor)) = q_player.get_single() else {
		return;
	};
	let p = player_xf.translation();
	let grounded = sensor.output.is_some();
	let color = if grounded {
		Color::srgb(0.2, 1.0, 0.2)
	} else {
		Color::srgb(1.0, 0.2, 0.2)
	};
	gizmos.sphere(p + Vec3::Y * 1.2, 0.15, color);
}

/// Visualize the player's forward orientation (chest-height) using a gizmo arrow.
/// This function has been moved to orientation::gizmo.
pub fn visualize_player_orientation(
	mut gizmos: Gizmos,
	q_player: Query<&GlobalTransform, With<Player>>,
) {
	let Ok(player_xf) = q_player.get_single() else {
		return;
	};
	let p = player_xf.translation();
	let player_forward = -player_xf.compute_transform().forward().normalize_or_zero();
	let origin = p + Vec3::Y * 0.8;
	let tip = origin + player_forward * 1.2;
	gizmos.arrow(origin, tip, Color::srgb(1.0, 0.5, 0.0));
}
