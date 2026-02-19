//! Fly camera controller with cross-platform input support.
//!
//! Supports keyboard/mouse (WASD + right-click look) and gamepad (sticks).
//! Uses bevy_enhanced_input for unified input handling.

use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

// =============================================================================
// Components
// =============================================================================

/// Fly camera component for first-person-style navigation.
#[derive(Component)]
pub struct FlyCamera {
	/// Movement speed in units per second.
	pub speed: f32,
	/// Mouse sensitivity multiplier.
	pub mouse_sensitivity: f32,
	/// Gamepad stick sensitivity (radians per second at full deflection).
	pub gamepad_sensitivity: f32,
	/// Current yaw (horizontal rotation) in radians.
	pub yaw: f32,
	/// Current pitch (vertical rotation) in radians.
	pub pitch: f32,
}

impl Default for FlyCamera {
	fn default() -> Self {
		Self {
			speed: 50.0,
			mouse_sensitivity: 0.003,
			gamepad_sensitivity: 2.0,
			yaw: 0.0,
			pitch: 0.0,
		}
	}
}

/// Fly camera state that accumulates input each frame.
/// Reset at the start of each frame by `reset_fly_camera_input`.
#[derive(Component, Default)]
pub struct FlyCameraInput {
	pub move_input: Vec2,
	pub look_input: Vec2,
	pub move_up: bool,
	pub move_down: bool,
	pub sprint: bool,
	pub enable_look: bool,
}

// =============================================================================
// Input Actions
// =============================================================================

/// Movement action (WASD or left stick) - outputs Vec2.
#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub struct Move;

/// Look action (mouse delta or right stick) - outputs Vec2.
#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub struct Look;

/// Move up action (E or right trigger) - outputs bool.
#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct MoveUp;

/// Move down action (Q or left trigger) - outputs bool.
#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct MoveDown;

/// Sprint modifier (Shift or left bumper) - outputs bool.
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
			// Move Up: E + Right Trigger
			(
				Action::<MoveUp>::default(),
				bindings![KeyCode::KeyE, GamepadButton::RightTrigger],
			),
			// Move Down: Q + Left Trigger
			(
				Action::<MoveDown>::default(),
				bindings![KeyCode::KeyQ, GamepadButton::LeftTrigger],
			),
			// Sprint: Shift + Left Bumper
			(
				Action::<Sprint>::default(),
				bindings![KeyCode::ShiftLeft, GamepadButton::LeftTrigger2],
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
// Observers
// =============================================================================

/// Observer: Movement input (WASD/left stick) - active
fn on_move(trigger: On<Fire<Move>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		// Axis inputs may fire with small values instead of Complete event
		input.move_input = if trigger.value.length() > 0.05 {
			trigger.value
		} else {
			Vec2::ZERO
		};
	}
}

/// Observer: Movement input completed (keyboard released)
fn on_move_completed(trigger: On<Complete<Move>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.move_input = Vec2::ZERO;
	}
}

/// Observer: Look input (mouse/right stick) - active
fn on_look(trigger: On<Fire<Look>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		// Axis inputs may fire with small values instead of Complete event
		input.look_input = if trigger.value.length() > 0.05 {
			trigger.value
		} else {
			Vec2::ZERO
		};
	}
}

/// Observer: Look input completed
fn on_look_completed(trigger: On<Complete<Look>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.look_input = Vec2::ZERO;
	}
}

/// Observer: Move up (Space/right trigger) - active
fn on_move_up(trigger: On<Fire<MoveUp>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.move_up = trigger.value;
	}
}

/// Observer: Move up completed
fn on_move_up_completed(trigger: On<Complete<MoveUp>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.move_up = false;
	}
}

/// Observer: Move down (Shift/left trigger) - active
fn on_move_down(trigger: On<Fire<MoveDown>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.move_down = trigger.value;
	}
}

/// Observer: Move down completed
fn on_move_down_completed(trigger: On<Complete<MoveDown>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.move_down = false;
	}
}

/// Observer: Sprint (Shift/left bumper) - active
fn on_sprint(trigger: On<Fire<Sprint>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.sprint = trigger.value;
	}
}

/// Observer: Sprint completed
fn on_sprint_completed(trigger: On<Complete<Sprint>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.sprint = false;
	}
}

/// Observer: Enable look (right mouse button) - active
fn on_enable_look(trigger: On<Fire<EnableLook>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.enable_look = trigger.value;
	}
}

/// Observer: Enable look completed
fn on_enable_look_completed(trigger: On<Complete<EnableLook>>, mut query: Query<&mut FlyCameraInput>) {
	if let Ok(mut input) = query.get_mut(trigger.context) {
		input.enable_look = false;
	}
}

// =============================================================================
// Systems
// =============================================================================

/// System to update fly camera based on accumulated input.
pub fn update_fly_camera(
	time: Res<Time>,
	mut query: Query<(&mut FlyCamera, &mut Transform, &FlyCameraInput)>,
	gamepads: Query<&Gamepad>,
) {
	let Ok((mut fly, mut transform, input)) = query.single_mut() else {
		return;
	};

	// Check if gamepad has input (for sensitivity selection)
	let gamepad_active = gamepads
		.iter()
		.any(|gp| gp.left_stick().length() > 0.1 || gp.right_stick().length() > 0.1);

	// Look: mouse requires right-click hold, gamepad is always active
	let look_enabled = input.enable_look || gamepad_active;

	if look_enabled && input.look_input.length() > 0.001 {
		// Choose sensitivity based on input source
		let (sensitivity, invert_y) = if gamepad_active && !input.enable_look {
			(fly.gamepad_sensitivity * time.delta_secs(), -1.0)
		} else {
			(fly.mouse_sensitivity, 1.0)
		};

		fly.yaw -= input.look_input.x * sensitivity;
		fly.pitch -= input.look_input.y * sensitivity * invert_y;
		fly.pitch = fly.pitch.clamp(-1.5, 1.5);
	}

	// Build rotation
	let rotation = Quat::from_euler(EulerRot::YXZ, fly.yaw, fly.pitch, 0.0);
	transform.rotation = rotation;

	// Movement
	let forward = transform.forward();
	let right = transform.right();

	let mut velocity = Vec3::ZERO;
	velocity += *forward * input.move_input.y;
	velocity += *right * input.move_input.x;

	// Vertical movement
	if input.move_up {
		velocity += Vec3::Y;
	}
	if input.move_down {
		velocity -= Vec3::Y;
	}

	// Clamp velocity magnitude to 1.0 max (keyboard gives 1.0, gamepad gives 0-1)
	if velocity.length() > 1.0 {
		velocity = velocity.normalize();
	}

	// Sprint
	let speed = if input.sprint {
		fly.speed * 3.0
	} else {
		fly.speed
	};

	transform.translation += velocity * speed * time.delta_secs();
}

// =============================================================================
// Plugin helpers
// =============================================================================

/// Register fly camera observers. Call this from your app setup.
fn register_fly_camera_observers(app: &mut App) {
	// Fire observers - set values when input is active
	app.add_observer(on_move)
		.add_observer(on_look)
		.add_observer(on_move_up)
		.add_observer(on_move_down)
		.add_observer(on_sprint)
		.add_observer(on_enable_look);

	// Complete observers - reset values when input ends
	app.add_observer(on_move_completed)
		.add_observer(on_look_completed)
		.add_observer(on_move_up_completed)
		.add_observer(on_move_down_completed)
		.add_observer(on_sprint_completed)
		.add_observer(on_enable_look_completed);
}

// =============================================================================
// Plugin
// =============================================================================

/// Plugin that registers the input manager for camera controls.
pub struct CameraInputPlugin;

impl Plugin for CameraInputPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(EnhancedInputPlugin)
			.add_input_context::<CameraInputContext>()
			.add_systems(Update, update_fly_camera);

		register_fly_camera_observers(app);
	}
}
