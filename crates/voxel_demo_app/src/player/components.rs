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

#[derive(Component, Clone, Copy)]
pub struct PlayerConfig {
	/// Max walkable slope angle in radians (TnuaBuiltinWalk::max_slope)
	pub walk_max_slope_rad: f32,
}

impl Default for PlayerConfig {
	fn default() -> Self {
		Self {
			walk_max_slope_rad: std::f32::consts::FRAC_PI_4, // 45° default
		}
	}
}

#[derive(Resource)]
pub struct ClimbConfig {
	pub detect_distance: f32,
	pub engage_distance: f32,
	pub up_speed: f32,
	pub down_speed: f32,
	pub lateral_speed: f32,
	pub stick_inward_speed: f32,
}

impl Default for ClimbConfig {
	fn default() -> Self {
		Self {
			detect_distance: 4.0,
			engage_distance: 4.0,
			up_speed: 3.5,
			down_speed: 3.0,
			lateral_speed: 2.0,
			stick_inward_speed: 1.0,
		}
	}
}

// Removed: punch cooldown is handled by `leafwing_abilities` CooldownState
