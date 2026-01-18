//! Pipeline architecture benchmarks.
//!
//! Compares three pipeline strategies:
//! - **A: Presample-first**: sample 32³ → check homogeneous → mesh
//! - **B: On-demand**: batch presample via rayon
//! - **C: No presample**: sample 32³ → mesh (direct approach)
//!
//! Each strategy is tested with different sampler scenarios:
//! - **worst_case**: High-frequency noise (surfaces everywhere)
//! - **realistic**: Terrain with caves/islands (mix of homogeneous/surface)
//! - **controlled**: Sphere (predictable surface ratio)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use voxel_plugin::{
  constants::{SAMPLE_SIZE, SAMPLE_SIZE_CB},
  octree::{OctreeConfig, OctreeNode},
  pipeline::{
    meshing::mesh_batch,
    presample::{presample_batch, presample_node},
    types::{MeshInput, VolumeSampler, WorkSource},
  },
  sdf_conversion,
  surface_nets::generate as mesh_generate,
  MaterialId, MeshConfig, SdfSample,
};

// =============================================================================
// Synthetic SDF Samplers
// =============================================================================

/// High-frequency noise sampler (worst case: surfaces everywhere).
///
/// Uses a simple procedural noise based on hash functions.
/// Produces ~50% surface crossings per chunk.
#[derive(Clone)]
pub struct NoiseSampler {
  frequency: f64,
  seed: u32,
}

impl NoiseSampler {
  pub fn new(frequency: f64, seed: u32) -> Self {
    Self { frequency, seed }
  }

  /// High frequency = more surface crossings per chunk.
  pub fn worst_case() -> Self {
    Self::new(0.5, 12345)
  }

  /// Lower frequency = fewer crossings, more homogeneous regions.
  pub fn medium_frequency() -> Self {
    Self::new(0.1, 12345)
  }
}

impl VolumeSampler for NoiseSampler {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    for x in 0..SAMPLE_SIZE {
      for y in 0..SAMPLE_SIZE {
        for z in 0..SAMPLE_SIZE {
          let idx = x * SAMPLE_SIZE * SAMPLE_SIZE + y * SAMPLE_SIZE + z;
          let wx = (grid_offset[0] + x as i64) as f64 * voxel_size;
          let wy = (grid_offset[1] + y as i64) as f64 * voxel_size;
          let wz = (grid_offset[2] + z as i64) as f64 * voxel_size;

          let fx = wx * self.frequency;
          let fy = wy * self.frequency;
          let fz = wz * self.frequency;

          // Simple 3D value noise using hash
          let value = hash_noise_3d(fx, fy, fz, self.seed);

          // Scale to SDF range then quantize
          let sdf = value * 10.0;
          volume[idx] = sdf_conversion::to_storage(sdf as f32, voxel_size as f32);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Terrain sampler with Y-bias (realistic scenario).
///
/// Produces terrain-like SDFs with:
/// - Ground below Y=0
/// - Air above Y=32
/// - Noisy surface in between with caves
#[derive(Clone)]
pub struct TerrainSampler {
  surface_y: f64,
  noise_amplitude: f64,
  cave_frequency: f64,
  seed: u32,
}

impl TerrainSampler {
  pub fn new(surface_y: f64, noise_amplitude: f64, cave_frequency: f64, seed: u32) -> Self {
    Self {
      surface_y,
      noise_amplitude,
      cave_frequency,
      seed,
    }
  }

  /// Standard terrain with surface at y=16 and caves.
  pub fn standard() -> Self {
    Self::new(16.0, 4.0, 0.1, 42)
  }

  /// Dense terrain (mostly solid, few surfaces).
  pub fn dense() -> Self {
    Self::new(48.0, 2.0, 0.05, 42)
  }

  /// Sparse terrain (mostly air, few surfaces).
  pub fn sparse() -> Self {
    Self::new(-16.0, 2.0, 0.05, 42)
  }
}

impl VolumeSampler for TerrainSampler {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    for xi in 0..SAMPLE_SIZE {
      for yi in 0..SAMPLE_SIZE {
        for zi in 0..SAMPLE_SIZE {
          let idx = xi * SAMPLE_SIZE * SAMPLE_SIZE + yi * SAMPLE_SIZE + zi;
          let x = (grid_offset[0] + xi as i64) as f64 * voxel_size;
          let y = (grid_offset[1] + yi as i64) as f64 * voxel_size;
          let z = (grid_offset[2] + zi as i64) as f64 * voxel_size;

          // Base terrain: distance from surface plane
          let surface_noise =
            hash_noise_3d(x * 0.05, 0.0, z * 0.05, self.seed) * self.noise_amplitude;
          let terrain_sdf = y - (self.surface_y + surface_noise);

          // Cave carving: negative values carve into terrain
          let cave_noise = hash_noise_3d(
            x * self.cave_frequency,
            y * self.cave_frequency,
            z * self.cave_frequency,
            self.seed.wrapping_add(1000),
          );
          let cave_sdf = if cave_noise > 0.3 {
            (cave_noise - 0.3) * 20.0 // Carve caves where noise > 0.3
          } else {
            0.0
          };

          let final_sdf = terrain_sdf.max(-cave_sdf);
          volume[idx] = sdf_conversion::to_storage(final_sdf as f32, voxel_size as f32);

          // Simple material based on depth
          let depth = self.surface_y - y;
          materials[idx] = if depth < 1.0 {
            0 // Grass
          } else if depth < 5.0 {
            1 // Dirt
          } else {
            2 // Stone
          };
        }
      }
    }
  }
}

/// Sphere sampler for controlled benchmarks (predictable surface ratio).
#[derive(Clone)]
pub struct SphereSampler {
  center: [f64; 3],
  radius: f64,
}

impl SphereSampler {
  pub fn new(center: [f64; 3], radius: f64) -> Self {
    Self { center, radius }
  }

  /// Sphere centered in chunk with radius 12.
  pub fn standard() -> Self {
    Self::new([16.0, 16.0, 16.0], 12.0)
  }

  /// Small sphere (fewer surface voxels).
  pub fn small() -> Self {
    Self::new([16.0, 16.0, 16.0], 6.0)
  }

  /// Large sphere (more surface voxels).
  pub fn large() -> Self {
    Self::new([16.0, 16.0, 16.0], 14.0)
  }
}

impl VolumeSampler for SphereSampler {
  fn sample_volume(
    &self,
    grid_offset: [i64; 3],
    voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    for xi in 0..SAMPLE_SIZE {
      for yi in 0..SAMPLE_SIZE {
        for zi in 0..SAMPLE_SIZE {
          let idx = xi * SAMPLE_SIZE * SAMPLE_SIZE + yi * SAMPLE_SIZE + zi;
          let wx = (grid_offset[0] + xi as i64) as f64 * voxel_size;
          let wy = (grid_offset[1] + yi as i64) as f64 * voxel_size;
          let wz = (grid_offset[2] + zi as i64) as f64 * voxel_size;

          let dx = wx - self.center[0];
          let dy = wy - self.center[1];
          let dz = wz - self.center[2];
          let dist = (dx * dx + dy * dy + dz * dz).sqrt() - self.radius;
          volume[idx] = sdf_conversion::to_storage(dist as f32, voxel_size as f32);
          materials[idx] = 0;
        }
      }
    }
  }
}

/// Constant sampler for homogeneous baseline.
#[derive(Clone)]
pub struct ConstantSampler {
  value: SdfSample,
}

impl ConstantSampler {
  pub fn all_air() -> Self {
    Self { value: 127 }
  }

  pub fn all_solid() -> Self {
    Self { value: -127 }
  }
}

impl VolumeSampler for ConstantSampler {
  fn sample_volume(
    &self,
    _grid_offset: [i64; 3],
    _voxel_size: f64,
    volume: &mut [SdfSample; SAMPLE_SIZE_CB],
    materials: &mut [MaterialId; SAMPLE_SIZE_CB],
  ) {
    volume.fill(self.value);
    materials.fill(0);
  }
}

// =============================================================================
// Hash-based noise (no external deps)
// =============================================================================

/// Simple 3D hash noise returning [-1, 1].
fn hash_noise_3d(x: f64, y: f64, z: f64, seed: u32) -> f64 {
  // Integer cell coordinates
  let ix = x.floor() as i32;
  let iy = y.floor() as i32;
  let iz = z.floor() as i32;

  // Fractional position within cell
  let fx = x - x.floor();
  let fy = y - y.floor();
  let fz = z - z.floor();

  // Smoothstep for interpolation
  let ux = smoothstep(fx);
  let uy = smoothstep(fy);
  let uz = smoothstep(fz);

  // Hash 8 corners and trilinear interpolate
  let c000 = hash_to_float(hash_3d(ix, iy, iz, seed));
  let c100 = hash_to_float(hash_3d(ix + 1, iy, iz, seed));
  let c010 = hash_to_float(hash_3d(ix, iy + 1, iz, seed));
  let c110 = hash_to_float(hash_3d(ix + 1, iy + 1, iz, seed));
  let c001 = hash_to_float(hash_3d(ix, iy, iz + 1, seed));
  let c101 = hash_to_float(hash_3d(ix + 1, iy, iz + 1, seed));
  let c011 = hash_to_float(hash_3d(ix, iy + 1, iz + 1, seed));
  let c111 = hash_to_float(hash_3d(ix + 1, iy + 1, iz + 1, seed));

  // Trilinear interpolation
  let x00 = lerp(c000, c100, ux);
  let x10 = lerp(c010, c110, ux);
  let x01 = lerp(c001, c101, ux);
  let x11 = lerp(c011, c111, ux);

  let y0 = lerp(x00, x10, uy);
  let y1 = lerp(x01, x11, uy);

  lerp(y0, y1, uz)
}

#[inline]
fn smoothstep(t: f64) -> f64 {
  t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
  a + (b - a) * t
}

/// Hash 3D integer coordinates to u32.
#[inline]
fn hash_3d(x: i32, y: i32, z: i32, seed: u32) -> u32 {
  let mut h = seed;
  h ^= x as u32;
  h = h.wrapping_mul(0x85ebca6b);
  h ^= y as u32;
  h = h.wrapping_mul(0xc2b2ae35);
  h ^= z as u32;
  h = h.wrapping_mul(0x27d4eb2d);
  h ^= h >> 15;
  h
}

/// Convert hash to float in [-1, 1].
#[inline]
fn hash_to_float(h: u32) -> f64 {
  (h as f64 / u32::MAX as f64) * 2.0 - 1.0
}

// =============================================================================
// Volume sampling utilities
// =============================================================================

/// Sample full 32³ volume from sampler.
fn sample_full_volume<S: VolumeSampler>(
  sampler: &S,
  node: &OctreeNode,
  config: &OctreeConfig,
) -> (
  Box<[SdfSample; SAMPLE_SIZE_CB]>,
  Box<[MaterialId; SAMPLE_SIZE_CB]>,
) {
  let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
  let mut materials = Box::new([0u8; SAMPLE_SIZE_CB]);

  let node_min = config.get_node_min(node);
  let voxel_size = config.get_voxel_size(node.lod);

  let grid_offset = [
    (node_min.x / voxel_size).round() as i64,
    (node_min.y / voxel_size).round() as i64,
    (node_min.z / voxel_size).round() as i64,
  ];
  sampler.sample_volume(grid_offset, voxel_size, &mut volume, &mut materials);

  (volume, materials)
}

/// Check if volume is homogeneous (skip meshing optimization).
fn is_homogeneous(volume: &[SdfSample; SAMPLE_SIZE_CB]) -> bool {
  let first_sign = volume[0] < 0;
  volume.iter().all(|&v| (v < 0) == first_sign)
}

// =============================================================================
// Test fixtures
// =============================================================================

fn test_config() -> OctreeConfig {
  OctreeConfig {
    voxel_size: 1.0,
    min_lod: 0,
    max_lod: 5,
    ..OctreeConfig::default()
  }
}

fn test_node() -> OctreeNode {
  OctreeNode::new(0, 0, 0, 0)
}

fn mesh_config() -> MeshConfig {
  MeshConfig::default()
}

// =============================================================================
// Pipeline strategies (generic over sampler type)
// =============================================================================

/// Strategy A: Presample full volume first, then check homogeneous, then mesh.
fn pipeline_presample_first<S: VolumeSampler>(
  sampler: &S,
  node: &OctreeNode,
  octree_config: &OctreeConfig,
  mesh_config: &MeshConfig,
) -> Option<voxel_plugin::MeshOutput> {
  // Step 1: Sample full 32³ volume upfront
  let (volume, materials) = sample_full_volume(sampler, node, octree_config);

  // Step 2: Check if homogeneous (skip meshing)
  if is_homogeneous(&volume) {
    return None;
  }

  // Step 3: Mesh the volume
  Some(mesh_generate(&volume, &materials, mesh_config))
}

/// Strategy B: On-demand sampling (current approach) via presample.
fn pipeline_on_demand<S: VolumeSampler>(
  sampler: &S,
  node: &OctreeNode,
  octree_config: &OctreeConfig,
  mesh_config: &MeshConfig,
) -> Option<voxel_plugin::MeshOutput> {
  // Step 1: Prefilter (presample + homogeneity check)
  let presample_output = presample_node(*node, WorkSource::Refinement, sampler, octree_config);

  // Step 2: If homogeneous, skip
  let sampled = presample_output.volume?;

  // Step 3: Mesh the volume
  Some(mesh_generate(
    &sampled.volume,
    &sampled.materials,
    mesh_config,
  ))
}

/// Strategy C: No presample - always sample and mesh (direct approach).
fn pipeline_no_presample<S: VolumeSampler>(
  sampler: &S,
  node: &OctreeNode,
  octree_config: &OctreeConfig,
  mesh_config: &MeshConfig,
) -> voxel_plugin::MeshOutput {
  // Step 1: Sample full 32³ volume (always)
  let (volume, materials) = sample_full_volume(sampler, node, octree_config);

  // Step 2: Mesh directly (no homogeneity check)
  mesh_generate(&volume, &materials, mesh_config)
}

// =============================================================================
// Isolated Stage Benchmarks
// =============================================================================

/// Benchmark presample stage in isolation.
fn bench_presample_isolated(c: &mut Criterion) {
  let mut group = c.benchmark_group("isolated/presample");
  let config = test_config();
  let node = test_node();

  // Noise worst case
  let noise = NoiseSampler::worst_case();
  group.bench_function("noise_worst", |b| {
    b.iter(|| {
      presample_node(
        black_box(node),
        black_box(WorkSource::Refinement),
        &noise,
        black_box(&config),
      )
    })
  });

  // Terrain standard
  let terrain = TerrainSampler::standard();
  group.bench_function("terrain_std", |b| {
    b.iter(|| {
      presample_node(
        black_box(node),
        black_box(WorkSource::Refinement),
        &terrain,
        black_box(&config),
      )
    })
  });

  // Sphere standard
  let sphere = SphereSampler::standard();
  group.bench_function("sphere_std", |b| {
    b.iter(|| {
      presample_node(
        black_box(node),
        black_box(WorkSource::Refinement),
        &sphere,
        black_box(&config),
      )
    })
  });

  // Constant air (homogeneous - should be fastest)
  let air = ConstantSampler::all_air();
  group.bench_function("constant_air", |b| {
    b.iter(|| {
      presample_node(
        black_box(node),
        black_box(WorkSource::Refinement),
        &air,
        black_box(&config),
      )
    })
  });

  group.finish();
}

/// Benchmark meshing stage in isolation (with pre-sampled volume).
fn bench_meshing_isolated(c: &mut Criterion) {
  let mut group = c.benchmark_group("isolated/meshing");
  let config = test_config();
  let mesh_cfg = mesh_config();
  let node = test_node();

  // Pre-generate volumes for each sampler
  let sphere_vol = sample_full_volume(&SphereSampler::standard(), &node, &config);
  let noise_vol = sample_full_volume(&NoiseSampler::worst_case(), &node, &config);
  let terrain_vol = sample_full_volume(&TerrainSampler::standard(), &node, &config);

  group.bench_function("sphere", |b| {
    b.iter(|| {
      mesh_generate(
        black_box(&sphere_vol.0),
        black_box(&sphere_vol.1),
        black_box(&mesh_cfg),
      )
    })
  });

  group.bench_function("noise_worst", |b| {
    b.iter(|| {
      mesh_generate(
        black_box(&noise_vol.0),
        black_box(&noise_vol.1),
        black_box(&mesh_cfg),
      )
    })
  });

  group.bench_function("terrain", |b| {
    b.iter(|| {
      mesh_generate(
        black_box(&terrain_vol.0),
        black_box(&terrain_vol.1),
        black_box(&mesh_cfg),
      )
    })
  });

  group.finish();
}

/// Benchmark just the sampling operation (no presample or mesh).
fn bench_sampling_isolated(c: &mut Criterion) {
  let mut group = c.benchmark_group("isolated/sampling");
  let config = test_config();
  let node = test_node();

  let noise = NoiseSampler::worst_case();
  group.bench_function("noise_worst_32x32x32", |b| {
    b.iter(|| sample_full_volume(&noise, black_box(&node), black_box(&config)))
  });

  let terrain = TerrainSampler::standard();
  group.bench_function("terrain_std_32x32x32", |b| {
    b.iter(|| sample_full_volume(&terrain, black_box(&node), black_box(&config)))
  });

  let sphere = SphereSampler::standard();
  group.bench_function("sphere_std_32x32x32", |b| {
    b.iter(|| sample_full_volume(&sphere, black_box(&node), black_box(&config)))
  });

  group.finish();
}

// =============================================================================
// Pipeline Architecture Benchmarks
// =============================================================================

/// Benchmark pipeline architectures with a single chunk.
fn bench_pipeline_single_chunk(c: &mut Criterion) {
  let mut group = c.benchmark_group("pipeline/single_chunk");
  let octree_config = test_config();
  let mesh_cfg = mesh_config();
  let node = test_node();

  // Noise worst case
  let noise = NoiseSampler::worst_case();
  group.bench_function("presample_first/noise_worst", |b| {
    b.iter(|| {
      pipeline_presample_first(
        &noise,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("on_demand/noise_worst", |b| {
    b.iter(|| {
      pipeline_on_demand(
        &noise,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("no_presample/noise_worst", |b| {
    b.iter(|| {
      pipeline_no_presample(
        &noise,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });

  // Terrain standard
  let terrain = TerrainSampler::standard();
  group.bench_function("presample_first/terrain_std", |b| {
    b.iter(|| {
      pipeline_presample_first(
        &terrain,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("on_demand/terrain_std", |b| {
    b.iter(|| {
      pipeline_on_demand(
        &terrain,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("no_presample/terrain_std", |b| {
    b.iter(|| {
      pipeline_no_presample(
        &terrain,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });

  // Terrain dense (mostly solid - tests homogeneous optimization)
  let terrain_dense = TerrainSampler::dense();
  group.bench_function("presample_first/terrain_dense", |b| {
    b.iter(|| {
      pipeline_presample_first(
        &terrain_dense,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("on_demand/terrain_dense", |b| {
    b.iter(|| {
      pipeline_on_demand(
        &terrain_dense,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("no_presample/terrain_dense", |b| {
    b.iter(|| {
      pipeline_no_presample(
        &terrain_dense,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });

  // Sphere standard
  let sphere = SphereSampler::standard();
  group.bench_function("presample_first/sphere_std", |b| {
    b.iter(|| {
      pipeline_presample_first(
        &sphere,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("on_demand/sphere_std", |b| {
    b.iter(|| {
      pipeline_on_demand(
        &sphere,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });
  group.bench_function("no_presample/sphere_std", |b| {
    b.iter(|| {
      pipeline_no_presample(
        &sphere,
        black_box(&node),
        black_box(&octree_config),
        black_box(&mesh_cfg),
      )
    })
  });

  group.finish();
}

/// Benchmark pipeline architectures with batches of chunks.
///
/// This tests realistic scenarios where multiple chunks are processed,
/// measuring how well each strategy handles mixes of homogeneous and surface
/// chunks.
fn bench_pipeline_batch(c: &mut Criterion) {
  let mut group = c.benchmark_group("pipeline/batch");
  let octree_config = test_config();
  let mesh_cfg = mesh_config();

  // Create a batch of nodes spanning different regions
  let nodes: Vec<OctreeNode> = (0..8)
    .flat_map(|x| (0..8).flat_map(move |y| (0..8).map(move |z| OctreeNode::new(x, y, z, 0))))
    .collect(); // 512 nodes

  let batch_sizes = [8, 64, 512];

  // Terrain scenario (realistic mix of homogeneous/surface)
  let terrain = TerrainSampler::standard();

  for &batch_size in &batch_sizes {
    let batch: Vec<_> = nodes.iter().take(batch_size).cloned().collect();

    // Strategy A: Presample first (batch)
    group.bench_with_input(
      BenchmarkId::new("presample_first/terrain", batch_size),
      &batch_size,
      |b, _| {
        b.iter(|| {
          let outputs: Vec<_> = batch
            .iter()
            .filter_map(|node| pipeline_presample_first(&terrain, node, &octree_config, &mesh_cfg))
            .collect();
          black_box(outputs)
        })
      },
    );

    // Strategy B: On-demand (current) using presample_batch
    group.bench_with_input(
      BenchmarkId::new("on_demand/terrain", batch_size),
      &batch_size,
      |b, _| {
        b.iter(|| {
          // Use the batch API for realistic parallel processing
          let nodes_with_source: Vec<_> =
            batch.iter().map(|&n| (n, WorkSource::Refinement)).collect();

          let presampled = presample_batch(nodes_with_source, &terrain, &octree_config);

          // Convert to mesh inputs
          let mesh_inputs: Vec<_> = presampled
            .into_iter()
            .filter_map(|output| {
              output.volume.map(|sampled| MeshInput {
                node: output.node,
                volume: sampled.volume,
                materials: sampled.materials,
                config: mesh_cfg.clone(),
                work_source: output.work_source,
              })
            })
            .collect();

          let mesh_results = mesh_batch(mesh_inputs);
          black_box(mesh_results)
        })
      },
    );

    // Strategy C: No presample (batch)
    group.bench_with_input(
      BenchmarkId::new("no_presample/terrain", batch_size),
      &batch_size,
      |b, _| {
        b.iter(|| {
          let outputs: Vec<_> = batch
            .iter()
            .map(|node| pipeline_no_presample(&terrain, node, &octree_config, &mesh_cfg))
            .collect();
          black_box(outputs)
        })
      },
    );
  }

  group.finish();
}

/// Benchmark homogeneous vs surface chunk ratios.
///
/// Tests how each strategy performs with different ratios of
/// homogeneous (skip) vs surface (mesh) chunks.
fn bench_homogeneous_ratio(c: &mut Criterion) {
  let mut group = c.benchmark_group("pipeline/homogeneous_ratio");
  let octree_config = test_config();
  let mesh_cfg = mesh_config();

  // Create nodes at different positions to test different scenarios
  let nodes: Vec<OctreeNode> = (0..8)
    .flat_map(|x| (0..8).flat_map(move |y| (0..8).map(move |z| OctreeNode::new(x, y, z, 0))))
    .take(64)
    .collect();

  // Test with samplers that produce different homogeneous/surface ratios
  // Dense terrain: most chunks below surface are homogeneous solid
  let terrain_dense = TerrainSampler::dense();
  group.bench_function("presample_first/90%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .filter_map(|node| {
          pipeline_presample_first(&terrain_dense, node, &octree_config, &mesh_cfg)
        })
        .collect();
      black_box(outputs)
    })
  });
  group.bench_function("on_demand/90%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .filter_map(|node| pipeline_on_demand(&terrain_dense, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });
  group.bench_function("no_presample/90%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .map(|node| pipeline_no_presample(&terrain_dense, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });

  // Noise worst: almost all chunks have surfaces
  let noise = NoiseSampler::worst_case();
  group.bench_function("presample_first/10%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .filter_map(|node| pipeline_presample_first(&noise, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });
  group.bench_function("on_demand/10%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .filter_map(|node| pipeline_on_demand(&noise, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });
  group.bench_function("no_presample/10%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .map(|node| pipeline_no_presample(&noise, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });

  // Standard terrain: ~50% homogeneous
  let terrain_std = TerrainSampler::standard();
  group.bench_function("presample_first/50%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .filter_map(|node| pipeline_presample_first(&terrain_std, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });
  group.bench_function("on_demand/50%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .filter_map(|node| pipeline_on_demand(&terrain_std, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });
  group.bench_function("no_presample/50%_homogeneous", |b| {
    b.iter(|| {
      let outputs: Vec<_> = nodes
        .iter()
        .map(|node| pipeline_no_presample(&terrain_std, node, &octree_config, &mesh_cfg))
        .collect();
      black_box(outputs)
    })
  });

  group.finish();
}

criterion_group!(
  isolated,
  bench_presample_isolated,
  bench_meshing_isolated,
  bench_sampling_isolated,
);

criterion_group!(
  pipeline,
  bench_pipeline_single_chunk,
  bench_pipeline_batch,
  bench_homogeneous_ratio,
);

criterion_main!(isolated, pipeline);
