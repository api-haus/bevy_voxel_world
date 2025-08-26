use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
// no gamepad mapping for movement yet; only WASD

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerAction {
	MoveLeft,
	MoveRight,
	MoveForward,
	MoveBack,
	Jump,
	Boost,
	Punch,
}

pub fn default_input_map() -> InputMap<PlayerAction> {
	let mut map = InputMap::default();
	// WASD movement
	map.insert(PlayerAction::MoveLeft, KeyCode::KeyA);
	map.insert(PlayerAction::MoveRight, KeyCode::KeyD);
	map.insert(PlayerAction::MoveForward, KeyCode::KeyW);
	map.insert(PlayerAction::MoveBack, KeyCode::KeyS);
	// Jump / Boost / Punch
	map.insert(PlayerAction::Jump, KeyCode::Space);
	map.insert(PlayerAction::Boost, KeyCode::ControlLeft);
	map.insert(PlayerAction::Punch, KeyCode::KeyE);
	map
}
