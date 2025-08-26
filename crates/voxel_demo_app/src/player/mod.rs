pub mod abilities;
pub mod actions;
pub mod components;
pub mod movement;
pub mod orientation;
pub mod punch;
pub mod spawn;

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
			// Input/abilities local to the player feature
			.add_plugins((
				InputManagerPlugin::<actions::PlayerAction>::default(),
				AbilityPlugin::<abilities::PlayerAbility>::default(),
			))
			.configure_sets(
				Update,
				(PlayerSet::Input, PlayerSet::Movement, PlayerSet::Post).chain(),
			)
			.add_systems(Startup, spawn::setup_player)
			.add_systems(
				Update,
				(
					movement::control_player.in_set(PlayerSet::Movement),
					punch::punch_attack.in_set(PlayerSet::Post),
					punch::draw_and_cleanup_punch_gizmos.in_set(PlayerSet::Post),
					movement::visualize_player_movement.in_set(PlayerSet::Post),
					movement::visualize_jump_input.in_set(PlayerSet::Post),
					movement::visualize_grounded_state.in_set(PlayerSet::Post),
					orientation::gizmo::visualize_player_orientation.in_set(PlayerSet::Post),
				),
			);
	}
}
