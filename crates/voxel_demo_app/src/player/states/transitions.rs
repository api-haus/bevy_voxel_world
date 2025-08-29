use bevy::prelude::*;
use bevy_tnua::math::AsF32;
use bevy_tnua::radar_lens::TnuaRadarLens;
use bevy_tnua_avian3d::TnuaSpatialExtAvian3d;

use crate::player::states::{PlayerMoveState, PlayerTransitionIntent};

pub fn from_locomotion(
	mut ev: EventWriter<PlayerTransitionIntent>,
	q_player: Query<
		(
			&crate::player::components::Player,
			Option<&crate::player::states::climb::ClimbSuppress>,
			&crate::player::input::PlayerInput,
			&bevy_tnua::TnuaObstacleRadar,
		),
		With<crate::player::components::Player>,
	>,
	q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::player::components::Player>)>,
	q_actions: Query<
		&leafwing_input_manager::prelude::ActionState<crate::player::actions::PlayerAction>,
		With<crate::player::components::Player>,
	>,
	state: Res<State<PlayerMoveState>>,
	spatial_ext: TnuaSpatialExtAvian3d,
) {
	// Only evaluate while in Locomotion to avoid conflicting intents
	if state.get() != &PlayerMoveState::Locomotion {
		return;
	}
	// Toggle into DebugFly
	let mut debug_fly_toggle = false;

	if let Ok(actions) = q_actions.single()
		&& actions.just_pressed(&crate::player::actions::PlayerAction::ToggleDebugFly)
	{
		debug_fly_toggle = true;
	}

	if debug_fly_toggle {
		ev.write(PlayerTransitionIntent {
			to: PlayerMoveState::DebugFly,
			priority: 200,
			reason: "toggle_on",
		});
		return;
	}

	// Climb detection: radar finds a near-vertical surface in front, and no
	// suppression
	if let Ok((_player, maybe_suppress, input, radar)) = q_player.single() {
		let suppressed = maybe_suppress.map(|s| s.0 > 0.0).unwrap_or(false);
		if suppressed {
			return;
		}
		let radar_lens = TnuaRadarLens::new(radar, &spatial_ext);

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
		let move2d = input.move2d;
		let wish_dir = (yaw_right * move2d.x + yaw_fwd * move2d.y).normalize_or_zero();

		let mut found_valid_surface = false;
		for blip in radar_lens.iter_blips() {
			let n = blip.normal_from_closest_point().f32().normalize_or_zero();
			let slope_from_horizontal = (n.dot(Vec3::Y).abs()).clamp(0.0, 1.0).acos();
			let near_vertical = slope_from_horizontal > std::f32::consts::FRAC_PI_4;
			let pushing_in = wish_dir.length() > 0.1 && n.dot(wish_dir) < -0.2;
			if near_vertical && pushing_in {
				found_valid_surface = true;
				break;
			}
		}

		if found_valid_surface {
			tracing::info!("transition intent: Locomotion -> Climb (radar, pushing in)");
			ev.write(PlayerTransitionIntent {
				to: PlayerMoveState::Climb,
				priority: 100,
				reason: "climb_contact_pushing",
			});
		}
	}
}

pub fn from_or_within_climb(
	mut commands: Commands,
	mut ev: EventWriter<PlayerTransitionIntent>,
	state: Res<State<PlayerMoveState>>,
	time: Res<Time>,
	q_actions: Query<
		&leafwing_input_manager::prelude::ActionState<crate::player::actions::PlayerAction>,
		With<crate::player::components::Player>,
	>,
	q_player: Query<
		(
			Entity,
			&GlobalTransform,
			&crate::player::components::PlayerDimensions,
		),
		With<crate::player::components::Player>,
	>,
	mut q_suppress: Query<
		&mut crate::player::states::climb::ClimbSuppress,
		With<crate::player::components::Player>,
	>,
) {
	let in_climb = state.get() == &PlayerMoveState::Climb;
	let Ok((ent, _xf, _dims)) = q_player.single() else {
		return;
	};

	// Tick suppression if present
	if let Ok(mut sup) = q_suppress.get_mut(ent) {
		sup.0 = (sup.0 - time.delta_secs()).max(0.0);
	}

	if !in_climb {
		return;
	}

	// Exit only by Jump: set suppression timer
	if let Ok(actions) = q_actions.single() {
		if actions.just_pressed(&crate::player::actions::PlayerAction::Jump) {
			commands
				.entity(ent)
				.insert(crate::player::states::climb::ClimbSuppress(
					crate::player::components::ClimbConfig::default().climb_reenter_suppress_secs,
				));
			tracing::info!("transition intent: Climb -> Locomotion (jump)");
			ev.write(PlayerTransitionIntent {
				to: PlayerMoveState::Locomotion,
				priority: 200,
				reason: "climb_jump",
			});
			return;
		}
	}

	// Otherwise: stay in Climb (sticky); no other exits
}

pub fn from_debugfly(
	mut ev: EventWriter<PlayerTransitionIntent>,
	state: Res<State<PlayerMoveState>>,
	q_actions: Query<
		&leafwing_input_manager::prelude::ActionState<crate::player::actions::PlayerAction>,
		With<crate::player::components::Player>,
	>,
) {
	// Only evaluate while in DebugFly to avoid conflicting intents
	if state.get() != &PlayerMoveState::DebugFly {
		return;
	}

	let mut toggle = false;

	if let Ok(actions) = q_actions.single()
		&& actions.just_pressed(&crate::player::actions::PlayerAction::ToggleDebugFly)
	{
		toggle = true;
	}

	if toggle {
		ev.write(PlayerTransitionIntent {
			to: PlayerMoveState::Locomotion,
			priority: 200,
			reason: "toggle_off",
		});
	}
}
