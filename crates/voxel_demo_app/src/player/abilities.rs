use bevy::prelude::*;
use leafwing_abilities::prelude::*;
use leafwing_input_manager::Actionlike;

// use crate::player::actions::PlayerAction;

#[derive(Abilitylike, Actionlike, Clone, Copy, Debug, Hash, PartialEq, Eq, Reflect)]
pub enum PlayerAbility {
	Punch,
}

impl PlayerAbility {
	pub fn abilities_bundle() -> AbilitiesBundle<PlayerAbility> {
		// No cooldowns/charges yet; defaults are fine
		AbilitiesBundle::<PlayerAbility> {
			cooldowns: Default::default(),
			charges: Default::default(),
		}
	}
}

// No bridging system yet; we activate punch directly in gameplay for now.
