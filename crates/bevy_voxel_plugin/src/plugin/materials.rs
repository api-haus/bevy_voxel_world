use bevy::pbr::{
	ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
	AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
};
use tracing::{debug, info, warn};

#[derive(Resource, Clone)]
pub(crate) struct VoxelRenderMaterial {
	pub(crate) handle: Handle<ExtendedMaterial<StandardMaterial, TriplanarExtension>>,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(TriplanarExtensionKey)]
pub struct TriplanarExtension {
	#[texture(100, dimension = "2d_array")]
	#[sampler(101)]
	pub(crate) albedo_array: Handle<Image>,
	#[uniform(102)]
	pub(crate) tiling_scale: f32,
	#[uniform(103)]
	pub(crate) albedo_layer_count: u32,
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
			tiling_scale: 0.08,
			albedo_layer_count: 1,
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

pub(crate) fn init_voxel_material_when_ready(
	mut commands: Commands,
	mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, TriplanarExtension>>>,
	mut images: ResMut<Assets<Image>>,
	asset_server: Res<AssetServer>,
	maybe_existing: Option<Res<VoxelRenderMaterial>>,
) {
	if maybe_existing.is_some() {
		return;
	}
	let stacked: Handle<Image> = asset_server.load("generated/albedo_array_stacked.png");
	if let Some(img) = images.get_mut(&stacked) {
		debug!(target: "vox", "voxel_mat_image_ready size=({}x{}), format={:?}", img.texture_descriptor.size.width, img.texture_descriptor.size.height, img.texture_descriptor.format);
		let width = img.texture_descriptor.size.width.max(1);
		let height = img.texture_descriptor.size.height;
		let layers = (height / width).max(1);
		img.reinterpret_stacked_2d_as_array(layers);
		let handle = materials.add(ExtendedMaterial {
			base: StandardMaterial {
				base_color: Color::WHITE,
				base_color_texture: None,
				perceptual_roughness: 0.8,
				metallic: 0.0,
				..Default::default()
			},
			extension: TriplanarExtension {
				albedo_array: stacked.clone(),
				tiling_scale: 0.08,
				albedo_layer_count: layers,
				debug_mat_vis: true,
			},
		});
		commands.insert_resource(VoxelRenderMaterial { handle });
		info!(target: "vox", "voxel_mat_created layers={}", layers);
	} else {
		warn!(target: "vox", "voxel_mat_image_not_ready (asset not yet loaded)");
	}
}
