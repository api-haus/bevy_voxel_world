use bevy::prelude::*;

#[derive(Component)]
pub struct Player {
	pub facing: Vec3,
}

impl Default for Player {
	fn default() -> Self {
		Self {
			facing: Vec3::new(0.0, 0.0, -1.0),
		}
	}
}

#[derive(Component, Default)]
pub struct PlayerOrientation;

#[derive(Component, Clone, Copy)]
pub struct PlayerDimensions {
	pub height: f32,
	pub radius: f32,
}

#[derive(Component)]
pub struct PunchCooldown(pub Timer);

impl Default for PunchCooldown {
	fn default() -> Self {
		Self(Timer::from_seconds(0.25, TimerMode::Repeating))
	}
}
