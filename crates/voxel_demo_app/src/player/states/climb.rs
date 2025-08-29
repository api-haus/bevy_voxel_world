use avian3d::prelude as avian;
use bevy::prelude::*;
use bevy_tnua::builtins::TnuaBuiltinClimb;
use bevy_tnua::math::AsF32;
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
	pub fn on_enter() {}

	pub fn on_exit() {}

	#[allow(clippy::type_complexity)]
	pub fn on_update(
		_time: Res<Time>,
		mut q: Query<
			(
				&mut TnuaController,
				&GlobalTransform,
				&ActionState<PlayerAction>,
				&mut Player,
				&PlayerInput,
				&bevy_tnua::TnuaObstacleRadar,
			),
			With<crate::player::components::Player>,
		>,
		spatial_ext: TnuaSpatialExtAvian3d,
		q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::player::components::Player>)>,
		cfg: Res<crate::player::components::ClimbConfig>,
		mut _gizmos: Gizmos,
	) {
		let Ok((mut ctrl, _xf, _actions, mut player, input, radar)) = q.single_mut() else {
			return;
		};

		let radar_lens = TnuaRadarLens::new(radar, &spatial_ext);

		let mut picked = None;
		let mut best_dist = f32::MAX;
		for blip in radar_lens.iter_blips() {
			let cam_forward = if let Some(cam_xf) = q_cam.iter().next() {
				(-cam_xf.compute_transform().forward()).normalize_or_zero()
			} else {
				Vec3::Z
			};
			let n = blip.normal_from_closest_point().f32().normalize_or_zero();
			let slope_from_horizontal = (n.dot(Vec3::Y).abs()).clamp(0.0, 1.0).acos();
			let near_vertical = slope_from_horizontal > std::f32::consts::FRAC_PI_4;
			let facing_cam = n.dot(cam_forward) < -0.2;
			let d = blip
				.closest_point()
				.get()
				.distance(radar.tracked_position().f32());
			if near_vertical && facing_cam && d < best_dist {
				best_dist = d;
				picked = Some(blip);
			}
		}

		let Some(blip) = picked else {
			return;
		};

		// Camera forward, wall normal and plane axes
		let cam_forward = if let Some(cam_xf) = q_cam.iter().next() {
			(-cam_xf.compute_transform().forward()).normalize_or_zero()
		} else {
			Vec3::Z
		};
		let n = blip.normal_from_closest_point().f32();
		let mut plane_forward = (cam_forward - n * cam_forward.dot(n)).normalize_or_zero();
		if plane_forward.length_squared() < 1e-4 {
			plane_forward = (Vec3::Y - n * n.dot(Vec3::Y)).normalize_or_zero();
		}
		let plane_right = n.cross(plane_forward).normalize_or_zero();

		// Input mapping to climb params (demo-style)
		let move2d = input.move2d;
		let climb_speed = if move2d.y >= 0.0 {
			cfg.up_speed
		} else {
			cfg.down_speed
		};
		let desired_climb_velocity = Vec3::Y * (move2d.y * climb_speed);

		let inward = -n;
		let direction_to_anchor = (inward - Vec3::Y * inward.dot(Vec3::Y)).normalize_or_zero();
		let desired_vec_to_anchor = direction_to_anchor * 0.3;

		let desired_forward = bevy::math::Dir3::new(direction_to_anchor).ok();
		let initiation_direction =
			(plane_right * move2d.x + plane_forward * move2d.y).normalize_or_zero();

		ctrl.action(TnuaBuiltinClimb {
			climbable_entity: Some(blip.entity()),
			anchor: blip.closest_point().get(),
			desired_climb_velocity: desired_climb_velocity.into(),
			desired_vec_to_anchor: desired_vec_to_anchor.into(),
			desired_forward,
			initiation_direction: initiation_direction.into(),
			..Default::default()
		});

		// Do not override facing while climbing; let Tnua handle alignment via
		// desired_forward

		#[cfg(feature = "debug_gizmos")]
		{
			let _p = Vec3::ZERO;
		}
	}
}
