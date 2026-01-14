//! Benchmarks for noise generation - 32³ volume fill workloads.
//!
//! All benchmarks use the same workload: filling a 32x32x32 voxel chunk.
//! This reflects the actual use case for voxel terrain generation.

#![feature(portable_simd)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::simd::{f32x4, f32x8};
use voxel_plugin::fastnoise_lite::{
	simd as noise_simd, simd_x4, simd_x8, FastNoiseLite, FractalType, NoiseType,
};

// External noise libraries for comparison
use fastnoise2::generator::{perlin::perlin, prelude::Generator};
use simdnoise::NoiseBuilder;

const CHUNK_SIZE: usize = 32;
const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE; // 32768
const FREQ: f32 = 0.02;

// ============================================================================
// FastNoiseLite Internal Benchmarks
// ============================================================================

/// Compare different noise types filling a 32³ volume.
fn bench_noise_types(c: &mut Criterion) {
	let mut group = c.benchmark_group("volume_32_noise_types");
	group.throughput(Throughput::Elements(CHUNK_VOLUME as u64));

	let noise_types = [
		("Perlin", NoiseType::Perlin),
		("OpenSimplex2", NoiseType::OpenSimplex2),
		("OpenSimplex2S", NoiseType::OpenSimplex2S),
		("Value", NoiseType::Value),
		("ValueCubic", NoiseType::ValueCubic),
	];

	for (name, noise_type) in noise_types {
		let mut noise = FastNoiseLite::new();
		noise.set_noise_type(Some(noise_type));
		noise.set_frequency(Some(FREQ));

		let mut output = [0.0f32; CHUNK_VOLUME];

		group.bench_function(name, |b| {
			b.iter(|| {
				noise_simd::fill_chunk_32(&mut output, 0.0, 0.0, 0.0, 1.0, &noise);
				black_box(output[0])
			})
		});
	}

	group.finish();
}

/// Compare octave counts for FBm filling a 32³ volume.
fn bench_octaves(c: &mut Criterion) {
	let mut group = c.benchmark_group("volume_32_octaves");
	group.throughput(Throughput::Elements(CHUNK_VOLUME as u64));

	for octaves in [1, 2, 4, 6, 8] {
		let mut noise = FastNoiseLite::new();
		noise.set_noise_type(Some(NoiseType::Perlin));
		noise.set_fractal_type(Some(FractalType::FBm));
		noise.set_fractal_octaves(Some(octaves));
		noise.set_frequency(Some(FREQ));

		let mut output = [0.0f32; CHUNK_VOLUME];

		group.bench_with_input(BenchmarkId::from_parameter(octaves), &octaves, |b, _| {
			b.iter(|| {
				noise_simd::fill_chunk_32(&mut output, 0.0, 0.0, 0.0, 1.0, &noise);
				black_box(output[0])
			})
		});
	}

	group.finish();
}

/// Compare scalar vs SIMD implementations filling a 32³ volume.
fn bench_scalar_vs_simd(c: &mut Criterion) {
	let mut group = c.benchmark_group("volume_32_scalar_vs_simd");
	group.throughput(Throughput::Elements(CHUNK_VOLUME as u64));

	let mut noise = FastNoiseLite::new();
	noise.set_noise_type(Some(NoiseType::Perlin));
	noise.set_fractal_type(Some(FractalType::FBm));
	noise.set_fractal_octaves(Some(4));
	noise.set_frequency(Some(FREQ));

	// Scalar implementation
	{
		let mut output = [0.0f32; CHUNK_VOLUME];
		group.bench_function("scalar", |b| {
			b.iter(|| {
				for x in 0..CHUNK_SIZE {
					for y in 0..CHUNK_SIZE {
						for z in 0..CHUNK_SIZE {
							let idx = x * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + z;
							output[idx] = noise.get_noise_3d(x as f32, y as f32, z as f32);
						}
					}
				}
				black_box(output[0])
			})
		});
	}

	// SIMD fill_chunk_32 (uses internal batching)
	{
		let mut output = [0.0f32; CHUNK_VOLUME];
		group.bench_function("simd_fill_chunk", |b| {
			b.iter(|| {
				noise_simd::fill_chunk_32(&mut output, 0.0, 0.0, 0.0, 1.0, &noise);
				black_box(output[0])
			})
		});
	}

	group.finish();
}

/// Compare SIMD widths (x4 vs x8) filling a 32³ volume.
fn bench_simd_widths(c: &mut Criterion) {
	let mut group = c.benchmark_group("volume_32_simd_widths");
	group.throughput(Throughput::Elements(CHUNK_VOLUME as u64));

	let mut noise = FastNoiseLite::new();
	noise.set_noise_type(Some(NoiseType::Perlin));
	noise.set_fractal_type(Some(FractalType::FBm));
	noise.set_fractal_octaves(Some(4));
	noise.set_frequency(Some(FREQ));

	let seed = noise.seed;
	let freq = noise.frequency;
	let octaves = 4u8;
	let lac = noise.lacunarity;
	let gain = noise.gain;

	// x4: 8 batches per row (32/4)
	{
		let mut output = [0.0f32; CHUNK_VOLUME];
		group.bench_function("x4", |b| {
			b.iter(|| {
				for x in 0..32 {
					let wx = f32x4::splat(x as f32 * freq);
					for y in 0..32 {
						let wy = f32x4::splat(y as f32 * freq);
						let base = x * 1024 + y * 32;
						for z_batch in 0..8 {
							let z_base = z_batch * 4;
							let wz = f32x4::from_array([
								(z_base) as f32 * freq,
								(z_base + 1) as f32 * freq,
								(z_base + 2) as f32 * freq,
								(z_base + 3) as f32 * freq,
							]);
							let values = simd_x4::fbm_3d(seed, wx, wy, wz, octaves, lac, gain);
							output[base + z_base..][..4].copy_from_slice(&values.to_array());
						}
					}
				}
				black_box(output[0])
			})
		});
	}

	// x8: 4 batches per row (32/8)
	{
		let mut output = [0.0f32; CHUNK_VOLUME];
		group.bench_function("x8", |b| {
			b.iter(|| {
				for x in 0..32 {
					let wx = f32x8::splat(x as f32 * freq);
					for y in 0..32 {
						let wy = f32x8::splat(y as f32 * freq);
						let base = x * 1024 + y * 32;
						for z_batch in 0..4 {
							let z_base = z_batch * 8;
							let wz = f32x8::from_array([
								(z_base) as f32 * freq,
								(z_base + 1) as f32 * freq,
								(z_base + 2) as f32 * freq,
								(z_base + 3) as f32 * freq,
								(z_base + 4) as f32 * freq,
								(z_base + 5) as f32 * freq,
								(z_base + 6) as f32 * freq,
								(z_base + 7) as f32 * freq,
							]);
							let values = simd_x8::fbm_3d(seed, wx, wy, wz, octaves, lac, gain);
							output[base + z_base..][..8].copy_from_slice(&values.to_array());
						}
					}
				}
				black_box(output[0])
			})
		});
	}

	group.finish();
}

/// Benchmark multi-chunk throughput (10 chunks).
fn bench_throughput(c: &mut Criterion) {
	let mut group = c.benchmark_group("volume_32_throughput");

	let mut noise = FastNoiseLite::new();
	noise.set_noise_type(Some(NoiseType::Perlin));
	noise.set_fractal_type(Some(FractalType::FBm));
	noise.set_fractal_octaves(Some(4));
	noise.set_frequency(Some(FREQ));

	const NUM_CHUNKS: usize = 10;
	const TOTAL_SAMPLES: usize = CHUNK_VOLUME * NUM_CHUNKS;

	let mut output = [0.0f32; CHUNK_VOLUME];

	group.throughput(Throughput::Elements(TOTAL_SAMPLES as u64));
	group.bench_function("10_chunks", |b| {
		b.iter(|| {
			for chunk in 0..NUM_CHUNKS {
				let offset = chunk as f32 * 32.0;
				noise_simd::fill_chunk_32(&mut output, offset, 0.0, 0.0, 1.0, &noise);
			}
			black_box(output[0])
		})
	});

	group.finish();
}

// ============================================================================
// Library Comparison Benchmarks - All 32³ Volume Fills
// fastnoise_lite (portable_simd) vs simdnoise (runtime AVX2/SSE detection)
// ============================================================================

/// Compare libraries: simple noise (1 octave) filling 32³ volume.
fn bench_lib_simple(c: &mut Criterion) {
	let mut group = c.benchmark_group("lib_volume_32_simple");
	group.throughput(Throughput::Elements(CHUNK_VOLUME as u64));

	// FastNoiseLite SIMD
	{
		let mut noise = FastNoiseLite::new();
		noise.set_noise_type(Some(NoiseType::Perlin));
		noise.set_frequency(Some(FREQ));

		let mut output = [0.0f32; CHUNK_VOLUME];

		group.bench_function("fastnoise_lite", |b| {
			b.iter(|| {
				noise_simd::fill_chunk_32(&mut output, 0.0, 0.0, 0.0, 1.0, &noise);
				black_box(output[0])
			})
		});
	}

	// simdnoise gradient (Perlin-like)
	{
		group.bench_function("simdnoise", |b| {
			b.iter(|| {
				let (noise, _, _) = NoiseBuilder::gradient_3d(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE)
					.with_seed(1337)
					.with_freq(FREQ)
					.generate();
				black_box(noise[0])
			})
		});
	}

	// fastnoise2 (C++ FastNoise2 via FFI)
	{
		let node = perlin().build();
		let mut output = vec![0.0f32; CHUNK_VOLUME];

		group.bench_function("fastnoise2", |b| {
			b.iter(|| {
				node.gen_uniform_grid_3d(
					&mut output,
					0.0,
					0.0,
					0.0,
					CHUNK_SIZE as i32,
					CHUNK_SIZE as i32,
					CHUNK_SIZE as i32,
					FREQ,
					FREQ,
					FREQ,
					1337,
				);
				black_box(output[0])
			})
		});
	}

	group.finish();
}

/// Compare libraries: FBm 4 octaves filling 32³ volume.
/// This is the primary voxel terrain workload.
fn bench_lib_fbm(c: &mut Criterion) {
	let mut group = c.benchmark_group("lib_volume_32_fbm4");
	group.throughput(Throughput::Elements(CHUNK_VOLUME as u64));

	// FastNoiseLite SIMD
	{
		let mut noise = FastNoiseLite::new();
		noise.set_noise_type(Some(NoiseType::Perlin));
		noise.set_fractal_type(Some(FractalType::FBm));
		noise.set_fractal_octaves(Some(4));
		noise.set_frequency(Some(FREQ));

		let mut output = [0.0f32; CHUNK_VOLUME];

		group.bench_function("fastnoise_lite", |b| {
			b.iter(|| {
				noise_simd::fill_chunk_32(&mut output, 0.0, 0.0, 0.0, 1.0, &noise);
				black_box(output[0])
			})
		});
	}

	// simdnoise FBm
	{
		group.bench_function("simdnoise", |b| {
			b.iter(|| {
				let (noise, _, _) = NoiseBuilder::fbm_3d(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE)
					.with_seed(1337)
					.with_freq(FREQ)
					.with_octaves(4)
					.with_lacunarity(2.0)
					.with_gain(0.5)
					.generate();
				black_box(noise[0])
			})
		});
	}

	// fastnoise2 FBm
	{
		let node = perlin().fbm(0.5, 0.0, 4, 2.0).build();
		let mut output = vec![0.0f32; CHUNK_VOLUME];

		group.bench_function("fastnoise2", |b| {
			b.iter(|| {
				node.gen_uniform_grid_3d(
					&mut output,
					0.0,
					0.0,
					0.0,
					CHUNK_SIZE as i32,
					CHUNK_SIZE as i32,
					CHUNK_SIZE as i32,
					FREQ,
					FREQ,
					FREQ,
					1337,
				);
				black_box(output[0])
			})
		});
	}

	group.finish();
}

/// Compare libraries across octave counts filling 32³ volume.
fn bench_lib_octaves(c: &mut Criterion) {
	let mut group = c.benchmark_group("lib_volume_32_octaves");
	group.throughput(Throughput::Elements(CHUNK_VOLUME as u64));

	for octaves in [1usize, 2, 4, 6, 8] {
		// FastNoiseLite
		{
			let mut noise = FastNoiseLite::new();
			noise.set_noise_type(Some(NoiseType::Perlin));
			noise.set_fractal_type(Some(FractalType::FBm));
			noise.set_fractal_octaves(Some(octaves as i32));
			noise.set_frequency(Some(FREQ));

			let mut output = [0.0f32; CHUNK_VOLUME];

			group.bench_with_input(
				BenchmarkId::new("fastnoise_lite", octaves),
				&octaves,
				|b, _| {
					b.iter(|| {
						noise_simd::fill_chunk_32(&mut output, 0.0, 0.0, 0.0, 1.0, &noise);
						black_box(output[0])
					})
				},
			);
		}

		// simdnoise
		{
			group.bench_with_input(BenchmarkId::new("simdnoise", octaves), &octaves, |b, &oct| {
				b.iter(|| {
					let (noise, _, _) = NoiseBuilder::fbm_3d(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE)
						.with_seed(1337)
						.with_freq(FREQ)
						.with_octaves(oct as u8)
						.with_lacunarity(2.0)
						.with_gain(0.5)
						.generate();
					black_box(noise[0])
				})
			});
		}

		// fastnoise2
		{
			let node = perlin().fbm(0.5, 0.0, octaves as i32, 2.0).build();
			let mut output = vec![0.0f32; CHUNK_VOLUME];

			group.bench_with_input(
				BenchmarkId::new("fastnoise2", octaves),
				&octaves,
				|b, _| {
					b.iter(|| {
						node.gen_uniform_grid_3d(
							&mut output,
							0.0,
							0.0,
							0.0,
							CHUNK_SIZE as i32,
							CHUNK_SIZE as i32,
							CHUNK_SIZE as i32,
							FREQ,
							FREQ,
							FREQ,
							1337,
						);
						black_box(output[0])
					})
				},
			);
		}
	}

	group.finish();
}

criterion_group!(
	internal,
	bench_noise_types,
	bench_octaves,
	bench_scalar_vs_simd,
	bench_simd_widths,
	bench_throughput,
);

criterion_group!(library, bench_lib_simple, bench_lib_fbm, bench_lib_octaves,);

criterion_main!(internal, library);
