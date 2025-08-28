use avian3d::prelude as avian;
use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use tracing::debug;

use crate::player::actions::PlayerAction;
use crate::player::components::{ClimbConfig, Player, PlayerConfig, PlayerDimensions};
use crate::player::input::PlayerInput;
use crate::player::states::{PlayerMoveState, PlayerState};

#[derive(Component, Default, Clone, Copy)]
pub struct ClimbContact {
	pub point: Vec3,
	pub normal: Vec3,
	pub distance: f32,
}

#[derive(Component, Default)]
pub struct ClimbSticky;

#[derive(Component, Default)]
pub struct ClimbSuppress(pub f32);

pub fn detect_climbable(
	mut commands: Commands,
	q_player: Query<
		(
			Entity,
			&GlobalTransform,
			&Player,
			&PlayerConfig,
			&PlayerDimensions,
		),
		With<Player>,
	>,
	mut spatial_query: avian::SpatialQuery,
	config: Res<ClimbConfig>,
	mut _gizmos: Gizmos,
) {
	let Ok((ent, xf, player, pconf, dims)) = q_player.single() else {
		debug!("climb: no valid player");
		return;
	};
	let origin = xf.translation();
	let mut move_dir = player.facing.normalize_or_zero();

	if move_dir == Vec3::ZERO {
		move_dir = Vec3::Z;
	}

	let chest = origin + Vec3::Y * 0.8;
	let samples = [chest];
	let max_dist = config.detect_distance;

	spatial_query.update_pipeline();

	// Exclude the player entity from spatial queries to avoid self-hits
	let filter = avian::SpatialQueryFilter::default().with_excluded_entities([ent]);

	let mut contact: Option<ClimbContact> = None;
	let mut best: Option<(Vec3, f32)> = None;

	for s in samples.into_iter() {
		let origin = s + move_dir * (dims.radius + 0.05);

		if let Some(hit) = spatial_query.cast_ray(
			origin,
			Dir3::new_unchecked(move_dir),
			max_dist,
			true,
			&filter,
		) {
			let hit_dist = hit.distance.min(max_dist);
			let n = hit.normal.normalize_or_zero();
			let surface_slope_from_horizontal = (n.dot(Vec3::Y).abs()).clamp(0.0, 1.0).acos();
			let is_steeper_than_walkable = surface_slope_from_horizontal > pconf.walk_max_slope_rad;
			let facing_dot = n.dot(move_dir);
			let min_self_distance = 0.1;

			if is_steeper_than_walkable
				&& facing_dot < -0.2
				&& hit_dist > min_self_distance
				&& best.map(|(_, d)| hit_dist < d).unwrap_or(true)
			{
				best = Some((n, hit_dist));
			}

			debug!(
				hit_distance = hit_dist,
				facing_dot,
				slope = surface_slope_from_horizontal,
				is_steeper_than_walkable,
				"climb: ray hit"
			);
			#[cfg(feature = "debug_gizmos")]
			{
				let start = origin;
				let end = start + move_dir * hit_dist;
				let col = if is_steeper_than_walkable && facing_dot < -0.2 {
					Color::srgb(0.2, 1.0, 0.2)
				} else {
					Color::srgb(0.8, 0.2, 0.2)
				};
				_gizmos.sphere(s, 0.06, Color::srgb(0.9, 0.9, 0.9));
				_gizmos.sphere(start, 0.05, Color::srgb(0.6, 0.6, 0.6));
				_gizmos.arrow(start, end, col);
				_gizmos.arrow(end, end + n * 0.4, Color::srgb(0.9, 0.9, 0.2));
			}
		} else {
			debug!("climb: no hit");
		}
	}

	if let Some((n, d)) = best {
		if d <= config.engage_distance {
			contact = Some(ClimbContact {
				point: chest + move_dir * d,
				normal: n,
				distance: d,
			});
			debug!(distance = d, normal = ?n, "climb: contact within engage distance");
		} else {
			commands.entity(ent).remove::<ClimbContact>();
			debug!(
				distance = d,
				"climb: contact too far; removing ClimbContact"
			);
		}
	}

	if let Some(c) = contact {
		commands.entity(ent).insert(c);
		commands.entity(ent).insert(ClimbSticky);
		#[cfg(feature = "debug_gizmos")]
		{
			_gizmos.sphere(c.point, 0.1, Color::srgb(1.0, 0.2, 0.2));
			_gizmos.arrow(
				c.point,
				c.point + c.normal * 0.5,
				Color::srgb(0.9, 0.9, 0.2),
			);
			let right = Vec3::Y.cross(c.normal).normalize_or_zero();
			_gizmos.arrow(
				c.point,
				c.point + right * 0.5,
				Color::srgb(0.95, 0.35, 0.35),
			);
		}
	} else {
		commands.entity(ent).remove::<ClimbContact>();
		debug!("climb: no valid contact; removing ClimbContact");
	}
}

pub struct Climb;

impl PlayerState for Climb {
	const VARIANT: PlayerMoveState = PlayerMoveState::Climb;
}

impl Climb {
	pub fn on_enter(
		mut q: Query<
			(
				&mut avian::RigidBody,
				Option<&mut avian::GravityScale>,
				Option<&mut avian::LinearVelocity>,
				Option<&mut avian::AngularVelocity>,
				Entity,
			),
			With<Player>,
		>,
		mut commands: Commands,
	) {
		for (mut body, grav, lin, ang, ent) in q.iter_mut() {
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

			// Remove TnuaController so its systems do not affect the player while climbing
			commands
				.entity(ent)
				.remove::<bevy_tnua::controller::TnuaController>();
		}
	}

	pub fn on_exit(
		mut q: Query<
			(
				&mut avian::RigidBody,
				Option<&mut avian::GravityScale>,
				Entity,
			),
			With<Player>,
		>,
		mut commands: Commands,
	) {
		for (mut body, grav, ent) in q.iter_mut() {
			*body = avian::RigidBody::Dynamic;
			if let Some(mut g) = grav {
				*g = avian::GravityScale(1.0);
			}

			// Restore TnuaController (default) so locomotion resumes
			commands
				.entity(ent)
				.insert(bevy_tnua::controller::TnuaController::default());
		}
	}

	#[allow(clippy::type_complexity)]
	pub fn on_update(
		time: Res<Time>,
		mut q: Query<
			(
				&mut Transform,
				&GlobalTransform,
				&ActionState<PlayerAction>,
				&mut Player,
				&PlayerInput,
				Option<&ClimbContact>,
			),
			With<crate::player::components::Player>,
		>,
		_q_gamepads: Query<&Gamepad>,
		cfg: Res<crate::player::components::ClimbConfig>,
	) {
		let Ok((mut t, xf, _actions, mut player, input, contact)) = q.single_mut() else {
			return;
		};

		let Some(contact) = contact else {
			return;
		};
		// Ensure all fields are considered used regardless of debug feature gates
		let _ = contact.point;
		let normal = contact.normal.normalize_or_zero();

		// Build climb plane basis
		let mut up_tangent = (Vec3::Y - normal * normal.dot(Vec3::Y)).normalize_or_zero();
		if up_tangent.length_squared() < 1e-4 {
			// Fallback if normal is nearly vertical
			up_tangent = (Vec3::X - normal * normal.dot(Vec3::X)).normalize_or_zero();
		}
		let right_tangent = up_tangent.cross(normal).normalize_or_zero();

		// Read input and map onto plane tangents: prefer PlayerInput snapshot
		let move2d = input.move2d;

		let lateral = right_tangent * move2d.x * cfg.lateral_speed;
		let vertical_speed = if move2d.y >= 0.0 {
			cfg.up_speed
		} else {
			cfg.down_speed
		};
		let vertical = up_tangent * move2d.y.abs() * vertical_speed;

		// Adhesion PD toward target wall distance (derivative term omitted for
		// kinematic simplicity)
		let distance_error = contact.distance - cfg.target_distance; // positive => too far, move inward
		let adhesion_speed =
			(cfg.adhesion_kp * distance_error).clamp(-cfg.max_inward_speed, cfg.max_inward_speed);
		let adhesion = -normal * adhesion_speed;

		// Desired plane motion; ensure no normal component in the tangential wish
		let mut wish = lateral + vertical;
		wish -= normal * wish.dot(normal);
		let desired = wish + adhesion;

		let dt = time.delta_secs();
		if desired != Vec3::ZERO {
			t.translation += desired * dt;
		}

		// Update visual facing toward the surface (into the wall)
		let target_dir = (-normal).normalize_or_zero();
		if target_dir != Vec3::ZERO {
			let current = player.facing.normalize_or_zero();
			let target = target_dir;
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

		debug!(
			move2d = ?move2d,
			right = ?right_tangent,
			normal = ?normal,
			lateral = ?lateral,
			vertical = ?vertical,
			adhesion = ?adhesion,
			desired = ?desired,
			pos = ?xf.translation(),
			"climb: kinematic control"
		);
	}
}
