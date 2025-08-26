use bevy::prelude::*;

use crate::player::states::{PlayerMoveState, PlayerTransitionIntent};

pub fn from_locomotion(
	mut ev: EventWriter<PlayerTransitionIntent>,
	q_player: Query<
		&crate::player::states::climb::ClimbContact,
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

	// Climb detection
	if q_player.single().is_ok() {
		tracing::info!("transition intent: Locomotion -> Climb (climb_contact present)");
		ev.write(PlayerTransitionIntent {
			to: PlayerMoveState::Climb,
			priority: 100,
			reason: "climb_contact",
		});
	}
}

pub fn from_or_within_climb(
	mut ev: EventWriter<PlayerTransitionIntent>,
	state: Res<State<PlayerMoveState>>,
	q: Query<
		Option<&crate::player::states::climb::ClimbContact>,
		With<crate::player::components::Player>,
	>,
) {
	let in_climb = state.get() == &PlayerMoveState::Climb;
	let has_contact = q.single().ok().flatten().is_some();

	if !in_climb && has_contact {
		tracing::info!("transition intent: Locomotion -> Climb (contact)");
		ev.write(PlayerTransitionIntent {
			to: PlayerMoveState::Climb,
			priority: 100,
			reason: "climb_enter",
		});
	} else if in_climb && !has_contact {
		tracing::info!("transition intent: Climb -> Locomotion (lost contact)");
		ev.write(PlayerTransitionIntent {
			to: PlayerMoveState::Locomotion,
			priority: 90,
			reason: "climb_exit",
		});
	}
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
