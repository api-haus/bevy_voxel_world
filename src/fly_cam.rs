use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

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

pub fn setup(mut commands: Commands) {
    commands.insert_resource(FlyCamTuning::default());

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 24.0, 72.0).looking_at(Vec3::ZERO, Vec3::Y),
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
        ]),
    ));

    // Simple light
    commands.spawn((
        DirectionalLight {
            illuminance: 4000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::default().looking_at(-Vec3::Y, Vec3::Z),
    ));
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
