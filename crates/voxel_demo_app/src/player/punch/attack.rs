use bevy::prelude::*;
use bevy_voxel_plugin::plugin::{EditOp, VoxelEditEvent};
use leafwing_abilities::prelude::*;
use leafwing_input_manager::prelude::*;

use super::visualize::PunchGizmo;
use crate::player::abilities::PlayerAbility;
use crate::player::components::{Player, PlayerDimensions};
use crate::player::input::PlayerInput;

/// Punch attack: destroys voxels in front of the character at chest height.
pub fn punch_attack(
	mut evw_dig: EventWriter<VoxelEditEvent>,
	mut q_player: Query<
		(
			&GlobalTransform,
			&Player,
			&PlayerDimensions,
			&mut CooldownState<PlayerAbility>,
		),
		With<Player>,
	>,
	q_input: Query<&PlayerInput, With<Player>>,
	mut commands: Commands,
) {
	let Ok((player_xf, player, dims, mut ability_cds)) = q_player.single_mut() else {
		return;
	};
	let Ok(input) = q_input.single() else {
		return;
	};

	let pressed = input.punch;

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

	if pressed {
		if ability_cds.ready(&PlayerAbility::Punch).is_ok() {
			fire(&mut evw_dig, &mut commands, pos, forward, up);
			let _ = ability_cds.trigger(&PlayerAbility::Punch);
		}
	}
}
