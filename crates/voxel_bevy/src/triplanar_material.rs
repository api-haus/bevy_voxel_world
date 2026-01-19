//! Triplanar PBR material for voxel terrain.
//!
//! Manual AsBindGroup implementation to bypass Bevy's bindless mode.
//! Uses triplanar texture sampling with 4-layer blending.
//! Material blend weights are passed via vertex color (RGBA = 4 layer weights).
//!
//! Channel packing:
//! - Diffuse array (4 layers): RGB=Diffuse, A=Height
//! - Normal array (4 layers): RGB=Normal, A=unused
//! - Mask array (4 layers): R=Roughness, G=Metallic, B=AO

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::{lifetimeless::SRes, SystemParamItem};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    binding_types::{sampler, texture_2d_array, uniform_buffer},
    AsBindGroupError, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindGroupLayoutEntries,
    BindingResources, BindingResource, PipelineCache, PreparedBindGroup, SamplerBindingType,
    ShaderStages, ShaderType, TextureSampleType, UnpreparedBindGroup,
    encase::StorageBuffer, BufferInitDescriptor, BufferUsages,
};
use bevy::render::render_resource::BindGroupLayoutDescriptor;
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::{FallbackImage, GpuImage};
use bevy::shader::ShaderRef;

/// Number of texture layers in each array.
pub const NUM_LAYERS: u32 = 4;

/// Triplanar material for voxel terrain.
///
/// Provides texture arrays and triplanar sampling parameters with custom PBR lighting.
/// Material blend weights are read from vertex colors (RGBA = 4 layer weights).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct TriplanarMaterial {
    /// Diffuse/albedo texture array (4 layers): RGB=Diffuse, A=Height.
    pub diffuse_array: Handle<Image>,

    /// Normal map texture array (4 layers): RGB=Normal.
    pub normal_array: Handle<Image>,

    /// Material mask texture array (4 layers): R=Roughness, G=Metallic, B=AO.
    pub mask_array: Handle<Image>,

    /// Triplanar mapping parameters.
    pub params: TriplanarParams,
}

/// Key for material pipeline specialization.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct TriplanarMaterialKey {}

impl From<&TriplanarMaterial> for TriplanarMaterialKey {
    fn from(_material: &TriplanarMaterial) -> Self {
        Self {}
    }
}

/// Shader parameters for triplanar mapping.
#[derive(ShaderType, Debug, Clone, Copy)]
pub struct TriplanarParams {
    /// Texture scale (world units per texture repeat).
    pub texture_scale: f32,
    /// Blend sharpness for triplanar projection (higher = sharper transitions).
    pub blend_sharpness: f32,
    /// Normal map strength multiplier.
    pub normal_strength: f32,
    /// Padding for 16-byte alignment.
    pub _padding: f32,
}

impl Default for TriplanarParams {
    fn default() -> Self {
        Self {
            texture_scale: 0.1,
            blend_sharpness: 4.0,
            normal_strength: 1.0,
            _padding: 0.0,
        }
    }
}

/// Manual AsBindGroup implementation to bypass bindless mode.
/// texture_2d_array is not compatible with Bevy's bindless system.
impl bevy::render::render_resource::AsBindGroup for TriplanarMaterial {
    type Data = TriplanarMaterialKey;
    type Param = (SRes<RenderAssets<GpuImage>>, SRes<FallbackImage>);

    fn label() -> &'static str {
        "triplanar_material_bind_group"
    }

    fn as_bind_group(
        &self,
        layout_descriptor: &BindGroupLayoutDescriptor,
        render_device: &RenderDevice,
        pipeline_cache: &PipelineCache,
        (image_assets, _fallback_image): &mut SystemParamItem<'_, '_, Self::Param>,
    ) -> Result<PreparedBindGroup, AsBindGroupError> {
        // Get GPU images or return retry
        let diffuse_gpu = image_assets
            .get(&self.diffuse_array)
            .ok_or(AsBindGroupError::RetryNextUpdate)?;
        let normal_gpu = image_assets
            .get(&self.normal_array)
            .ok_or(AsBindGroupError::RetryNextUpdate)?;
        let mask_gpu = image_assets
            .get(&self.mask_array)
            .ok_or(AsBindGroupError::RetryNextUpdate)?;

        // Create uniform buffer for params
        let mut buffer = StorageBuffer::new(Vec::new());
        buffer.write(&self.params).unwrap();
        let params_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("triplanar_params_buffer"),
            contents: buffer.as_ref(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let layout = &pipeline_cache.get_bind_group_layout(layout_descriptor);

        // Create bind group with all resources
        let bind_group = render_device.create_bind_group(
            Self::label(),
            layout,
            &[
                // Binding 0: diffuse texture array
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&diffuse_gpu.texture_view),
                },
                // Binding 1: diffuse sampler
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&diffuse_gpu.sampler),
                },
                // Binding 2: normal texture array
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&normal_gpu.texture_view),
                },
                // Binding 3: normal sampler
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Sampler(&normal_gpu.sampler),
                },
                // Binding 4: mask texture array
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&mask_gpu.texture_view),
                },
                // Binding 5: mask sampler
                BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::Sampler(&mask_gpu.sampler),
                },
                // Binding 6: params uniform
                BindGroupEntry {
                    binding: 6,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        );

        Ok(PreparedBindGroup {
            bindings: BindingResources(vec![]),
            bind_group,
        })
    }

    fn bind_group_data(&self) -> Self::Data {
        TriplanarMaterialKey::from(self)
    }

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        _render_device: &RenderDevice,
        _param: &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        // Signal that we create the bind group directly via as_bind_group()
        // This bypasses bindless mode which doesn't support texture_2d_array
        Err(AsBindGroupError::CreateBindGroupDirectly)
    }

    fn bind_group_layout_entries(
        _render_device: &RenderDevice,
        _force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry>
    where
        Self: Sized,
    {
        BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                // Binding 0: diffuse texture array
                (0, texture_2d_array(TextureSampleType::Float { filterable: true })),
                // Binding 1: diffuse sampler
                (1, sampler(SamplerBindingType::Filtering)),
                // Binding 2: normal texture array
                (2, texture_2d_array(TextureSampleType::Float { filterable: true })),
                // Binding 3: normal sampler
                (3, sampler(SamplerBindingType::Filtering)),
                // Binding 4: mask texture array
                (4, texture_2d_array(TextureSampleType::Float { filterable: true })),
                // Binding 5: mask sampler
                (5, sampler(SamplerBindingType::Filtering)),
                // Binding 6: params uniform
                (6, uniform_buffer::<TriplanarParams>(false)),
            ),
        )
        .to_vec()
    }
}

impl Material for TriplanarMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/triplanar.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/triplanar.wgsl".into()
    }
}

/// Plugin to register the triplanar material.
pub struct TriplanarMaterialPlugin;

impl Plugin for TriplanarMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TriplanarMaterial>::default());
    }
}

/// Layer colors for placeholder diffuse textures.
pub const LAYER_COLORS: [[u8; 4]; 4] = [
    [120, 100, 80, 255],  // Layer 0: Brown (dirt)
    [100, 140, 80, 255],  // Layer 1: Green (grass)
    [140, 140, 140, 255], // Layer 2: Gray (stone)
    [200, 200, 210, 255], // Layer 3: Light gray (snow/sand)
];

/// Create a 2D array texture with 4 layers.
///
/// Each layer has a checkerboard pattern with the specified tint color.
fn create_texture_array(
    size: u32,
    layers: &[[u8; 4]; 4],
    format: bevy::render::render_resource::TextureFormat,
) -> Image {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureViewDescriptor, TextureViewDimension};

    let mut data = Vec::with_capacity((size * size * 4) as usize * NUM_LAYERS as usize);

    let checker_size = size / 4;

    for layer_color in layers {
        for y in 0..size {
            for x in 0..size {
                let checker_x = (x / checker_size.max(1)) % 2;
                let checker_y = (y / checker_size.max(1)) % 2;
                let is_light = (checker_x + checker_y) % 2 == 0;

                if is_light {
                    data.push(layer_color[0].saturating_add(40));
                    data.push(layer_color[1].saturating_add(40));
                    data.push(layer_color[2].saturating_add(40));
                    data.push(layer_color[3]);
                } else {
                    data.push(layer_color[0].saturating_sub(40));
                    data.push(layer_color[1].saturating_sub(40));
                    data.push(layer_color[2].saturating_sub(40));
                    data.push(layer_color[3]);
                }
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: NUM_LAYERS,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );

    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });

    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    });

    image
}

/// Create a neutral normal map array (all layers pointing up).
fn create_neutral_normal_array(size: u32) -> Image {
    use bevy::render::render_resource::TextureFormat;
    let neutral_normal = [128u8, 128, 255, 255];
    let layers = [neutral_normal, neutral_normal, neutral_normal, neutral_normal];
    create_texture_array(size, &layers, TextureFormat::Rgba8Unorm)
}

/// Create a default mask array (medium roughness, no metallic, full AO).
fn create_default_mask_array(size: u32) -> Image {
    use bevy::render::render_resource::TextureFormat;
    let default_mask = [178u8, 0, 255, 255]; // R=roughness, G=metallic, B=AO
    let layers = [default_mask, default_mask, default_mask, default_mask];
    create_texture_array(size, &layers, TextureFormat::Rgba8Unorm)
}

/// Create a triplanar material with placeholder texture arrays.
pub fn create_placeholder_material(images: &mut Assets<Image>) -> TriplanarMaterial {
    let texture_size = 64;

    use bevy::render::render_resource::TextureFormat;
    let diffuse_array = images.add(create_texture_array(texture_size, &LAYER_COLORS, TextureFormat::Rgba8UnormSrgb));
    let normal_array = images.add(create_neutral_normal_array(texture_size));
    let mask_array = images.add(create_default_mask_array(texture_size));

    TriplanarMaterial {
        diffuse_array,
        normal_array,
        mask_array,
        params: TriplanarParams::default(),
    }
}

/// Load baked terrain texture arrays from KTX2 files.
pub fn load_baked_terrain_material(
    asset_server: &AssetServer,
    materials: &mut Assets<TriplanarMaterial>,
) -> Handle<TriplanarMaterial> {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};

    let sampler = ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    };

    let settings = move |s: &mut ImageLoaderSettings| {
        s.sampler = ImageSampler::Descriptor(sampler.clone());
    };

    let diffuse_array = asset_server.load_with_settings("textures/terrain/diffuse_height.ktx2", settings.clone());
    let normal_array = asset_server.load_with_settings("textures/terrain/normal.ktx2", settings.clone());
    let mask_array = asset_server.load_with_settings("textures/terrain/material.ktx2", settings);

    materials.add(TriplanarMaterial {
        diffuse_array,
        normal_array,
        mask_array,
        params: TriplanarParams::default(),
    })
}

/// Configuration for loading terrain textures from files.
pub struct TerrainTextureConfig {
    pub base_path: String,
    pub diffuse_files: [String; 4],
    pub normal_files: [String; 4],
    pub mask_files: [String; 4],
    pub texture_scale: f32,
}

impl Default for TerrainTextureConfig {
    fn default() -> Self {
        Self {
            base_path: "textures/terrain/".to_string(),
            diffuse_files: [
                "diffuse_height_0.png".to_string(),
                "diffuse_height_1.png".to_string(),
                "diffuse_height_2.png".to_string(),
                "diffuse_height_3.png".to_string(),
            ],
            normal_files: [
                "normal_0.png".to_string(),
                "normal_1.png".to_string(),
                "normal_2.png".to_string(),
                "normal_3.png".to_string(),
            ],
            mask_files: [
                "material_0.png".to_string(),
                "material_1.png".to_string(),
                "material_2.png".to_string(),
                "material_3.png".to_string(),
            ],
            texture_scale: 0.1,
        }
    }
}
