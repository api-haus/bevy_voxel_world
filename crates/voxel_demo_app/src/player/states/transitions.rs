use bevy::prelude::*;

use crate::player::states::{PlayerMoveState, PlayerTransitionIntent};

pub fn from_locomotion(
	mut ev: EventWriter<PlayerTransitionIntent>,
	q_player: Query<
		(
			Option<&crate::player::states::climb::ClimbContact>,
			&crate::player::components::Player,
			Option<&crate::player::states::climb::ClimbSuppress>,
		),
		With<crate::player::components::Player>,
	>,
	q_actions: Query<
		&leafwing_input_manager::prelude::ActionState<crate::player::actions::PlayerAction>,
		With<crate::player::components::Player>,
	>,
	state: Res<State<PlayerMoveState>>,
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

	// Climb detection: contact exists AND pushing roughly into the surface, and no
	// suppression active
	if let Ok((maybe_contact, player, maybe_suppress)) = q_player.single() {
		let suppressed = maybe_suppress.map(|s| s.0 > 0.0).unwrap_or(false);
		if suppressed {
			return;
		}
		if let Some(contact) = maybe_contact {
			let facing = player.facing.normalize_or_zero();
			let toward_surface = contact.normal.dot(facing) < -0.2;
			if toward_surface {
				tracing::info!("transition intent: Locomotion -> Climb (steep surface, pushing in)");
				ev.write(PlayerTransitionIntent {
					to: PlayerMoveState::Climb,
					priority: 100,
					reason: "climb_contact_pushing",
				});
			}
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
