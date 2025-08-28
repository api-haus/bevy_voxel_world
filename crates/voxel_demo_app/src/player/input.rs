use bevy::input::gamepad::{Gamepad, GamepadAxis};
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::player::actions::PlayerAction;
use crate::player::components::Player;

#[derive(Component, Default, Clone, Copy, Debug)]
pub struct PlayerInput {
	pub move2d: Vec2,
	pub look2d: Vec2,
	pub jump: bool,
	pub boost: bool,
	pub punch: bool,
}

/// Gather input from the input map into a stable `PlayerInput` snapshot.
/// Run this in `Update`; movement controllers can consume it in `FixedUpdate`.
pub fn update_player_input(
	keyboard: Res<ButtonInput<KeyCode>>,
	q_gamepads: Query<&Gamepad>,
	mut q: Query<(&mut ActionState<PlayerAction>, &mut PlayerInput), With<Player>>,
) {
	for (mut actions, mut input) in q.iter_mut() {
		let mut v = actions.axis_pair(&PlayerAction::Move2D);

		if v == Vec2::ZERO {
			let x =
				(keyboard.pressed(KeyCode::KeyD) as i32 - keyboard.pressed(KeyCode::KeyA) as i32) as f32;
			let y =
				(keyboard.pressed(KeyCode::KeyW) as i32 - keyboard.pressed(KeyCode::KeyS) as i32) as f32;
			v = Vec2::new(x, y);
		}

		if v == Vec2::ZERO {
			if let Some(gp) = q_gamepads.iter().next() {
				let x = gp.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
				let y = gp.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
				v = Vec2::new(x, y);
			}
		}

		v = v.clamp_length_max(1.0);
		input.move2d = v;

		// Look and buttons
		let look = actions
			.axis_pair(&PlayerAction::Look2D)
			.clamp_length_max(1.0);
		input.look2d = look;
		input.jump = actions.pressed(&PlayerAction::Jump);
		input.boost = actions.pressed(&PlayerAction::Boost);

		// Buttons
		input.punch = actions.pressed(&PlayerAction::Punch);

		// Mirror into ActionState for universal access
		actions.set_axis_pair(&PlayerAction::Move2D, v);
		actions.set_axis_pair(&PlayerAction::Look2D, look);
	}
}
