use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

#[derive(Resource, Clone)]
pub(crate) struct VoxelRenderMaterial {
	pub(crate) handle: Handle<ExtendedMaterial<StandardMaterial, TriplanarExtension>>,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TriplanarExtension {
	#[texture(100)]
	#[sampler(101)]
	pub(crate) albedo_map: Option<Handle<Image>>,
	#[uniform(102)]
	pub(crate) tiling_scale: f32,
}

impl Default for TriplanarExtension {
	fn default() -> Self {
		Self {
			albedo_map: None,
			tiling_scale: 0.08,
		}
	}
}

impl MaterialExtension for TriplanarExtension {
	fn fragment_shader() -> ShaderRef {
		ShaderRef::Path("shaders/triplanar_pbr.wgsl".into())
	}
	fn deferred_fragment_shader() -> ShaderRef {
		ShaderRef::Path("shaders/triplanar_pbr.wgsl".into())
	}
}

pub(crate) fn setup_voxel_material(
	mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, TriplanarExtension>>>,
	asset_server: Res<AssetServer>,
	mut commands: Commands,
) {
	let albedo: Handle<Image> =
		asset_server.load("free_stylized_textures/ground_01_1k/ground_01_color_1k.png");
	let handle = materials.add(ExtendedMaterial {
		base: StandardMaterial {
			base_color: Color::WHITE,
			base_color_texture: None,
			perceptual_roughness: 0.8,
			metallic: 0.0,
			..Default::default()
		},
		extension: TriplanarExtension {
			albedo_map: Some(albedo),
			tiling_scale: 0.08,
		},
	});
	commands.insert_resource(VoxelRenderMaterial { handle });
}
