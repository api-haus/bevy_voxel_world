//! Terrain texture array baker.
//!
//! Bakes source textures into packed KTX2 texture arrays for efficient runtime loading.
//!
//! Channel packing scheme:
//! - diffuse_height.ktx2: R=Diffuse.R, G=Diffuse.G, B=Diffuse.B, A=Height
//! - normal.ktx2: R=Normal.X, G=Normal.Y, B=Normal.Z, A=unused
//! - material.ktx2: R=Roughness, G=Metallic, B=AO, A=unused

mod config;
mod packer;

use anyhow::{Context, Result};
use clap::Parser;
use ktx2_rw::{BasisCompressionParams, Ktx2Texture, TranscodeFormat, VkFormat};
use std::path::{Path, PathBuf};

use config::Config;
use packer::PackedLayer;

/// Terrain texture array baker for voxel framework.
#[derive(Parser, Debug)]
#[command(name = "bake_textures")]
#[command(about = "Bakes terrain textures into packed KTX2 arrays")]
struct Args {
	/// Path to configuration TOML file.
	#[arg(short, long)]
	config: PathBuf,

	/// Assets directory (default: inferred from config path).
	#[arg(short, long)]
	assets_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
	let args = Args::parse();

	// Determine assets directory
	let assets_dir = args.assets_dir.unwrap_or_else(|| {
		args.config
			.parent()
			.unwrap_or(Path::new("."))
			.to_path_buf()
	});

	println!("Loading config from: {}", args.config.display());
	println!("Assets directory: {}", assets_dir.display());

	let config = Config::load(&args.config)?;

	println!(
		"Baking {} layers at {}x{} resolution",
		config.layers.len(),
		config.output_size,
		config.output_size
	);

	// Pack all layers
	let mut packed_layers = Vec::new();
	for layer_config in &config.layers {
		let packed = PackedLayer::from_config(layer_config, &assets_dir, config.output_size)?;
		packed_layers.push(packed);
	}

	// Pad to 4 layers if needed (using the last layer as fill)
	while packed_layers.len() < 4 {
		let last = packed_layers.last().unwrap();
		packed_layers.push(PackedLayer {
			diffuse_height: last.diffuse_height.clone(),
			normal: last.normal.clone(),
			material: last.material.clone(),
		});
	}

	// Create output directory
	let output_dir = assets_dir.join(&config.output_dir);
	std::fs::create_dir_all(&output_dir)
		.with_context(|| format!("Failed to create output dir: {}", output_dir.display()))?;

	// Build and save texture arrays
	println!("\nBuilding KTX2 arrays...");

	let thread_count = std::thread::available_parallelism()
		.map(|n| n.get() as u32)
		.unwrap_or(4);

	// Diffuse+Height array (sRGB for diffuse colors)
	build_ktx2_array(
		&packed_layers.iter().map(|l| &l.diffuse_height).collect::<Vec<_>>(),
		config.output_size,
		VkFormat::R8G8B8A8Srgb, // sRGB for color data
		&output_dir.join("diffuse_height.ktx2"),
		false,
		thread_count,
	)
	.context("Building diffuse_height.ktx2")?;
	println!("  ✓ diffuse_height.ktx2");

	// Normal array (linear)
	build_ktx2_array(
		&packed_layers.iter().map(|l| &l.normal).collect::<Vec<_>>(),
		config.output_size,
		VkFormat::R8G8B8A8Unorm, // Linear for normal data
		&output_dir.join("normal.ktx2"),
		true,
		thread_count,
	)
	.context("Building normal.ktx2")?;
	println!("  ✓ normal.ktx2");

	// Material array (linear)
	build_ktx2_array(
		&packed_layers.iter().map(|l| &l.material).collect::<Vec<_>>(),
		config.output_size,
		VkFormat::R8G8B8A8Unorm, // Linear for material data
		&output_dir.join("material.ktx2"),
		false,
		thread_count,
	)
	.context("Building material.ktx2")?;
	println!("  ✓ material.ktx2");

	println!("\nDone! Output written to: {}", output_dir.display());

	Ok(())
}

/// Build a KTX2 2D array texture from packed layers with Basis Universal compression.
fn build_ktx2_array(
	layers: &[&image::RgbaImage],
	size: u32,
	format: VkFormat,
	output_path: &Path,
	is_normal_map: bool,
	thread_count: u32,
) -> Result<()> {
	let num_layers = layers.len() as u32;

	// Create KTX2 texture: 2D array (depth=1, faces=1, levels=1)
	let mut texture = Ktx2Texture::create(size, size, 1, num_layers, 1, 1, format)
		.context("Failed to create KTX2 texture")?;

	// Set image data for each layer
	for (layer_idx, layer_data) in layers.iter().enumerate() {
		let raw_data = layer_data.as_raw();
		texture
			.set_image_data(0, layer_idx as u32, 0, raw_data)
			.with_context(|| format!("Failed to set image data for layer {}", layer_idx))?;
	}

	// Compress with Basis Universal (ETC1S), then transcode to ETC2 GPU format.
	// ETC2 is natively supported by WebGL2 and all modern desktop GPUs,
	// so no runtime transcoder (basis-universal) is needed.
	let params = BasisCompressionParams::builder()
		.quality_level(128)
		.thread_count(thread_count)
		.normal_map(is_normal_map)
		.build();
	texture
		.compress_basis(&params)
		.context("Basis Universal compression failed")?;
	texture
		.transcode_basis(TranscodeFormat::Etc2Rgba)
		.context("ETC2 transcoding failed")?;

	// Write to file
	texture
		.write_to_file(output_path)
		.with_context(|| format!("Failed to write: {}", output_path.display()))?;

	Ok(())
}
