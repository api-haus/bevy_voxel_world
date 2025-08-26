use bevy::prelude::*;

use bevy_tnua::prelude::*;

use leafwing_input_manager::prelude::*;

use crate::player::actions::PlayerAction;

use crate::player::components::{Player, PlayerConfig};

use crate::player::states::{PlayerMoveState, PlayerState};

pub struct Locomotion;

impl PlayerState for Locomotion {
	const VARIANT: PlayerMoveState = PlayerMoveState::Locomotion;
}

impl Locomotion {
	pub fn on_enter() {}

	pub fn on_exit() {}

	#[allow(clippy::type_complexity)]
	pub fn on_update(
		_time: Res<Time>,
		q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<Player>)>,
		keyboard: Res<ButtonInput<KeyCode>>,
		mut q_player: Query<
			(
				&mut TnuaController,
				&GlobalTransform,
				&ActionState<PlayerAction>,
				&mut Player,
				&PlayerConfig,
			),
			With<Player>,
		>,
		mut _gizmos: Gizmos,
	) {
		let cam_yaw_forward = || -> Vec3 {
			if let Some(xf) = q_cam.iter().next() {
				let f = xf.compute_transform().forward();
				Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
			} else {
				Vec3::Z
			}
		};

		for (mut ctrl, _player_xf, actions, mut player, pconf) in q_player.iter_mut() {
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
				float_height: 1.2,
				cling_distance: 0.4,
				spring_strength: 800.0,
				spring_dampening: 1.4,
				acceleration: 80.0,
				air_acceleration: 6.0,
				free_fall_extra_gravity: 80.0,
				tilt_offset_angvel: 0.0,
				tilt_offset_angacl: 0.0,
				turning_angvel: 10.0,
				max_slope: pconf.walk_max_slope_rad,
				..Default::default()
			});

			if jump_pressed {
				ctrl.action(TnuaBuiltinJump {
					height: 3.5,
					allow_in_air: false,
					..Default::default()
				});
			}

			if wish_dir != Vec3::ZERO {
				let current = player.facing.normalize_or_zero();
				let target = wish_dir.normalize_or_zero();
				let smooth = 0.2;
				let dot = current.dot(target).clamp(-1.0, 1.0);
				let angle = dot.acos();

				if angle > 1e-4 {
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

			#[cfg(feature = "debug_gizmos")]
			{
				let p = _player_xf.translation() + Vec3::Y * 0.8;
				let col = if boosting {
					Color::srgb(1.0, 0.6, 0.2)
				} else {
					Color::srgb(0.2, 0.6, 1.0)
				};
				let len = if speed > 0.0 { speed * 0.15 } else { 0.0 };
				_gizmos.arrow(p, p + wish_dir * len, col);
				_gizmos.arrow(p, p + yaw_fwd * 0.6, Color::srgb(0.6, 0.6, 0.6));
				_gizmos.arrow(p, p + yaw_right * 0.6, Color::srgb(0.6, 0.6, 0.6));
			}
		}
	}
}
