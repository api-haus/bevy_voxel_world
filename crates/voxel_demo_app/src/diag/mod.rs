use bevy::app::{App, Plugin};
use bevy::diagnostic::*;
use bevy::render::diagnostic;

pub mod onscreen;

pub struct DiagPlugin;

impl Plugin for DiagPlugin {
	fn build(&self, app: &mut App) {
		// we want Bevy to measure these values for us:
		app.add_plugins((
			FrameTimeDiagnosticsPlugin::default(),
			EntityCountDiagnosticsPlugin,
			SystemInformationDiagnosticsPlugin,
			diagnostic::RenderDiagnosticsPlugin,
		));
	}
}
