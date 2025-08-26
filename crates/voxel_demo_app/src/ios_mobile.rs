#[cfg(target_os = "ios")]
use bevy::{
	color::palettes::basic::*,
	input::{gestures::RotationGesture, touch::TouchPhase},
	log::{Level, LogPlugin},
	prelude::*,
	window::{AppLifecycle, WindowMode},
	winit::WinitSettings,
};

/// Bevy plugin for iOS that configures winit for mobile-friendly behavior
/// (reduced CPU usage when idle by waiting more aggressively for input).
#[cfg(target_os = "ios")]
pub struct IosMobilePlugin;

#[cfg(target_os = "ios")]
impl Plugin for IosMobilePlugin {
	fn build(&self, app: &mut App) {
		// Make the winit loop wait more aggressively when no user input is received
		// This helps reduce CPU usage on mobile devices
		app.insert_resource(WinitSettings::mobile());

		app.add_systems(
			Update,
			(
				touch_camera,
				button_handler,
				// Only run the lifetime handler when an [`AudioSink`] component exists in the world.
				// This ensures we don't try to manage audio that hasn't been initialized yet.
				handle_lifetime.run_if(any_with_component::<AudioSink>),
			),
		);
	}
}

#[cfg(target_os = "ios")]
fn touch_camera(
	window: Query<&Window>,
	mut touches: EventReader<TouchInput>,
	mut camera_transform: Single<&mut Transform, With<Camera3d>>,
	mut last_position: Local<Option<Vec2>>,
	mut rotations: EventReader<RotationGesture>,
) {
	let Ok(window) = window.single() else {
		return;
	};

	for touch in touches.read() {
		if touch.phase == TouchPhase::Started {
			*last_position = None;
		}

		if let Some(last_position) = *last_position {
			**camera_transform = Transform::from_xyz(
				camera_transform.translation.x
					+ (touch.position.x - last_position.x) / window.width() * 5.0,
				camera_transform.translation.y,
				camera_transform.translation.z
					+ (touch.position.y - last_position.y) / window.height() * 5.0,
			)
			.looking_at(Vec3::ZERO, Vec3::Y);
		}

		*last_position = Some(touch.position);
	}
	// Rotation gestures only work on iOS
	for rotation in rotations.read() {
		let forward = camera_transform.forward();
		camera_transform.rotate_axis(forward, rotation.0 / 10.0);
	}
}

#[cfg(target_os = "ios")]
fn button_handler(
	mut interaction_query: Query<
		(&Interaction, &mut BackgroundColor),
		(Changed<Interaction>, With<Button>),
	>,
) {
	for (interaction, mut color) in &mut interaction_query {
		match *interaction {
			Interaction::Pressed => {
				*color = BLUE.into();
			}

			Interaction::Hovered => {
				*color = GRAY.into();
			}

			Interaction::None => {
				*color = WHITE.into();
			}
		}
	}
}

// Pause audio when app goes into background and resume when it returns.
// This is handled by the OS on iOS, but not on Android.
#[cfg(target_os = "ios")]
fn handle_lifetime(
	mut lifecycle_events: EventReader<AppLifecycle>,
	music_controller: Single<&AudioSink>,
) {
	for event in lifecycle_events.read() {
		match event {
			AppLifecycle::Idle | AppLifecycle::WillSuspend | AppLifecycle::WillResume => {}

			AppLifecycle::Suspended => music_controller.pause(),
			AppLifecycle::Running => music_controller.play(),
		}
	}
}
