#[cfg(feature = "diagnostics_ui")]
use bevy::app::{App, Plugin};
#[cfg(feature = "diagnostics_ui")]
use bevy::prelude::{Commands, Startup};
#[cfg(feature = "diagnostics_ui")]
use iyes_perf_ui::prelude::{
	PerfUiEntryCpuUsage, PerfUiEntryEntityCount, PerfUiEntryFPS, PerfUiEntryFPSWorst,
	PerfUiEntryFrameCount, PerfUiEntryFrameTime, PerfUiEntryFrameTimeWorst, PerfUiEntryMemUsage,
	PerfUiEntryRenderCpuTime, PerfUiEntryRenderGpuTime, PerfUiWidgetBar,
};
#[cfg(feature = "diagnostics_ui")]
use iyes_perf_ui::PerfUiPlugin;

#[cfg(feature = "diagnostics_ui")]
pub struct OnScreenDiagPlugin;

#[cfg(feature = "diagnostics_ui")]
impl Plugin for OnScreenDiagPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(PerfUiPlugin);
		app.add_systems(Startup, setup);
	}
}

#[cfg(feature = "diagnostics_ui")]
fn setup(mut commands: Commands) {
	// Customize your Perf UI by explicitly listing the entries you want.
	commands.spawn((
		// when we have lots of entries, we have to group them
		// into tuples, because of Bevy Rust syntax limitations
		(
			PerfUiWidgetBar::new(PerfUiEntryFPS::default()),
			PerfUiWidgetBar::new(PerfUiEntryFPSWorst::default()),
			PerfUiWidgetBar::new(PerfUiEntryFrameTime::default()),
			PerfUiWidgetBar::new(PerfUiEntryFrameTimeWorst::default()),
			PerfUiWidgetBar::new(PerfUiEntryRenderCpuTime::default()),
			PerfUiWidgetBar::new(PerfUiEntryRenderGpuTime::default()),
			PerfUiWidgetBar::new(PerfUiEntryEntityCount::default()),
			PerfUiWidgetBar::new(PerfUiEntryCpuUsage::default()),
			PerfUiWidgetBar::new(PerfUiEntryMemUsage::default()),
			PerfUiEntryFrameCount::default(),
		),
	));
}
