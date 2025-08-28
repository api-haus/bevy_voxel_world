use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

// movement supports both keyboard (WASD) and gamepad left stick

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerAction {
	// Unified 2D movement vector (keyboard WASD and gamepad left stick)
	#[actionlike(DualAxis)]
	Move2D,
	// Optional: look vector (e.g., mouse/gamepad). Not mapped by default.
	#[actionlike(DualAxis)]
	Look2D,
	Jump,
	Boost,
	Punch,
	ToggleDebugFly,
}

pub fn default_input_map() -> InputMap<PlayerAction> {
	let mut map = InputMap::default();
	// No discrete movement buttons; Move2D is read from axis_pair + gamepad axes
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
