pub mod abilities;

pub mod actions;

// merged: climb systems/state now live under states::climb

pub mod components;

pub mod orientation;

pub mod punch;

pub mod spawn;

pub mod states;

pub use components::Player;

use bevy::prelude::*;

use leafwing_abilities::prelude::AbilityPlugin;

use leafwing_input_manager::prelude::InputManagerPlugin;

pub struct PlayerPlugin;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PlayerSet {
	Input,
	Movement,
	Post,
}

impl Plugin for PlayerPlugin {
	fn build(&self, app: &mut App) {
		app
			.add_plugins(states::PlayerStatesPlugin)
			.init_resource::<components::ClimbConfig>()
			// Input/abilities local to the player feature
			.add_plugins((
				InputManagerPlugin::<actions::PlayerAction>::default(),
				AbilityPlugin::<abilities::PlayerAbility>::default(),
			))
			.configure_sets(
				FixedUpdate,
				(PlayerSet::Input, PlayerSet::Movement, PlayerSet::Post).chain(),
			)
			.add_systems(Startup, spawn::setup_player)
			.add_systems(
				FixedUpdate,
				(
					punch::punch_attack.in_set(PlayerSet::Post),
					#[cfg(feature = "debug_gizmos")]
					punch::draw_and_cleanup_punch_gizmos.in_set(PlayerSet::Post),
					#[cfg(feature = "debug_gizmos")]
					visualize_state_gizmo.in_set(PlayerSet::Post),
					#[cfg(feature = "debug_gizmos")]
					orientation::gizmo::visualize_player_orientation.in_set(PlayerSet::Post),
				),
			);
	}
}

#[cfg(feature = "debug_gizmos")]
fn visualize_state_gizmo(
	mut gizmos: Gizmos,
	state: Res<State<states::PlayerMoveState>>,
	q_player: Query<&GlobalTransform, With<Player>>,
) {
	let Ok(xf) = q_player.single() else {
		return;
	};
	let p = xf.translation() + Vec3::Y * 1.6;
	let color = match state.get() {
		states::PlayerMoveState::Locomotion => Color::srgb(0.2, 1.0, 0.2),
		states::PlayerMoveState::Climb => Color::srgb(0.2, 0.6, 1.0),
		states::PlayerMoveState::DebugFly => Color::srgb(1.0, 0.2, 0.2),
	};
	gizmos.sphere(p, 0.1, color);
}
