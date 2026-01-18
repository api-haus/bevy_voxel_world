//! Benchmark comparing voxel_plugin's surface_nets against fast_surface_nets
//! crate.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use fast_surface_nets::ndshape::{ConstShape, ConstShape3u32};
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};
use voxel_plugin::{
  sdf_conversion, surface_nets as my_surface_nets, MeshConfig, SdfSample, SAMPLE_SIZE,
  SAMPLE_SIZE_CB,
};

/// Grid shape for fast_surface_nets (32³).
type GridShape = ConstShape3u32<32, 32, 32>;

/// Generate sphere SDF for our implementation (i8 quantized).
fn generate_sphere_sdf_i8(center: [f32; 3], radius: f32) -> [SdfSample; SAMPLE_SIZE_CB] {
  let mut sdf = [0i8; SAMPLE_SIZE_CB];

  for x in 0..SAMPLE_SIZE {
    for y in 0..SAMPLE_SIZE {
      for z in 0..SAMPLE_SIZE {
        let dx = x as f32 - center[0];
        let dy = y as f32 - center[1];
        let dz = z as f32 - center[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt() - radius;
        let idx = (x << 10) | (y << 5) | z;
        sdf[idx] = sdf_conversion::to_storage(distance, 1.0);
      }
    }
  }

  sdf
}

/// Generate sphere SDF for fast_surface_nets (f32).
fn generate_sphere_sdf_f32(center: [f32; 3], radius: f32) -> [f32; GridShape::USIZE] {
  let mut sdf = [1.0f32; GridShape::USIZE];

  for i in 0u32..GridShape::SIZE {
    let [x, y, z] = GridShape::delinearize(i);
    let dx = x as f32 - center[0];
    let dy = y as f32 - center[1];
    let dz = z as f32 - center[2];
    let distance = (dx * dx + dy * dy + dz * dz).sqrt() - radius;
    sdf[i as usize] = distance;
  }

  sdf
}

/// Benchmark our surface nets implementation.
fn bench_our_surface_nets(c: &mut Criterion) {
  let center = [16.0, 16.0, 16.0];
  let radius = 12.0;
  let sdf = generate_sphere_sdf_i8(center, radius);
  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::default();

  c.bench_function("voxel_plugin::surface_nets (32³ sphere)", |b| {
    b.iter(|| {
      let output = my_surface_nets::generate(black_box(&sdf), black_box(&materials), &config);
      black_box(output)
    })
  });
}

/// Benchmark fast_surface_nets crate.
fn bench_fast_surface_nets(c: &mut Criterion) {
  let center = [16.0, 16.0, 16.0];
  let radius = 12.0;
  let sdf = generate_sphere_sdf_f32(center, radius);

  c.bench_function("fast_surface_nets (32³ sphere)", |b| {
    b.iter(|| {
      let mut buffer = SurfaceNetsBuffer::default();
      surface_nets(black_box(&sdf), &GridShape {}, [0; 3], [31; 3], &mut buffer);
      black_box(buffer)
    })
  });
}

/// Direct comparison with varying sphere radii.
fn bench_comparison(c: &mut Criterion) {
  let mut group = c.benchmark_group("surface_nets_comparison");

  for radius in [8.0, 12.0, 14.0] {
    let center = [16.0, 16.0, 16.0];

    // Prepare data for our implementation
    let sdf_i8 = generate_sphere_sdf_i8(center, radius);
    let materials = [0u8; SAMPLE_SIZE_CB];
    let config = MeshConfig::default();

    // Prepare data for fast_surface_nets
    let sdf_f32 = generate_sphere_sdf_f32(center, radius);

    group.bench_with_input(
      BenchmarkId::new("voxel_plugin", format!("r={}", radius)),
      &radius,
      |b, _| {
        b.iter(|| my_surface_nets::generate(black_box(&sdf_i8), black_box(&materials), &config))
      },
    );

    group.bench_with_input(
      BenchmarkId::new("fast_surface_nets", format!("r={}", radius)),
      &radius,
      |b, _| {
        b.iter(|| {
          let mut buffer = SurfaceNetsBuffer::default();
          surface_nets(
            black_box(&sdf_f32),
            &GridShape {},
            [0; 3],
            [31; 3],
            &mut buffer,
          );
          black_box(buffer)
        })
      },
    );
  }

  group.finish();
}

/// Benchmark multiple spheres (simulating complex terrain).
fn bench_complex_sdf(c: &mut Criterion) {
  let mut group = c.benchmark_group("complex_sdf");

  // Generate overlapping spheres SDF for our implementation
  let mut sdf_i8 = [127i8; SAMPLE_SIZE_CB];
  let spheres = [
    ([10.0, 16.0, 16.0], 8.0),
    ([22.0, 16.0, 16.0], 8.0),
    ([16.0, 10.0, 16.0], 6.0),
    ([16.0, 22.0, 16.0], 6.0),
    ([16.0, 16.0, 16.0], 10.0),
  ];

  for x in 0..SAMPLE_SIZE {
    for y in 0..SAMPLE_SIZE {
      for z in 0..SAMPLE_SIZE {
        let mut min_dist = f32::MAX;
        for (center, radius) in &spheres {
          let dx = x as f32 - center[0];
          let dy = y as f32 - center[1];
          let dz = z as f32 - center[2];
          let dist = (dx * dx + dy * dy + dz * dz).sqrt() - radius;
          min_dist = min_dist.min(dist);
        }
        let idx = (x << 10) | (y << 5) | z;
        sdf_i8[idx] = sdf_conversion::to_storage(min_dist, 1.0);
      }
    }
  }

  // Generate for fast_surface_nets
  let mut sdf_f32 = [f32::MAX; GridShape::USIZE];
  for i in 0u32..GridShape::SIZE {
    let [x, y, z] = GridShape::delinearize(i);
    let mut min_dist = f32::MAX;
    for (center, radius) in &spheres {
      let dx = x as f32 - center[0];
      let dy = y as f32 - center[1];
      let dz = z as f32 - center[2];
      let dist = (dx * dx + dy * dy + dz * dz).sqrt() - radius;
      min_dist = min_dist.min(dist);
    }
    sdf_f32[i as usize] = min_dist;
  }

  let materials = [0u8; SAMPLE_SIZE_CB];
  let config = MeshConfig::default();

  group.bench_function("voxel_plugin (5 spheres)", |b| {
    b.iter(|| my_surface_nets::generate(black_box(&sdf_i8), black_box(&materials), &config))
  });

  group.bench_function("fast_surface_nets (5 spheres)", |b| {
    b.iter(|| {
      let mut buffer = SurfaceNetsBuffer::default();
      surface_nets(
        black_box(&sdf_f32),
        &GridShape {},
        [0; 3],
        [31; 3],
        &mut buffer,
      );
      black_box(buffer)
    })
  });

  group.finish();
}

criterion_group!(
  benches,
  bench_our_surface_nets,
  bench_fast_surface_nets,
  bench_comparison,
  bench_complex_sdf
);
criterion_main!(benches);
