use bevy::input::gamepad::GamepadButton;

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
	ToggleDebugFly,
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
	map.insert(PlayerAction::Boost, KeyCode::ShiftLeft);
	map.insert(PlayerAction::Punch, MouseButton::Left);
	map.insert(PlayerAction::ToggleDebugFly, KeyCode::KeyZ);
	// Gamepad chord: SELECT + START toggles debug fly
	map.insert(
		PlayerAction::ToggleDebugFly,
		ButtonlikeChord::new([GamepadButton::Select, GamepadButton::Start]),
	);
	map
}
