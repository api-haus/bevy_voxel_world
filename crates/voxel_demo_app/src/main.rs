use std::path::{Path, PathBuf};

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

#[cfg(not(target_arch = "wasm32"))]
fn main() {
	run();
}

#[cfg(target_arch = "wasm32")]
fn main() {
	use wasm_bindgen_futures::{JsFuture, spawn_local};

	spawn_local(async move {
		let threads = std::thread::available_parallelism()
			.map(|n| n.get())
			.unwrap_or(4);

		let promise = wasm_bindgen_rayon::init_thread_pool(threads);
		let _ = JsFuture::from(promise).await;

		run();
	});
}

pub fn run() {
	let assets_root: PathBuf = {
		let mut candidates: Vec<PathBuf> = Vec::new();

		if let Ok(env_override) = std::env::var("BEVISTER_ASSETS") {
			candidates.push(PathBuf::from(env_override));
		}
		candidates.push(PathBuf::from("assets"));
		let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
		candidates.push(crate_dir.join("../../assets"));
		candidates.push(crate_dir.join("../assets"));
		candidates.push(crate_dir.join("assets"));
		candidates
			.into_iter()
			.find(|p| p.exists())
			.unwrap_or_else(|| PathBuf::from("assets"))
	};

	let log_filter = {
		#[cfg(feature = "verbose_logs")]
		{
			"wgpu=error,bevy_render=info,bevy_ecs=trace,vox=trace,voxel_demo_app=debug".to_string()
		}
		#[cfg(not(feature = "verbose_logs"))]
		{
			"wgpu=error,bevy_render=warn,bevy_ecs=info,vox=info,voxel_demo_app=info".to_string()
		}
	};

	let mut app = App::new();
	app.add_plugins({
		let mut p = DefaultPlugins
			.set(LogPlugin {
				level: Level::DEBUG,
				filter: log_filter,
				..Default::default()
			})
			.set({
				#[cfg(target_arch = "wasm32")]
				{
					AssetPlugin {
						file_path: "assets".to_string(),
						..Default::default()
					}
				}
				#[cfg(not(target_arch = "wasm32"))]
				{
					AssetPlugin {
						file_path: assets_root.display().to_string(),
						..Default::default()
					}
				}
			});
		#[cfg(not(target_os = "ios"))]
		{
			#[cfg(target_arch = "wasm32")]
			{
				use bevy::window::WindowResolution;
				p = p.set(WindowPlugin {
					primary_window: Some(Window {
						title: "voxel_demo_app".to_string(),
						mode: WindowMode::Windowed,
						fit_canvas_to_parent: true,
						canvas: None,
						resolution: WindowResolution::new(1280.0, 720.0),
						resizable: true,
						..default()
					}),
					..default()
				});
			}
			#[cfg(not(target_arch = "wasm32"))]
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
			player::PlayerPlugin,
			camera::CameraPlugin,
			atmosphere::AtmospherePlugin,
			orbit_cam::OrbitCamPlugin,
		))
		.run();
}
