use crate::player::state::PlayerMoveState;

use avian3d::prelude as avian;

use bevy::prelude::*;

use bevy_enhanced_input::prelude::*;

use bevy_voxel_plugin::plugin::EditOp;

use bevy_voxel_plugin::plugin::VoxelEditEvent;

use bevy_voxel_plugin::plugin::VoxelVolumeDesc;

#[derive(Component)]
pub struct FlyCamCtx;

#[derive(Resource)]
pub struct FlyCamTuning {
	pub move_speed: f32,
	pub look_sensitivity: f32,
	pub boost_multiplier: f32,
}

impl Default for FlyCamTuning {
	fn default() -> Self {
		Self {
			move_speed: 20.0,
			look_sensitivity: 0.15,
			boost_multiplier: 3.0,
		}
	}
}

#[derive(Component, Default)]
pub struct FlyCam {
	pub yaw: f32,
	pub pitch: f32,
}

#[derive(InputAction)]
#[action_output(Vec3)]
pub struct Move3D;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Look2D;

#[derive(InputAction)]
#[action_output(bool)]
pub struct AimRmb;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Boost;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Dig;

#[derive(InputAction)]
#[action_output(bool)]
pub struct SpawnBall;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Place;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Jump;

pub struct FlyCamPlugin;

#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum FlyCamSet {
	Input,
	Move,
	Interact,
}

impl Plugin for FlyCamPlugin {
	fn build(&self, app: &mut App) {
		app
			.configure_sets(
				Update,
				(FlyCamSet::Input, FlyCamSet::Move, FlyCamSet::Interact).chain(),
			)
			.add_systems(Startup, setup)
			.add_systems(
				Update,
				(
					mouse_look
						.in_set(FlyCamSet::Input)
						.run_if(in_state(PlayerMoveState::DebugFly)),
					movement
						.in_set(FlyCamSet::Move)
						.run_if(in_state(PlayerMoveState::DebugFly)),
					interact
						.in_set(FlyCamSet::Interact)
						.run_if(in_state(PlayerMoveState::DebugFly)),
				),
			);
	}
}

pub fn setup(mut commands: Commands, desc: Res<VoxelVolumeDesc>) {
	commands.insert_resource(FlyCamTuning::default());

	// Position camera to view the center of the voxel volume
	let total_dims = Vec3::new(
		(desc.grid_dims.x * desc.chunk_core_dims.x) as f32,
		(desc.grid_dims.y * desc.chunk_core_dims.y) as f32,
		(desc.grid_dims.z * desc.chunk_core_dims.z) as f32,
	);
	let center = Vec3::new(total_dims.x * 0.5, total_dims.y * 0.5, total_dims.z * 0.5);
	let cam_pos = center + Vec3::new(0.0, 48.0, 96.0);

	commands.spawn((
		Camera3d::default(),
		Transform::from_translation(cam_pos).looking_at(center, Vec3::Y),
		FlyCam::default(),
		FlyCamCtx,
		// Bindings modeled after the bevy_enhanced_input simple fly cam
		actions!(FlyCamCtx[
				(
						Action::<Move3D>::new(),
						Bindings::spawn(Spatial::wasd_and(KeyCode::Space, KeyCode::ShiftLeft)),
				),
				(
						Action::<Look2D>::new(),
						bindings![Binding::mouse_motion()],
				),
				(
						Action::<AimRmb>::new(),
						bindings![MouseButton::Right],
				),
				(
						Action::<Boost>::new(),
						bindings![KeyCode::ControlLeft],
				),
				(
						Action::<Dig>::new(),
						bindings![KeyCode::KeyE],
				),
				(
						Action::<SpawnBall>::new(),
						bindings![KeyCode::KeyF],
				),
				(
						Action::<Place>::new(),
						bindings![KeyCode::KeyR],
				),
				(
						Action::<Jump>::new(),
						bindings![KeyCode::Space],
				),
		]),
	));
}

pub fn interact(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
	mut evw_dig: EventWriter<VoxelEditEvent>,
	q_cam: Query<(&GlobalTransform, &Actions<FlyCamCtx>)>,
	q_dig: Query<&Action<Dig>>,
	q_shoot: Query<&Action<SpawnBall>>,
	q_place: Query<&Action<Place>>,
	q_player: Query<Entity, With<crate::player::components::Player>>,
	mut spatial_query: avian::SpatialQuery,
) {
	for (xf, actions) in q_cam.iter() {
		let cam_pos = xf.translation();
		let forward = -xf.compute_transform().forward();

		// Exclude the player entity from spatial queries to avoid self-hits
		let filter = if let Ok(player_ent) = q_player.get_single() {
			avian::SpatialQueryFilter::default().with_excluded_entities([player_ent])
		} else {
			avian::SpatialQueryFilter::default()
		};

		for ent in actions.iter() {
			if let Ok(d) = q_dig.get(ent) {
				if **d {
					let dir_vec = -forward.normalize_or_zero();
					let max_t = 100.0;
					let mut hit_point = cam_pos + dir_vec * max_t;
					// Update and cast ray via Avian3D SpatialQuery; fall back to max distance if nothing hit
					spatial_query.update_pipeline();

					if let Some(hit) =
						spatial_query.cast_ray(cam_pos, Dir3::new_unchecked(dir_vec), max_t, true, &filter)
					{
						hit_point = cam_pos + dir_vec * hit.distance;
					}

					evw_dig.write(VoxelEditEvent {
						center_world: hit_point,
						radius: 1.0,
						op: EditOp::Destroy,
					});
				}
			}

			if let Ok(p) = q_place.get(ent) {
				if **p {
					let dir_vec = -forward.normalize_or_zero();
					let max_t = 100.0;
					let mut hit_point = cam_pos + dir_vec * max_t;
					spatial_query.update_pipeline();

					if let Some(hit) =
						spatial_query.cast_ray(cam_pos, Dir3::new_unchecked(dir_vec), max_t, true, &filter)
					{
						hit_point = cam_pos + dir_vec * hit.distance;
					}

					evw_dig.write(VoxelEditEvent {
						center_world: hit_point,
						radius: 2.0,
						op: EditOp::Place,
					});
				}
			}

			if let Ok(s) = q_shoot.get(ent) {
				if **s {
					let radius = 0.5;
					let start = cam_pos - forward * 2.0;
					let velocity = Vec3::ZERO;

					let mesh = meshes.add(Mesh::from(Sphere::new(radius)));
					let mat = materials.add(StandardMaterial {
						base_color: Color::srgb(0.9, 0.2, 0.2),
						..Default::default()
					});

					commands.spawn((
						avian::RigidBody::Dynamic,
						avian::Collider::sphere(radius),
						Mesh3d(mesh),
						MeshMaterial3d(mat),
						Transform::from_translation(start),
						GlobalTransform::default(),
						avian::LinearVelocity(velocity),
					));
				}
			}
		}
	}
}

pub fn mouse_look(
	mut q_cam: Query<(&mut Transform, &mut FlyCam, &Actions<FlyCamCtx>)>,
	q_look: Query<&Action<Look2D>>,
	q_aim: Query<&Action<AimRmb>>,
	cfg: Res<FlyCamTuning>,
) {
	for (mut transform, mut fly, actions) in q_cam.iter_mut() {
		let mut look = Vec2::ZERO;
		let mut aiming = false;

		for ent in actions.iter() {
			if let Ok(v) = q_look.get(ent) {
				look = **v;
			}

			if let Ok(a) = q_aim.get(ent) {
				aiming = **a;
			}
		}

		if !aiming || look == Vec2::ZERO {
			continue;
		}

		fly.yaw -= look.x * cfg.look_sensitivity * 0.01;
		fly.pitch -= look.y * cfg.look_sensitivity * 0.01;
		fly.pitch = fly.pitch.clamp(-1.54, 1.54);

		let yaw_rot = Quat::from_rotation_y(fly.yaw);
		let pitch_rot = Quat::from_rotation_x(fly.pitch);
		transform.rotation = yaw_rot * pitch_rot;
	}
}

pub fn movement(
	time: Res<Time>,
	mut q_cam: Query<(&mut Transform, &Actions<FlyCamCtx>)>,
	q_move: Query<&Action<Move3D>>,
	q_boost: Query<&Action<Boost>>,
	cfg: Res<FlyCamTuning>,
) {
	for (mut t, actions) in q_cam.iter_mut() {
		let mut move_vec = Vec3::ZERO;
		let mut boosting = false;

		for ent in actions.iter() {
			if let Ok(v) = q_move.get(ent) {
				move_vec = **v;
			}

			if let Ok(b) = q_boost.get(ent) {
				boosting = **b;
			}
		}

		if move_vec == Vec3::ZERO {
			continue;
		}

		let speed = cfg.move_speed * if boosting { cfg.boost_multiplier } else { 1.0 };
		let forward = t.forward();
		let right = t.right();
		let up = Vec3::Y;
		let world_dir =
			(forward * -move_vec.z + right * move_vec.x + up * move_vec.y).normalize_or_zero();
		t.translation += world_dir * speed * time.delta_secs();
	}
}
