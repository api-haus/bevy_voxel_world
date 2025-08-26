use bevy::prelude::*;

use leafwing_abilities::prelude::*;

use leafwing_input_manager::Actionlike;

// use crate::player::actions::PlayerAction;

#[derive(Abilitylike, Actionlike, Clone, Copy, Debug, Hash, PartialEq, Eq, Reflect)]
pub enum PlayerAbility {
	Punch,
}

impl PlayerAbility {
	pub fn cooldowns() -> CooldownState<PlayerAbility> {
		let mut cooldowns = CooldownState::default();
		cooldowns.set(PlayerAbility::Punch, Cooldown::from_secs(0.25));
		cooldowns
	}

	pub fn abilities_bundle() -> AbilitiesBundle<PlayerAbility> {
		AbilitiesBundle::<PlayerAbility> {
			cooldowns: Self::cooldowns(),
			charges: Default::default(),
		}
	}
}

// Bridging: punches are gated and triggered via `leafwing_abilities`.
