use avian3d::prelude as avian;

use bevy::prelude::*;

use leafwing_input_manager::prelude::ActionState;

use crate::player::actions::PlayerAction;

use crate::player::components::Player;

use crate::player::states::{PlayerMoveState, PlayerState};

pub struct DebugFly;

impl PlayerState for DebugFly {
	const VARIANT: PlayerMoveState = PlayerMoveState::DebugFly;
}

impl DebugFly {
	pub fn on_enter(
		mut q: Query<(&mut avian::RigidBody, Option<&mut avian::GravityScale>), With<Player>>,
	) {
		for (mut body, grav) in q.iter_mut() {
			*body = avian::RigidBody::Kinematic;

			if let Some(mut g) = grav {
				*g = avian::GravityScale(0.0);
			}
		}
	}

	pub fn on_exit(
		mut q: Query<(&mut avian::RigidBody, Option<&mut avian::GravityScale>), With<Player>>,
	) {
		for (mut body, grav) in q.iter_mut() {
			*body = avian::RigidBody::Dynamic;

			if let Some(mut g) = grav {
				*g = avian::GravityScale(1.0);
			}
		}
	}

	pub fn on_update(
		time: Res<Time>,
		q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<Player>)>,
		mut q_player: Query<(&mut Transform, &ActionState<PlayerAction>), With<Player>>,
	) {
		let cam_yaw_forward = || -> Vec3 {
			if let Some(xf) = q_cam.iter().next() {
				let f = xf.compute_transform().forward();
				Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
			} else {
				Vec3::Z
			}
		};

		let yaw_fwd = cam_yaw_forward();
		let yaw_right = yaw_fwd.cross(Vec3::Y).normalize_or_zero();

		for (mut t, actions) in q_player.iter_mut() {
			let x = (actions.pressed(&PlayerAction::MoveRight) as i32
				- actions.pressed(&PlayerAction::MoveLeft) as i32) as f32;
			let y = (actions.pressed(&PlayerAction::MoveForward) as i32
				- actions.pressed(&PlayerAction::MoveBack) as i32) as f32;
			let mut wish = yaw_right * x + yaw_fwd * y;

			if actions.pressed(&PlayerAction::Jump) {
				wish += Vec3::Y;
			}

			if actions.pressed(&PlayerAction::Boost) {
				wish -= Vec3::Y;
			}

			let speed = if actions.pressed(&PlayerAction::Boost) {
				24.0
			} else {
				12.0
			};
			let dir = wish.normalize_or_zero();

			if dir != Vec3::ZERO {
				t.translation += dir * speed * time.delta_secs();
			}
		}
	}
}
