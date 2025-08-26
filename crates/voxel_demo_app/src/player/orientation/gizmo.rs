use bevy::prelude::*;

use crate::player::components::Player;

/// Visualize the player's forward orientation (chest-height) using a gizmo arrow.
pub fn visualize_player_orientation(
	mut gizmos: Gizmos,
	q_player: Query<(&GlobalTransform, &Player), With<Player>>,
) {
	let Ok((player_xf, player)) = q_player.get_single() else {
		return;
	};
	let p = player_xf.translation();
	let player_forward = player.facing.normalize_or_zero();
	let origin = p + Vec3::Y * 0.6;
	let tip = origin + player_forward * 1.2;
	gizmos.arrow(origin, tip, Color::srgb(1.0, 0.5, 0.0));
}
