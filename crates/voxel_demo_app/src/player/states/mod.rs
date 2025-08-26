use bevy::prelude::*;

use tracing::info;

pub mod climb;

pub mod debugfly;

pub mod locomotion;

pub mod transitions;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default, Reflect)]
pub enum PlayerMoveState {
	#[default]
	Locomotion,
	Climb,
	DebugFly,
}

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PlayerStatesSet {
	Sense,
	Decide,
	Update,
	Post,
}

/// Contract for player movement states implemented as types.
///
/// Each concrete state type (e.g., `Locomotion`, `Climb`, `DebugFly`) should
/// implement this trait and declare which `PlayerMoveState` variant it
/// represents. Lifecycle systems and transition detection will be wired
/// separately by the plugin/macro.
pub trait PlayerState: Send + Sync + 'static {
	const VARIANT: PlayerMoveState;
}

#[derive(Event, Clone)]
pub struct PlayerTransitionIntent {
	pub to: PlayerMoveState,
	pub priority: i32,
	pub reason: &'static str,
}

pub fn resolve_transitions(
	mut ev_intents: EventReader<PlayerTransitionIntent>,
	mut next: ResMut<NextState<PlayerMoveState>>,
) {
	let intents: Vec<_> = ev_intents.read().cloned().collect();
	if intents.is_empty() {
		return;
	}

	if let Some(best) = intents.iter().max_by_key(|e| e.priority) {
		info!(
			target = ?best.to,
			priority = best.priority,
			reason = best.reason,
			count = intents.len(),
			"resolve_transitions: applying transition",
		);
		next.set(best.to);
	}
}

#[macro_export]
macro_rules! register_player_state {
	($app:expr, $state_ty:ty, $transition:path) => {{
		let __variant = <$state_ty as $crate::player::states::PlayerState>::VARIANT;
		$app
			.add_systems(OnEnter(__variant), <$state_ty>::on_enter)
			.add_systems(
				FixedUpdate,
				$transition.in_set($crate::player::states::PlayerStatesSet::Decide),
			)
			.add_systems(
				FixedUpdate,
				<$state_ty>::on_update
					.in_set($crate::player::states::PlayerStatesSet::Update)
					.run_if(in_state(__variant)),
			)
			.add_systems(OnExit(__variant), <$state_ty>::on_exit);
	}};
}

pub struct PlayerStatesPlugin;

impl Plugin for PlayerStatesPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_state::<PlayerMoveState>()
			.add_event::<PlayerTransitionIntent>()
			.configure_sets(
				FixedUpdate,
				(
					PlayerStatesSet::Sense,
					PlayerStatesSet::Decide,
					PlayerStatesSet::Update,
					PlayerStatesSet::Post,
				)
					.chain(),
			)
			.add_systems(
				FixedUpdate,
				resolve_transitions.after(PlayerStatesSet::Decide),
			);
		debug!("init player states plugin");

		// Sensors that feed transitions (e.g., climb contact detection)
		app.add_systems(
			FixedUpdate,
			crate::player::states::climb::detect_climbable.in_set(PlayerStatesSet::Sense),
		);

		// Register states via macro (enter/decide/update/exit/post)
		{
			use crate::player::states as st;
			register_player_state!(
				app,
				st::locomotion::Locomotion,
				st::transitions::from_locomotion
			);
			register_player_state!(app, st::climb::Climb, st::transitions::from_or_within_climb);
			register_player_state!(app, st::debugfly::DebugFly, st::transitions::from_debugfly);
		}
	}
}
