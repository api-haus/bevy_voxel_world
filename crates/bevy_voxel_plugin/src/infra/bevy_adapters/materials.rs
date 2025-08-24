//! Material handling for voxel rendering

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

/// Resource holding the voxel material
#[derive(Resource)]
pub struct VoxelMaterialResource {
	pub handle: Handle<ExtendedMaterial<StandardMaterial, TriplanarMaterialExtension>>,
}

/// Triplanar material extension for voxel rendering
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TriplanarMaterialExtension {
	#[texture(100, dimension = "2d_array")]
	#[sampler(101)]
	pub albedo_array: Handle<Image>,
	#[uniform(102)]
	pub tiling_scale: f32,
	#[uniform(103)]
	pub layer_count: u32,
}

impl Default for TriplanarMaterialExtension {
	fn default() -> Self {
		Self {
			albedo_array: Default::default(),
			tiling_scale: 0.08,
			layer_count: 1,
		}
	}
}

impl MaterialExtension for TriplanarMaterialExtension {
	fn fragment_shader() -> ShaderRef {
		ShaderRef::Path("shaders/triplanar_pbr.wgsl".into())
	}
}

/// System to initialize voxel materials
pub fn init_voxel_materials_system(
	mut commands: Commands,
	mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, TriplanarMaterialExtension>>>,
	asset_server: Res<AssetServer>,
	existing: Option<Res<VoxelMaterialResource>>,
) {
	if existing.is_some() {
		return;
	}

	let texture = asset_server.load("generated/albedo_array_stacked.png");

	let material = ExtendedMaterial {
		base: StandardMaterial {
			base_color: Color::WHITE,
			perceptual_roughness: 0.8,
			metallic: 0.0,
			..default()
		},
		extension: TriplanarMaterialExtension {
			albedo_array: texture,
			tiling_scale: 0.08,
			layer_count: 16, // Will be updated when texture loads
		},
	};

	let handle = materials.add(material);
	commands.insert_resource(VoxelMaterialResource { handle });
}
