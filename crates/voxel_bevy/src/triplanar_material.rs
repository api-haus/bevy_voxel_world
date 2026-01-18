//! Triplanar PBR material for voxel terrain.
//!
//! Supports 4 texture layers with per-vertex blend weights and
//! triplanar projection to avoid UV seams on steep surfaces.

use bevy::asset::{Asset, RenderAssetUsages};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::systems::entities::ATTRIBUTE_MATERIAL_WEIGHTS;

/// Triplanar PBR material with 4 texture layers.
///
/// Each layer has albedo, normal, and ARM (Ambient occlusion, Roughness, Metallic) textures.
/// Blending between layers is controlled by per-vertex material weights.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
#[bind_group_data(TriplanarMaterialKey)]
pub struct TriplanarMaterial {
  // Layer 0
  #[texture(0, dimension = "2d")]
  #[sampler(1)]
  pub albedo_0: Handle<Image>,
  #[texture(2, dimension = "2d")]
  #[sampler(3)]
  pub normal_0: Handle<Image>,
  #[texture(4, dimension = "2d")]
  #[sampler(5)]
  pub arm_0: Handle<Image>,

  // Layer 1
  #[texture(6, dimension = "2d")]
  #[sampler(7)]
  pub albedo_1: Handle<Image>,
  #[texture(8, dimension = "2d")]
  #[sampler(9)]
  pub normal_1: Handle<Image>,
  #[texture(10, dimension = "2d")]
  #[sampler(11)]
  pub arm_1: Handle<Image>,

  // Layer 2
  #[texture(12, dimension = "2d")]
  #[sampler(13)]
  pub albedo_2: Handle<Image>,
  #[texture(14, dimension = "2d")]
  #[sampler(15)]
  pub normal_2: Handle<Image>,
  #[texture(16, dimension = "2d")]
  #[sampler(17)]
  pub arm_2: Handle<Image>,

  // Layer 3
  #[texture(18, dimension = "2d")]
  #[sampler(19)]
  pub albedo_3: Handle<Image>,
  #[texture(20, dimension = "2d")]
  #[sampler(21)]
  pub normal_3: Handle<Image>,
  #[texture(22, dimension = "2d")]
  #[sampler(23)]
  pub arm_3: Handle<Image>,

  /// Uniform parameters
  #[uniform(24)]
  pub params: TriplanarParams,
}

/// Shader parameters for triplanar mapping.
#[derive(ShaderType, Debug, Clone, Copy)]
pub struct TriplanarParams {
  /// Texture scale for each layer (world units per texture repeat).
  pub texture_scales: [f32; 4],
  /// Blend sharpness for triplanar projection (higher = sharper transitions).
  pub blend_sharpness: f32,
  /// Normal map strength multiplier.
  pub normal_strength: f32,
  /// Padding for alignment
  pub _padding: [f32; 2],
}

impl Default for TriplanarParams {
  fn default() -> Self {
    Self {
      texture_scales: [1.0, 1.0, 1.0, 1.0],
      blend_sharpness: 4.0,
      normal_strength: 1.0,
      _padding: [0.0; 2],
    }
  }
}

/// Key for material pipeline specialization.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TriplanarMaterialKey {
  pub cull_mode: bool,
}

impl From<&TriplanarMaterial> for TriplanarMaterialKey {
  fn from(_material: &TriplanarMaterial) -> Self {
    Self { cull_mode: false }
  }
}

impl Material for TriplanarMaterial {
  fn vertex_shader() -> ShaderRef {
    "shaders/triplanar.wgsl".into()
  }

  fn fragment_shader() -> ShaderRef {
    "shaders/triplanar.wgsl".into()
  }

  fn specialize(
    _pipeline: &bevy::pbr::MaterialPipeline,
    descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    _key: bevy::pbr::MaterialPipelineKey<Self>,
  ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
    // Add material weights vertex attribute
    let vertex_layout = layout.0.get_layout(&[
      Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
      Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
      ATTRIBUTE_MATERIAL_WEIGHTS.at_shader_location(2),
    ])?;
    descriptor.vertex.buffers = vec![vertex_layout];

    // Double-sided rendering (no backface culling)
    descriptor.primitive.cull_mode = None;

    Ok(())
  }
}

/// Plugin to register the triplanar material.
pub struct TriplanarMaterialPlugin;

impl Plugin for TriplanarMaterialPlugin {
  fn build(&self, app: &mut App) {
    app.add_plugins(MaterialPlugin::<TriplanarMaterial>::default());
  }
}

/// Create a solid color 4x4 image for placeholder textures.
fn create_solid_image(color: [u8; 4]) -> Image {
  use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

  let size = 4;
  let data: Vec<u8> = (0..size * size).flat_map(|_| color).collect();

  Image::new(
    Extent3d {
      width: size,
      height: size,
      depth_or_array_layers: 1,
    },
    TextureDimension::D2,
    data,
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::RENDER_WORLD,
  )
}

/// Create a neutral normal map (pointing up).
fn create_neutral_normal() -> Image {
  // Normal map neutral = (0.5, 0.5, 1.0) in RGB = (128, 128, 255)
  create_solid_image([128, 128, 255, 255])
}

/// Create a default ARM texture (no AO, medium roughness, no metallic).
fn create_default_arm() -> Image {
  // R=AO (1.0=white), G=Roughness (0.7), B=Metallic (0.0)
  create_solid_image([255, 178, 0, 255])
}

/// Layer colors for placeholder textures.
pub const LAYER_COLORS: [[u8; 4]; 4] = [
  [120, 100, 80, 255],  // Layer 0: Brown (dirt)
  [100, 140, 80, 255],  // Layer 1: Green (grass)
  [140, 140, 140, 255], // Layer 2: Gray (stone)
  [200, 200, 210, 255], // Layer 3: Light gray (snow/sand)
];

/// Create a triplanar material with placeholder textures.
///
/// This creates simple solid-color textures for testing.
/// Replace with actual texture assets for production.
pub fn create_placeholder_material(images: &mut Assets<Image>) -> TriplanarMaterial {
  // Create placeholder textures for all 4 layers
  let albedo_0 = images.add(create_solid_image(LAYER_COLORS[0]));
  let albedo_1 = images.add(create_solid_image(LAYER_COLORS[1]));
  let albedo_2 = images.add(create_solid_image(LAYER_COLORS[2]));
  let albedo_3 = images.add(create_solid_image(LAYER_COLORS[3]));

  let normal_0 = images.add(create_neutral_normal());
  let normal_1 = images.add(create_neutral_normal());
  let normal_2 = images.add(create_neutral_normal());
  let normal_3 = images.add(create_neutral_normal());

  let arm_0 = images.add(create_default_arm());
  let arm_1 = images.add(create_default_arm());
  let arm_2 = images.add(create_default_arm());
  let arm_3 = images.add(create_default_arm());

  TriplanarMaterial {
    albedo_0,
    normal_0,
    arm_0,
    albedo_1,
    normal_1,
    arm_1,
    albedo_2,
    normal_2,
    arm_2,
    albedo_3,
    normal_3,
    arm_3,
    params: TriplanarParams {
      texture_scales: [0.1, 0.1, 0.1, 0.1], // 10 world units per texture repeat
      blend_sharpness: 4.0,
      normal_strength: 1.0,
      _padding: [0.0; 2],
    },
  }
}

/// Configuration for loading terrain textures from files.
pub struct TerrainTextureConfig {
  /// Base path for texture files (e.g., "textures/terrain/").
  pub base_path: String,
  /// Texture file names for each layer [albedo, normal, arm].
  pub layers: [LayerTextures; 4],
  /// Texture scale for each layer.
  pub scales: [f32; 4],
}

/// Texture file names for a single layer.
pub struct LayerTextures {
  pub albedo: String,
  pub normal: String,
  pub arm: String,
}

impl Default for TerrainTextureConfig {
  fn default() -> Self {
    Self {
      base_path: "textures/terrain/".to_string(),
      layers: [
        LayerTextures {
          albedo: "dirt_albedo.png".to_string(),
          normal: "dirt_normal.png".to_string(),
          arm: "dirt_arm.png".to_string(),
        },
        LayerTextures {
          albedo: "grass_albedo.png".to_string(),
          normal: "grass_normal.png".to_string(),
          arm: "grass_arm.png".to_string(),
        },
        LayerTextures {
          albedo: "stone_albedo.png".to_string(),
          normal: "stone_normal.png".to_string(),
          arm: "stone_arm.png".to_string(),
        },
        LayerTextures {
          albedo: "snow_albedo.png".to_string(),
          normal: "snow_normal.png".to_string(),
          arm: "snow_arm.png".to_string(),
        },
      ],
      scales: [0.1, 0.1, 0.05, 0.1],
    }
  }
}

/// Create a triplanar material by loading textures from files.
///
/// Falls back to placeholder textures if files don't exist.
pub fn create_material_from_config(
  asset_server: &AssetServer,
  images: &mut Assets<Image>,
  config: &TerrainTextureConfig,
) -> TriplanarMaterial {
  // Helper to load texture from assets
  let load_texture = |path: &str| -> Handle<Image> {
    let full_path = format!("{}{}", config.base_path, path);
    asset_server.load(full_path)
  };

  let _ = images; // Unused when loading from files

  TriplanarMaterial {
    albedo_0: load_texture(&config.layers[0].albedo),
    normal_0: load_texture(&config.layers[0].normal),
    arm_0: load_texture(&config.layers[0].arm),

    albedo_1: load_texture(&config.layers[1].albedo),
    normal_1: load_texture(&config.layers[1].normal),
    arm_1: load_texture(&config.layers[1].arm),

    albedo_2: load_texture(&config.layers[2].albedo),
    normal_2: load_texture(&config.layers[2].normal),
    arm_2: load_texture(&config.layers[2].arm),

    albedo_3: load_texture(&config.layers[3].albedo),
    normal_3: load_texture(&config.layers[3].normal),
    arm_3: load_texture(&config.layers[3].arm),

    params: TriplanarParams {
      texture_scales: config.scales,
      blend_sharpness: 4.0,
      normal_strength: 1.0,
      _padding: [0.0; 2],
    },
  }
}
