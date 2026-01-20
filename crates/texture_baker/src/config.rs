//! Configuration parsing for terrain texture baking.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Root configuration for terrain texture baking.
#[derive(Debug, Deserialize)]
pub struct Config {
	/// Output directory relative to assets folder.
	pub output_dir: String,
	/// Target size for all textures (square).
	pub output_size: u32,
	/// Layer definitions.
	pub layers: Vec<LayerConfig>,
}

/// Configuration for a single texture layer.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LayerConfig {
	/// Layer with actual texture files.
	Textured(TexturedLayer),
	/// Layer with solid color values.
	Solid(SolidLayer),
}

/// Layer backed by texture files.
#[derive(Debug, Deserialize)]
pub struct TexturedLayer {
	/// Layer name for identification.
	pub name: String,
	/// Path to diffuse/albedo texture.
	pub diffuse: String,
	/// Path to height map.
	pub height: String,
	/// Path to normal map.
	pub normal: String,
	/// Path to roughness map.
	pub roughness: String,
	/// Path to ambient occlusion map.
	pub ao: String,
	/// Path to metallic map (optional, defaults to 0).
	pub metallic: Option<String>,
}

/// Layer with solid color values (for placeholders).
#[derive(Debug, Deserialize)]
pub struct SolidLayer {
	/// Layer name for identification.
	pub name: String,
	/// Solid diffuse color [R, G, B].
	pub solid_color: [u8; 3],
	/// Roughness value 0.0-1.0.
	#[serde(default = "default_roughness")]
	pub roughness_value: f32,
	/// Ambient occlusion value 0.0-1.0.
	#[serde(default = "default_ao")]
	pub ao_value: f32,
	/// Metallic value 0.0-1.0.
	#[serde(default)]
	pub metallic_value: f32,
	/// Height value 0.0-1.0 (flat).
	#[serde(default = "default_height")]
	pub height_value: f32,
}

fn default_roughness() -> f32 {
	0.5
}

fn default_ao() -> f32 {
	1.0
}

fn default_height() -> f32 {
	0.5
}

impl Config {
	/// Load configuration from a TOML file.
	pub fn load(path: &Path) -> Result<Self> {
		let content = std::fs::read_to_string(path)
			.with_context(|| format!("Failed to read config file: {}", path.display()))?;
		let config: Config =
			toml::from_str(&content).with_context(|| "Failed to parse config TOML")?;

		if config.layers.is_empty() {
			anyhow::bail!("Config must have at least one layer");
		}
		if config.layers.len() > 4 {
			anyhow::bail!("Maximum 4 layers supported, found {}", config.layers.len());
		}
		if config.output_size == 0 || !config.output_size.is_power_of_two() {
			anyhow::bail!(
				"output_size must be a power of 2, got {}",
				config.output_size
			);
		}

		Ok(config)
	}
}

impl LayerConfig {
	/// Get the layer name.
	#[allow(dead_code)]
	pub fn name(&self) -> &str {
		match self {
			LayerConfig::Textured(t) => &t.name,
			LayerConfig::Solid(s) => &s.name,
		}
	}
}
