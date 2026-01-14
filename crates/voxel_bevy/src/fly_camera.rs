//! Fly camera controller with WASD movement and mouse look.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

/// Fly camera component for first-person-style navigation.
#[derive(Component)]
pub struct FlyCamera {
  /// Movement speed in units per second.
  pub speed: f32,
  /// Mouse sensitivity in radians per pixel.
  pub sensitivity: f32,
  /// Current yaw (horizontal rotation) in radians.
  pub yaw: f32,
  /// Current pitch (vertical rotation) in radians.
  pub pitch: f32,
}

impl Default for FlyCamera {
  fn default() -> Self {
    Self {
      speed: 50.0,
      sensitivity: 0.003,
      yaw: 0.0,
      pitch: 0.0,
    }
  }
}

/// System to update fly camera based on input.
pub fn update_fly_camera(
  time: Res<Time>,
  keys: Res<ButtonInput<KeyCode>>,
  mouse_button: Res<ButtonInput<MouseButton>>,
  mouse_motion: Res<AccumulatedMouseMotion>,
  mut query: Query<(&mut FlyCamera, &mut Transform)>,
) {
  let Ok((mut fly, mut transform)) = query.single_mut() else {
    return;
  };

  // Mouse look (right-click drag)
  if mouse_button.pressed(MouseButton::Right) {
    let delta = mouse_motion.delta;
    fly.yaw -= delta.x * fly.sensitivity;
    fly.pitch -= delta.y * fly.sensitivity;
    // Clamp pitch to prevent gimbal lock
    fly.pitch = fly.pitch.clamp(-1.5, 1.5);
  }

  // Build rotation from yaw/pitch (YXZ euler order)
  let rotation = Quat::from_euler(EulerRot::YXZ, fly.yaw, fly.pitch, 0.0);
  transform.rotation = rotation;

  // WASD movement
  let mut velocity = Vec3::ZERO;
  let forward = transform.forward();
  let right = transform.right();

  if keys.pressed(KeyCode::KeyW) {
    velocity += *forward;
  }
  if keys.pressed(KeyCode::KeyS) {
    velocity -= *forward;
  }
  if keys.pressed(KeyCode::KeyD) {
    velocity += *right;
  }
  if keys.pressed(KeyCode::KeyA) {
    velocity -= *right;
  }
  if keys.pressed(KeyCode::Space) {
    velocity += Vec3::Y;
  }
  if keys.pressed(KeyCode::ShiftLeft) {
    velocity -= Vec3::Y;
  }

  if velocity.length_squared() > 0.0 {
    velocity = velocity.normalize();
  }

  // Sprint with Ctrl
  let speed = if keys.pressed(KeyCode::ControlLeft) {
    fly.speed * 3.0
  } else {
    fly.speed
  };

  transform.translation += velocity * speed * time.delta_secs();
}
