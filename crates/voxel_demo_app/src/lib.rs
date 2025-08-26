use avian3d::prelude::*;
use bevy::asset::AssetPlugin;
use bevy::log::{Level, LogPlugin};
use bevy::pbr::{ExtendedMaterial, MaterialPlugin, StandardMaterial};
use bevy::prelude::*;
use bevy::window::WindowMode;
use bevy_prng::WyRand;
use bevy_rand::plugin::EntropyPlugin;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;
use std::path::{Path, PathBuf};

use bevy_voxel_plugin::plugin::{TriplanarExtension, VoxelPlugin};
mod atmosphere;
mod camera;
#[cfg(feature = "diagnostics_ui")]
mod diag;
pub mod orbit_cam;
mod player;
#[cfg(target_os = "ios")]
use bevy_ios_iap::IosIapPlugin;
#[cfg(target_os = "ios")]
mod ios_mobile;

/// C ABI entrypoint for iOS launcher
///
/// # Safety
/// This function is called from the iOS runtime and must be `extern "C"`.
/// It performs engine initialization and runs the app; it should only be called once
/// per process and assumes the process was prepared for a Bevy app run.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rust_main() {
	run();
}

/// Shared runner used by both desktop (bin main) and iOS launcher (rust_main)
pub fn run() {
	let assets_root: PathBuf = {
		let mut candidates: Vec<PathBuf> = Vec::new();
		if let Ok(env_override) = std::env::var("BEVISTER_ASSETS") {
			candidates.push(PathBuf::from(env_override));
		}
		// Workspace root when running from repo root
		candidates.push(PathBuf::from("assets"));
		// Relative to crate dir when running with crate CWD
		let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
		candidates.push(crate_dir.join("../../assets"));
		candidates.push(crate_dir.join("../assets"));
		candidates.push(crate_dir.join("assets"));
		candidates
			.into_iter()
			.find(|p| p.exists())
			.unwrap_or_else(|| PathBuf::from("assets"))
	};

	let log_filter = "wgpu=error,bevy_render=info,bevy_ecs=trace,vox=trace".to_string();

	let mut app = App::new();
	app.add_plugins({
		let mut p = DefaultPlugins
			.set(LogPlugin {
				level: Level::DEBUG,
				filter: log_filter,
				..Default::default()
			})
			.set(AssetPlugin {
				file_path: assets_root.display().to_string(),
				..Default::default()
			});
		#[cfg(not(target_os = "ios"))]
		{
			p = p.set(WindowPlugin {
				primary_window: Some(Window {
					resizable: false,
					mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
					recognize_rotation_gesture: true,
					prefers_home_indicator_hidden: true,
					prefers_status_bar_hidden: true,
					..default()
				}),
				..default()
			});
		}
		p
	});

	app
		.add_plugins((
			PhysicsPlugins::default(),
			VoxelPlugin,
			EntropyPlugin::<WyRand>::default(),
			TnuaControllerPlugin::new(FixedUpdate),
			TnuaAvian3dPlugin::new(FixedUpdate),
			#[cfg(feature = "diagnostics_ui")]
			diag::DiagPlugin,
			#[cfg(target_os = "ios")]
			IosIapPlugin::new(true),
			#[cfg(target_os = "ios")]
			ios_mobile::IosMobilePlugin,
			#[cfg(feature = "diagnostics_ui")]
			diag::onscreen::OnScreenDiagPlugin,
			MaterialPlugin::<ExtendedMaterial<StandardMaterial, TriplanarExtension>>::default(),
			// Feature plugins
			player::PlayerPlugin,
			camera::CameraPlugin,
			atmosphere::AtmospherePlugin,
			orbit_cam::OrbitCamPlugin,
		))
		.run();
}
