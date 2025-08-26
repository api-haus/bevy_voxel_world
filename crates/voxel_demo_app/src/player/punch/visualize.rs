use bevy::prelude::*;

#[derive(Component)]
pub struct Lifetime(pub Timer);

#[derive(Component)]
pub struct PunchGizmo {
	pub origin: Vec3,
	pub tip: Vec3,
	pub center: Vec3,
	pub radius: f32,
	pub color: Color,
}

impl PunchGizmo {
	pub fn with_sphere(
		origin: Vec3,
		tip: Vec3,
		center: Vec3,
		radius: f32,
		color: Color,
	) -> (Self, Lifetime) {
		(
			Self {
				origin,
				tip,
				center,
				radius,
				color,
			},
			Lifetime(Timer::from_seconds(1.0, TimerMode::Once)),
		)
	}
}

/// Draw punch gizmos and remove them after their lifetime expires.
pub fn draw_and_cleanup_punch_gizmos(
	mut commands: Commands,
	time: Res<Time>,
	mut gizmos: Gizmos,
	mut q: Query<(Entity, &mut Lifetime, &PunchGizmo)>,
) {
	for (ent, mut lifetime, gizmo) in q.iter_mut() {
		gizmos.arrow(gizmo.origin, gizmo.tip, gizmo.color);
		if gizmo.radius > 0.0 {
			gizmos.sphere(gizmo.center, gizmo.radius, gizmo.color);
		}
		lifetime.0.tick(time.delta());
		if lifetime.0.finished() {
			commands.entity(ent).despawn_recursive();
		}
	}
}
