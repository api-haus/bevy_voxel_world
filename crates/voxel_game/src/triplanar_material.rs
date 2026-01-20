//! Triplanar PBR material for voxel terrain.
//!
//! Uses ExtendedMaterial to extend StandardMaterial with triplanar texture arrays.
//! Material blend weights are passed via vertex color (RGBA = 4 layer weights).
//!
//! Channel packing:
//! - Diffuse array (4 layers): RGB=Diffuse, A=Height
//! - Normal array (4 layers): RGB=Normal, A=unused
//! - Mask array (4 layers): R=Roughness, G=Metallic, B=AO

use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};

/// Path to the embedded triplanar extension shader.
/// Uses embedded_asset! to avoid WASM .meta file issues.
const TRIPLANAR_SHADER_PATH: &str = "embedded://voxel_game/shaders/triplanar_ext.wgsl";

/// Number of texture layers in each array.
pub const NUM_LAYERS: u32 = 4;

/// Triplanar extension for StandardMaterial.
///
/// Uses bindings 100+ to avoid conflicts with StandardMaterial bindings.
/// Material blend weights are read from vertex colors (RGBA = 4 layer weights).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TriplanarExtension {
    /// Diffuse/albedo texture array (4 layers): RGB=Diffuse, A=Height.
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub diffuse_array: Handle<Image>,

    /// Normal map texture array (4 layers): RGB=Normal.
    #[texture(102, dimension = "2d_array")]
    #[sampler(103)]
    pub normal_array: Handle<Image>,

    /// Material mask texture array (4 layers): R=Roughness, G=Metallic, B=AO.
    #[texture(104, dimension = "2d_array")]
    #[sampler(105)]
    pub mask_array: Handle<Image>,

    /// Triplanar mapping parameters.
    #[uniform(106)]
    pub params: TriplanarParams,
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

impl MaterialExtension for TriplanarExtension {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        TRIPLANAR_SHADER_PATH.into()
    }
}

/// Type alias for the complete triplanar material.
pub type TriplanarMaterial = ExtendedMaterial<StandardMaterial, TriplanarExtension>;

/// Plugin to register the triplanar material.
pub struct TriplanarMaterialPlugin;

impl Plugin for TriplanarMaterialPlugin {
    fn build(&self, app: &mut App) {
        // Embed the shader to avoid WASM .meta file issues
        embedded_asset!(app, "shaders/triplanar_ext.wgsl");
        app.add_plugins(MaterialPlugin::<TriplanarMaterial>::default());
    }
}

// =============================================================================
// Material Resources
// =============================================================================

/// Resource containing LOD-colored materials for visualization.
#[derive(Resource)]
pub struct LodMaterials {
    pub materials: Vec<Handle<StandardMaterial>>,
    pub neutral: Handle<StandardMaterial>,
}

impl LodMaterials {
    /// Get material for a given LOD level.
    pub fn get(&self, lod: i32, use_lod_colors: bool) -> Handle<StandardMaterial> {
        if use_lod_colors {
            let idx = (lod as usize).min(self.materials.len() - 1);
            self.materials[idx].clone()
        } else {
            self.neutral.clone()
        }
    }
}

/// Resource containing the triplanar terrain material.
#[derive(Resource)]
pub struct TerrainMaterial {
    pub handle: Handle<TriplanarMaterial>,
}

/// Helper to create a triplanar extension with textures.
pub fn create_triplanar_extension(
    diffuse_array: Handle<Image>,
    normal_array: Handle<Image>,
    mask_array: Handle<Image>,
) -> TriplanarExtension {
    TriplanarExtension {
        diffuse_array,
        normal_array,
        mask_array,
        params: TriplanarParams::default(),
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

    ExtendedMaterial {
        base: StandardMaterial {
            // Enable vertex colors for material blend weights
            ..default()
        },
        extension: TriplanarExtension {
            diffuse_array,
            normal_array,
            mask_array,
            params: TriplanarParams::default(),
        },
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

    materials.add(ExtendedMaterial {
        base: StandardMaterial {
            ..default()
        },
        extension: TriplanarExtension {
            diffuse_array,
            normal_array,
            mask_array,
            params: TriplanarParams::default(),
        },
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
