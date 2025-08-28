use bevy::pbr::{
	ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
	AsBindGroup, RenderPipelineDescriptor, ShaderRef, ShaderType, SpecializedMeshPipelineError,
};
use tracing::{debug, info};

#[derive(Resource)]
pub(crate) struct LoadingTexture {
	pub is_loaded: bool,
	pub handle: Handle<Image>,
}

#[derive(Resource, Clone)]
pub(crate) struct VoxelRenderMaterial {
	pub(crate) handle: Handle<ExtendedMaterial<StandardMaterial, TriplanarExtension>>,
}

#[derive(ShaderType, Reflect, Debug, Clone, Copy)]
pub struct TriplanarParams {
	pub tiling_scale: f32,
	pub albedo_layer_count: u32,
	pub _pad: Vec2,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(TriplanarExtensionKey)]
pub struct TriplanarExtension {
	#[texture(100, dimension = "2d_array")]
	#[sampler(101)]
	pub(crate) albedo_array: Handle<Image>,
	#[uniform(102)]
	pub(crate) triplanar: TriplanarParams,
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
			albedo_array: Default::default(),
			triplanar: TriplanarParams {
				tiling_scale: 0.5,
				albedo_layer_count: 1,
				_pad: Vec2::ZERO,
			},
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
		if let Some(fragment) = descriptor.fragment.as_mut()
			&& key.bind_group_data.debug_mat_vis
		{
			fragment.shader_defs.push("DEBUG_MAT_VIS".into());
		}

		Ok(())
	}
}

pub(crate) fn init_voxel_material_when_ready(
	mut commands: Commands,
	mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, TriplanarExtension>>>,
	mut images: ResMut<Assets<Image>>,
	asset_server: Res<AssetServer>,
	mut loading_texture: ResMut<LoadingTexture>,
	maybe_existing: Option<Res<VoxelRenderMaterial>>,
) {
	if maybe_existing.is_some() {
		// info!(target: "vox", "init_voxel_material_when_ready: material already
		// exists");
		return;
	}

	let load_state = asset_server.load_state(loading_texture.handle.id());

	if loading_texture.is_loaded {
		return;
	}

	if !load_state.is_loaded() {
		debug!(target: "vox", "voxel_mat_waiting texture not loaded yet, state={:?}", load_state);
		return;
	}

	loading_texture.is_loaded = true;

	let image = images.get_mut(&loading_texture.handle).unwrap();
	debug!(target: "vox", "voxel_mat_image_ready size=({}x{}), format={:?}", image.texture_descriptor.size.width, image.texture_descriptor.size.height, image.texture_descriptor.format);
	let width = image.texture_descriptor.size.width.max(1);
	let height = image.texture_descriptor.size.height;
	let layers = (height / width).max(1);
	image.reinterpret_stacked_2d_as_array(layers);
	debug!(target: "vox", "voxel_mat_image_reinterpreted as array layers={}", layers);

	let handle = materials.add(ExtendedMaterial {
		base: StandardMaterial {
			base_color: Color::WHITE,
			base_color_texture: None,
			perceptual_roughness: 0.8,
			metallic: 0.0,
			..Default::default()
		},
		extension: TriplanarExtension {
			albedo_array: loading_texture.handle.clone(),
			triplanar: TriplanarParams {
				tiling_scale: 0.5,
				albedo_layer_count: layers as u32,
				_pad: Vec2::ZERO,
			},
			..Default::default()
		},
	});

	commands.insert_resource(VoxelRenderMaterial { handle });
	info!(target: "vox", "voxel_mat_created layers={}", layers);
}

pub(crate) fn init_texture_loading(mut commands: Commands, asset_server: Res<AssetServer>) {
	let texture_path = "generated/albedo_array_stacked.png";
	info!(target: "vox", "init_texture_loading: loading texture from {}", texture_path);
	let handle = asset_server.load(texture_path);
	commands.insert_resource(LoadingTexture {
		is_loaded: false,
		handle,
	});
}
