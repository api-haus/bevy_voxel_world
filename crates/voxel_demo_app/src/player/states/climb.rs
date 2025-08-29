use std::cmp::Ordering;

use avian3d::prelude as avian;
use bevy::prelude::*;
use bevy_tnua::builtins::{TnuaBuiltinClimb, TnuaBuiltinWalk};
use bevy_tnua::math::{AdjustPrecision, AsF32};
use bevy_tnua::prelude::*;
use bevy_tnua::radar_lens::TnuaRadarLens;
use bevy_tnua_avian3d::TnuaSpatialExtAvian3d;
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

#[derive(Component, Default)]
pub struct PrevGravityScale(pub f32);

pub fn detect_climbable(
	mut commands: Commands,
	q_player: Query<
		(
			Entity,
			&GlobalTransform,
			&Player,
			&PlayerConfig,
			&PlayerDimensions,
			&PlayerInput,
		),
		With<Player>,
	>,
	q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<Player>)>,
	mut spatial_query: avian::SpatialQuery,
	config: Res<ClimbConfig>,
	mut _gizmos: Gizmos,
) {
	let Ok((ent, xf, _player, pconf, _dims, _input)) = q_player.single() else {
		debug!("climb: no valid player");
		return;
	};

	// Ray direction: camera forward in world space
	let cam_forward = if let Some(cam_xf) = q_cam.iter().next() {
		(-cam_xf.compute_transform().forward()).normalize_or_zero()
	} else {
		Vec3::Z
	};

	let origin = xf.translation();
	let max_dist = config.detect_distance;

	spatial_query.update_pipeline();

	// Exclude the player entity from spatial queries to avoid self-hits
	let filter = avian::SpatialQueryFilter::default().with_excluded_entities([ent]);

	let mut contact: Option<ClimbContact> = None;

	if let Some(hit) = spatial_query.cast_ray(
		origin,
		Dir3::new_unchecked(cam_forward),
		max_dist,
		true,
		&filter,
	) {
		let hit_dist = hit.distance.min(max_dist);
		let n = hit.normal.normalize_or_zero();
		let surface_slope_from_horizontal = (n.dot(Vec3::Y).abs()).clamp(0.0, 1.0).acos();
		let is_steeper_than_walkable = surface_slope_from_horizontal > pconf.walk_max_slope_rad;
		let facing_dot = n.dot(cam_forward);
		let min_self_distance = 0.05;

		if is_steeper_than_walkable && facing_dot < -0.2 && hit_dist > min_self_distance {
			let point = origin + cam_forward * hit_dist;
			contact = Some(ClimbContact {
				point,
				normal: n,
				distance: hit_dist,
			});
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
			let end = start + cam_forward * hit_dist;
			let col = if is_steeper_than_walkable && facing_dot < -0.2 {
				Color::srgb(0.2, 1.0, 0.2)
			} else {
				Color::srgb(0.8, 0.2, 0.2)
			};
			_gizmos.sphere(start, 0.05, Color::srgb(0.6, 0.6, 0.6));
			_gizmos.arrow(start, end, col);
			_gizmos.arrow(end, end + n * 0.4, Color::srgb(0.9, 0.9, 0.2));
		}
	} else {
		debug!("climb: no hit");
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
		mut commands: Commands,
		mut q: Query<(Entity, &mut avian::GravityScale), With<Player>>,
	) {
		if let Ok((ent, mut g)) = q.get_single_mut() {
			let prev = g.0;
			g.0 = 0.0;
			commands.entity(ent).insert(PrevGravityScale(prev));
		}
	}

	pub fn on_exit(
		mut commands: Commands,
		mut q: Query<(Entity, &mut avian::GravityScale, Option<&PrevGravityScale>), With<Player>>,
	) {
		if let Ok((ent, mut g, prev)) = q.get_single_mut() {
			if let Some(p) = prev {
				g.0 = p.0;
			} else {
				g.0 = 1.0;
			}
			commands.entity(ent).remove::<PrevGravityScale>();
		}
	}

	#[allow(clippy::type_complexity)]
	pub fn on_update(
		_time: Res<Time>,
		mut q: Query<
			(
				&mut TnuaController,
				&GlobalTransform,
				&ActionState<PlayerAction>,
				&Player,
				&PlayerInput,
				&bevy_tnua::TnuaObstacleRadar,
				&PlayerConfig,
			),
			With<crate::player::components::Player>,
		>,
		spatial_ext: TnuaSpatialExtAvian3d,
		q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::player::components::Player>)>,
		cfg: Res<crate::player::components::ClimbConfig>,
		mut _gizmos: Gizmos,
	) {
		let Ok((mut ctrl, _xf, _actions, _player, input, obstacle_radar, pconf)) = q.single_mut()
		else {
			return;
		};

		let radar_lens = TnuaRadarLens::new(obstacle_radar, &spatial_ext);

		// Camera yaw-only forward/right for screen-space to world mapping
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

		let screen_space_direction =
			(yaw_right * input.move2d.x + yaw_fwd * input.move2d.y).clamp_length_max(1.0);
		let tn_direction = screen_space_direction.adjust_precision();

		// Prepare lateral movement along the surface (filled below when we know the
		// wall normal)
		let mut basis_lateral_velocity = Vec3::ZERO;

		// Always feed a walk basis while climbing to keep controller active
		ctrl.basis(TnuaBuiltinWalk {
			desired_velocity: Vec3::ZERO,
			float_height: 1.2,
			cling_distance: 0.4,
			spring_strength: 800.0,
			spring_dampening: 1.4,
			acceleration: 80.0,
			air_acceleration: 6.0,
			free_fall_extra_gravity: 0.0,
			tilt_offset_angvel: 0.0,
			tilt_offset_angacl: 0.0,
			turning_angvel: 10.0,
			max_slope: pconf.walk_max_slope_rad,
			..Default::default()
		});

		// Detect if already climbing on a valid blip
		let already_climbing_on = ctrl
			.concrete_action::<TnuaBuiltinClimb>()
			.and_then(|(action, _)| {
				let entity = action
					.climbable_entity
					.filter(|entity| obstacle_radar.has_blip(*entity))?;
				Some((entity, action.clone()))
			});

		let mut best_initiation: Option<(Entity, bevy_tnua::math::Vector3, Vec3, Vec3, f32)> = None;

		'blips_loop: for blip in radar_lens.iter_blips() {
			// Only consider near-vertical surfaces facing the character
			let n = blip.normal_from_closest_point().f32().normalize_or_zero();
			let slope_from_horizontal = (n.dot(Vec3::Y).abs()).clamp(0.0, 1.0).acos();
			let near_vertical = slope_from_horizontal > std::f32::consts::FRAC_PI_4;
			if !near_vertical {
				continue 'blips_loop;
			}

			if let Some((climbable_entity, prev_action)) = already_climbing_on.as_ref() {
				// Continue climbing only on the same entity
				if *climbable_entity != blip.entity() {
					continue 'blips_loop;
				}

				// Maintain initiation only if input aligns enough with previous direction
				let dot_initiation = tn_direction.dot(prev_action.initiation_direction);
				let initiation_direction = if 0.5 < dot_initiation {
					prev_action.initiation_direction
				} else {
					bevy_tnua::math::Vector3::ZERO
				};

				// Up/down speed from movement input
				let up_factor = input.move2d.y;
				let climb_speed = if up_factor >= 0.0 {
					cfg.up_speed
				} else {
					cfg.down_speed
				};
				let mut action = TnuaBuiltinClimb {
					climbable_entity: Some(blip.entity()),
					anchor: blip.closest_point().get(),
					desired_climb_velocity: (Vec3::Y * (up_factor * climb_speed)).into(),
					initiation_direction,
					// Keep previous anchor/forward for stability while continuing
					desired_vec_to_anchor: prev_action.desired_vec_to_anchor,
					desired_forward: prev_action.desired_forward,
					..Default::default()
				};

				// Compute lateral along the wall plane from input X/Y in camera space
				let wall_right = Vec3::Y.cross(n).normalize_or_zero();
				let lr = screen_space_direction.dot(wall_right);
				basis_lateral_velocity = wall_right * (cfg.lateral_speed * lr);

				// Hard stops above/below using obstacle extents
				const LOOK_ABOVE_OR_BELOW: f32 = 5.0;
				match action
					.desired_climb_velocity
					.dot(bevy_tnua::math::Vector3::Y)
					.partial_cmp(&0.0)
					.unwrap()
				{
					Ordering::Less => {
						if ctrl.is_airborne().unwrap_or(false) {
							let extent = blip.probe_extent_from_closest_point(-Dir3::Y, LOOK_ABOVE_OR_BELOW);
							if extent < 0.9 * LOOK_ABOVE_OR_BELOW {
								action.hard_stop_down =
									Some(blip.closest_point().get() - extent * bevy_tnua::math::Vector3::Y);
							}
						} else if initiation_direction == bevy_tnua::math::Vector3::ZERO {
							// On ground and no initiation - do not climb down
							action.desired_climb_velocity = bevy_tnua::math::Vector3::ZERO;
						}
					}
					Ordering::Equal => {}
					Ordering::Greater => {
						let extent = blip.probe_extent_from_closest_point(Dir3::Y, LOOK_ABOVE_OR_BELOW);
						if extent < 0.9 * LOOK_ABOVE_OR_BELOW {
							action.hard_stop_up =
								Some(blip.closest_point().get() + extent * bevy_tnua::math::Vector3::Y);
						}
					}
				}

				ctrl.action(action);
			} else {
				// Collect best candidate for initiation in case input doesn't align
				let anchor = blip.closest_point().get();
				let d = anchor
					.f32()
					.distance(obstacle_radar.tracked_position().f32());
				let direction_to_anchor = (-(n - Vec3::Y * n.dot(Vec3::Y))).normalize_or_zero();
				let score = d;
				match best_initiation {
					Some((_e, _a, _dir_to_anchor, _n, best_score)) if best_score <= score => {}
					_ => {
						best_initiation = Some((blip.entity(), anchor, direction_to_anchor, n, score));
					}
				}
			}
		}

		// If not already climbing on a specific entity, initiate climb on the nearest
		// valid blip
		if already_climbing_on.is_none() {
			if let Some((entity, anchor, direction_to_anchor, n, _score)) = best_initiation {
				ctrl.action(TnuaBuiltinClimb {
					climbable_entity: Some(entity),
					anchor,
					desired_vec_to_anchor: (direction_to_anchor * cfg.target_distance).into(),
					desired_forward: Dir3::new(direction_to_anchor).ok(),
					initiation_direction: tn_direction.normalize_or_zero(),
					..Default::default()
				});

				// Lateral along the wall plane on initiation
				let wall_right = Vec3::Y.cross(n).normalize_or_zero();
				let lr = screen_space_direction.dot(wall_right);
				basis_lateral_velocity = wall_right * (cfg.lateral_speed * lr);
			}
		}

		// Feed the basis with lateral velocity (keeps controller active and allows
		// left/right)
		ctrl.basis(TnuaBuiltinWalk {
			desired_velocity: basis_lateral_velocity,
			float_height: 1.2,
			cling_distance: 0.4,
			spring_strength: 800.0,
			spring_dampening: 1.4,
			acceleration: 80.0,
			air_acceleration: 6.0,
			free_fall_extra_gravity: 0.0,
			tilt_offset_angvel: 0.0,
			tilt_offset_angacl: 0.0,
			turning_angvel: 10.0,
			max_slope: pconf.walk_max_slope_rad,
			..Default::default()
		});

		// Let Tnua handle facing via desired_forward while climbing

		#[cfg(feature = "debug_gizmos")]
		{
			let _p = Vec3::ZERO;
		}
	}
}
