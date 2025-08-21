use bevy::pbr::{
	ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
	AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
};

#[derive(Resource, Clone)]
pub(crate) struct VoxelRenderMaterial {
	pub(crate) handle: Handle<ExtendedMaterial<StandardMaterial, TriplanarExtension>>,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(TriplanarExtensionKey)]
pub struct TriplanarExtension {
	#[texture(100)]
	#[sampler(101)]
	pub(crate) albedo_map: Option<Handle<Image>>,
	#[uniform(102)]
	pub(crate) tiling_scale: f32,
	pub(crate) debug_mat_vis: bool,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct TriplanarExtensionKey {
	pub debug_mat_vis: bool,
}

impl From<&TriplanarExtension> for TriplanarExtensionKey {
	fn from(ext: &TriplanarExtension) -> Self {
		Self {
			debug_mat_vis: ext.debug_mat_vis,
		}
	}
}

impl Default for TriplanarExtension {
	fn default() -> Self {
		Self {
			albedo_map: None,
			tiling_scale: 0.08,
			debug_mat_vis: false,
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

	fn specialize(
		_pipeline: &MaterialExtensionPipeline,
		descriptor: &mut RenderPipelineDescriptor,
		_layout: &MeshVertexBufferLayoutRef,
		key: MaterialExtensionKey<Self>,
	) -> Result<(), SpecializedMeshPipelineError> {
		if let Some(fragment) = descriptor.fragment.as_mut() {
			if key.bind_group_data.debug_mat_vis {
				fragment.shader_defs.push("DEBUG_MAT_VIS".into());
			}
		}
		Ok(())
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
			debug_mat_vis: false,
		},
	});
	commands.insert_resource(VoxelRenderMaterial { handle });
}
