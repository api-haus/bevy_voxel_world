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
			walk_max_slope_rad: std::f32::consts::FRAC_PI_3, // 60° default
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
	/// Desired adhesion distance from wall measured along hit.normal
	/// Used for snap positioning: position = hit.point - hit.normal *
	/// target_distance
	pub target_distance: f32,
	pub adhesion_kp: f32,
	pub max_inward_speed: f32,
	// Climb re-entry suppression after jump exit
	pub climb_reenter_suppress_secs: f32,
}

impl Default for ClimbConfig {
	fn default() -> Self {
		Self {
			detect_distance: 4.0,
			engage_distance: 4.0,
			up_speed: 3.5,
			down_speed: 3.0,
			lateral_speed: 2.0,
			target_distance: 0.35,
			adhesion_kp: 8.0,
			max_inward_speed: 3.0,
			climb_reenter_suppress_secs: 0.3,
		}
	}
}
