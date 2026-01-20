//! Texture channel packing for terrain arrays.
//!
//! Channel packing scheme:
//! - diffuse_height: R=Diffuse.R, G=Diffuse.G, B=Diffuse.B, A=Height
//! - normal: R=Normal.X, G=Normal.Y, B=Normal.Z, A=unused(255)
//! - material: R=Roughness, G=Metallic, B=AO, A=unused(255)

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::Path;

use crate::config::{LayerConfig, SolidLayer, TexturedLayer};

/// Packed texture data for a single layer.
pub struct PackedLayer {
	/// Diffuse RGB + Height alpha (RGBA).
	pub diffuse_height: RgbaImage,
	/// Normal XYZ + unused alpha (RGBA).
	pub normal: RgbaImage,
	/// Roughness/Metallic/AO + unused alpha (RGBA).
	pub material: RgbaImage,
}

impl PackedLayer {
	/// Create packed textures from a layer configuration.
	pub fn from_config(
		config: &LayerConfig,
		assets_dir: &Path,
		target_size: u32,
	) -> Result<Self> {
		match config {
			LayerConfig::Textured(t) => Self::from_textured(t, assets_dir, target_size),
			LayerConfig::Solid(s) => Self::from_solid(s, target_size),
		}
	}

	/// Create packed textures from file-based sources.
	fn from_textured(
		config: &TexturedLayer,
		assets_dir: &Path,
		target_size: u32,
	) -> Result<Self> {
		println!("  Loading textures for layer '{}'...", config.name);

		// Load and resize all source textures
		let diffuse = load_and_resize(assets_dir.join(&config.diffuse), target_size)
			.with_context(|| format!("Loading diffuse: {}", config.diffuse))?;
		let height = load_and_resize(assets_dir.join(&config.height), target_size)
			.with_context(|| format!("Loading height: {}", config.height))?;
		let normal = load_and_resize(assets_dir.join(&config.normal), target_size)
			.with_context(|| format!("Loading normal: {}", config.normal))?;
		let roughness = load_and_resize(assets_dir.join(&config.roughness), target_size)
			.with_context(|| format!("Loading roughness: {}", config.roughness))?;
		let ao = load_and_resize(assets_dir.join(&config.ao), target_size)
			.with_context(|| format!("Loading ao: {}", config.ao))?;

		// Metallic is optional, default to black (0)
		let metallic = if let Some(ref path) = config.metallic {
			load_and_resize(assets_dir.join(path), target_size)
				.with_context(|| format!("Loading metallic: {}", path))?
		} else {
			create_solid(target_size, 0)
		};

		// Pack channels
		let diffuse_height = pack_diffuse_height(&diffuse, &height);
		let normal_packed = pack_normal(&normal);
		let material = pack_material(&roughness, &metallic, &ao);

		Ok(Self {
			diffuse_height,
			normal: normal_packed,
			material,
		})
	}

	/// Create packed textures from solid color values.
	fn from_solid(config: &SolidLayer, target_size: u32) -> Result<Self> {
		println!(
			"  Creating solid layer '{}' ({:?})",
			config.name, config.solid_color
		);

		let diffuse = create_solid_rgb(target_size, config.solid_color);
		let height = create_solid(target_size, (config.height_value * 255.0) as u8);
		let normal = create_neutral_normal(target_size);
		let roughness = create_solid(target_size, (config.roughness_value * 255.0) as u8);
		let metallic = create_solid(target_size, (config.metallic_value * 255.0) as u8);
		let ao = create_solid(target_size, (config.ao_value * 255.0) as u8);

		let diffuse_height = pack_diffuse_height(&diffuse, &height);
		let normal_packed = pack_normal(&normal);
		let material = pack_material(&roughness, &metallic, &ao);

		Ok(Self {
			diffuse_height,
			normal: normal_packed,
			material,
		})
	}
}

/// Load an image and resize to target dimensions.
fn load_and_resize<P: AsRef<Path>>(path: P, target_size: u32) -> Result<RgbaImage> {
	let path = path.as_ref();
	let img = image::open(path).with_context(|| format!("Failed to open: {}", path.display()))?;

	let resized = img.resize_exact(target_size, target_size, image::imageops::FilterType::Lanczos3);
	Ok(resized.to_rgba8())
}

/// Create a solid grayscale image.
fn create_solid(size: u32, value: u8) -> RgbaImage {
	ImageBuffer::from_pixel(size, size, Rgba([value, value, value, 255]))
}

/// Create a solid RGB color image.
fn create_solid_rgb(size: u32, color: [u8; 3]) -> RgbaImage {
	ImageBuffer::from_pixel(size, size, Rgba([color[0], color[1], color[2], 255]))
}

/// Create a neutral normal map (pointing up: 0.5, 0.5, 1.0).
fn create_neutral_normal(size: u32) -> RgbaImage {
	// Normal map neutral = (128, 128, 255) = (0.5, 0.5, 1.0) in normalized space
	ImageBuffer::from_pixel(size, size, Rgba([128, 128, 255, 255]))
}

/// Pack diffuse RGB and height into a single RGBA image.
///
/// Output: R=Diffuse.R, G=Diffuse.G, B=Diffuse.B, A=Height.R
fn pack_diffuse_height(diffuse: &RgbaImage, height: &RgbaImage) -> RgbaImage {
	let (width, height_dim) = diffuse.dimensions();
	let mut output = ImageBuffer::new(width, height_dim);

	for (x, y, pixel) in output.enumerate_pixels_mut() {
		let d = diffuse.get_pixel(x, y);
		let h = height.get_pixel(x, y);
		*pixel = Rgba([d[0], d[1], d[2], h[0]]); // Use red channel of height map
	}

	output
}

/// Pack normal map into RGBA (pass-through, set A=255).
///
/// Output: R=Normal.X, G=Normal.Y, B=Normal.Z, A=255
fn pack_normal(normal: &RgbaImage) -> RgbaImage {
	let (width, height) = normal.dimensions();
	let mut output = ImageBuffer::new(width, height);

	for (x, y, pixel) in output.enumerate_pixels_mut() {
		let n = normal.get_pixel(x, y);
		*pixel = Rgba([n[0], n[1], n[2], 255]);
	}

	output
}

/// Pack roughness, metallic, and AO into a single RGBA image.
///
/// Output: R=Roughness, G=Metallic, B=AO, A=255
fn pack_material(roughness: &RgbaImage, metallic: &RgbaImage, ao: &RgbaImage) -> RgbaImage {
	let (width, height) = roughness.dimensions();
	let mut output = ImageBuffer::new(width, height);

	for (x, y, pixel) in output.enumerate_pixels_mut() {
		let r = roughness.get_pixel(x, y);
		let m = metallic.get_pixel(x, y);
		let a = ao.get_pixel(x, y);
		*pixel = Rgba([r[0], m[0], a[0], 255]);
	}

	output
}
