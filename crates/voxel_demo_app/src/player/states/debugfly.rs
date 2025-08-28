use avian3d::prelude as avian;
use bevy::prelude::*;

use crate::player::components::Player;
use crate::player::input::PlayerInput;
use crate::player::states::{PlayerMoveState, PlayerState};

pub struct DebugFly;

impl PlayerState for DebugFly {
	const VARIANT: PlayerMoveState = PlayerMoveState::DebugFly;
}

impl DebugFly {
	pub fn on_enter(
		mut q: Query<
			(
				&mut avian::RigidBody,
				Option<&mut avian::GravityScale>,
				Option<&mut avian::LinearVelocity>,
				Option<&mut avian::AngularVelocity>,
			),
			With<Player>,
		>,
	) {
		for (mut body, grav, lin, ang) in q.iter_mut() {
			*body = avian::RigidBody::Kinematic;
			if let Some(mut g) = grav {
				*g = avian::GravityScale(0.0);
			}
			if let Some(mut v) = lin {
				v.0 = Vec3::ZERO;
			}
			if let Some(mut w) = ang {
				w.0 = Vec3::ZERO;
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
		mut q_player: Query<(&mut Transform, &PlayerInput), With<Player>>,
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

		for (mut t, input) in q_player.iter_mut() {
			let move2d = input.move2d;

			let move2d = move2d.clamp_length_max(1.0);
			let mut wish = yaw_right * move2d.x + yaw_fwd * move2d.y;

			if input.jump {
				wish += Vec3::Y;
			}

			if input.boost {
				wish -= Vec3::Y;
			}

			let speed = if input.boost { 24.0 } else { 12.0 };
			let dir = wish.normalize_or_zero();

			if dir != Vec3::ZERO {
				t.translation += dir * speed * time.delta_secs();
			}
		}
	}
}
