use bevy::prelude::*;
use bevy_voxel_plugin::plugin::{EditOp, VoxelEditEvent};
use leafwing_input_manager::prelude::*;

use crate::player::actions::PlayerAction;
use crate::player::components::{Player, PlayerDimensions, PunchCooldown};

use super::visualize::PunchGizmo;

/// Punch attack: destroys voxels in front of the character at chest height.
pub fn punch_attack(
	mut evw_dig: EventWriter<VoxelEditEvent>,
	mut q_player: Query<
		(
			&GlobalTransform,
			&Player,
			&PlayerDimensions,
			&mut PunchCooldown,
		),
		With<Player>,
	>,
	q_actions: Query<&ActionState<PlayerAction>, With<Player>>,
	mut commands: Commands,
	time: Res<Time>,
) {
	let Ok((player_xf, player, dims, mut cooldown)) = q_player.single_mut() else {
		return;
	};
	let Ok(actions) = q_actions.single() else {
		return;
	};

	// Tick cooldown each frame
	cooldown.0.tick(time.delta());

	let pressed = actions.pressed(&PlayerAction::Punch);
	let just = actions.just_pressed(&PlayerAction::Punch);

	let fire = |evw: &mut EventWriter<VoxelEditEvent>,
	            cmds: &mut Commands,
	            pos: Vec3,
	            forward: Vec3,
	            up: Vec3| {
		let r = dims.radius * 4.0;
		let center = pos + forward * r + up * r * 0.2;
		evw.write(VoxelEditEvent {
			center_world: center,
			radius: r,
			op: EditOp::Destroy,
		});
		let chest = pos + Vec3::Y * (dims.height * 0.5 - dims.radius * 0.2);
		cmds.spawn(PunchGizmo::with_sphere(
			chest,
			center,
			center,
			r,
			Color::srgb(1.0, 0.5, 0.1),
		));
	};

	let up = Vec3::Y;
	let pos = player_xf.translation();
	let forward = player.facing.normalize_or_zero();

	if just || (pressed && cooldown.0.finished()) {
		fire(&mut evw_dig, &mut commands, pos, forward, up);
		cooldown.0.reset();
	}
}
