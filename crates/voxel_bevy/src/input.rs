//! Cross-platform input handling for camera controls.
//!
//! Uses bevy_enhanced_input for unified keyboard/mouse and gamepad support.

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::fly_camera::{FlyCamera, FlyCameraInput};

// =============================================================================
// Actions
// =============================================================================

/// Movement action (WASD or left stick) - outputs Vec2.
#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub struct Move;

/// Look action (mouse delta or right stick) - outputs Vec2.
#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub struct Look;

/// Move up action (Space or right trigger) - outputs bool.
#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct MoveUp;

/// Move down action (Shift or left trigger) - outputs bool.
#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct MoveDown;

/// Sprint modifier (Ctrl or left bumper) - outputs bool.
#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct Sprint;

/// Enable mouse look (right mouse button) - outputs bool.
#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct EnableLook;

// =============================================================================
// Input Context
// =============================================================================

/// Input context marker for camera controls.
#[derive(Component)]
pub struct CameraInputContext;

/// Returns a bundle with FlyCamera, input context, and all action bindings.
/// Use this when spawning or inserting onto a camera entity.
pub fn fly_camera_input_bundle(fly_camera: FlyCamera) -> impl Bundle {
	(
		fly_camera,
		FlyCameraInput::default(),
		CameraInputContext,
		actions!(CameraInputContext[
			// Movement: WASD + Left Stick
			(
				Action::<Move>::default(),
				Bindings::spawn((Cardinal::wasd_keys(), Axial::left_stick())),
			),
			// Look: Right Stick
			(
				Action::<Look>::default(),
				Bindings::spawn(Axial::right_stick()),
			),
			// Look: Mouse motion (separate binding)
			(
				Action::<Look>::default(),
				bindings![Binding::mouse_motion()],
			),
			// Move Up: Space + Right Trigger
			(
				Action::<MoveUp>::default(),
				bindings![KeyCode::Space, GamepadButton::RightTrigger],
			),
			// Move Down: Shift + Left Trigger
			(
				Action::<MoveDown>::default(),
				bindings![KeyCode::ShiftLeft, GamepadButton::LeftTrigger],
			),
			// Sprint: Ctrl + Left Bumper
			(
				Action::<Sprint>::default(),
				bindings![KeyCode::ControlLeft, GamepadButton::LeftTrigger2],
			),
			// Enable Look: Right Mouse Button
			(
				Action::<EnableLook>::default(),
				bindings![MouseButton::Right],
			),
		]),
	)
}

// =============================================================================
// Plugin
// =============================================================================

/// Plugin that registers the input manager for camera controls.
pub struct CameraInputPlugin;

impl Plugin for CameraInputPlugin {
	fn build(&self, app: &mut App) {
		use crate::fly_camera::{register_fly_camera_observers, update_fly_camera};

		app.add_plugins(EnhancedInputPlugin)
			.add_input_context::<CameraInputContext>()
			.add_systems(Update, update_fly_camera);

		register_fly_camera_observers(app);
	}
}
