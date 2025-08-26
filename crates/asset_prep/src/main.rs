use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct ArrayMeta {
	layers: Vec<String>,
}

fn is_albedo(name: &str) -> bool {
	let lower = name.to_ascii_lowercase();
	(lower.contains("color_1k.png")
		|| lower.contains("basecolor_1k.png")
		|| lower.contains("basecolor.png")
		|| lower.contains("base_1k.png")
		|| lower.ends_with("_color.png"))
		&& !lower.contains("normal")
		&& !lower.contains("roughness")
		&& !lower.contains("metallic")
		&& !lower.contains("height")
		&& !lower.contains("ambient")
}

fn collect_one_per_subdir(root: &Path) -> Result<Vec<PathBuf>> {
	let mut picks: Vec<PathBuf> = Vec::new();
	for entry in fs::read_dir(root).context("read_dir root")? {
		let entry = entry?;
		if entry.file_type()?.is_dir() {
			let mut chosen: Option<PathBuf> = None;
			for sub in fs::read_dir(entry.path()).context("read_dir sub")? {
				let sub = sub?;
				let p = sub.path();
				if let Some(name) = p.file_name().and_then(|s| s.to_str())
					&& is_albedo(name) {
						chosen = Some(p.clone());
						break;
					}
			}
			if let Some(p) = chosen {
				picks.push(p);
			}
		}
	}
	Ok(picks)
}

fn load_rgba8(path: &Path) -> Result<DynamicImage> {
	let img = image::open(path).with_context(|| format!("open {:?}", path))?;
	Ok(img.to_rgba8().into())
}

fn main() -> Result<()> {
	let cwd = std::env::current_dir()?;
	let assets = cwd.join("assets");
	let src_root = assets.join("free_stylized_textures");
	let out_dir = assets.join("generated");
	fs::create_dir_all(&out_dir)?;

	let picks = collect_one_per_subdir(&src_root)?;
	if picks.is_empty() {
		println!("no albedo textures found under {:?}", src_root);
		return Ok(());
	}
	let mut images: Vec<DynamicImage> = Vec::new();
	for p in &picks {
		images.push(load_rgba8(p)?);
	}
	let w = images[0].width();
	let h = images[0].height();
	for img in &images {
		if img.width() != w || img.height() != h {
			return Err(anyhow::anyhow!("dimension mismatch: expected {}x{}", w, h));
		}
	}
	let stacked_h = h * images.len() as u32;
	let mut stacked: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, stacked_h);
	for (idx, img) in images.iter().enumerate() {
		let y_off = (idx as u32) * h;
		for y in 0..h {
			for x in 0..w {
				let px = img.get_pixel(x, y);
				stacked.put_pixel(x, y_off + y, px);
			}
		}
	}
	let out_png = out_dir.join("albedo_array_stacked.png");
	stacked
		.save(&out_png)
		.with_context(|| format!("write {:?}", out_png))?;

	let rels: Vec<String> = picks
		.iter()
		.map(|p| {
			p.strip_prefix(&assets)
				.unwrap_or(p)
				.to_string_lossy()
				.to_string()
		})
		.collect();
	let meta = ArrayMeta { layers: rels };
	let out_meta = out_dir.join("albedo_array_stacked.json");
	fs::write(&out_meta, serde_json::to_string_pretty(&meta)?)?;
	println!(
		"wrote {:?} and {:?} ({} layers)",
		out_png,
		out_meta,
		picks.len()
	);
	Ok(())
}
