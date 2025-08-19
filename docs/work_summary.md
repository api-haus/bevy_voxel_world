## Work Summary

- Implemented Phase 0 core primitives per implementation plan.
- Added `src/core/grid.rs` with `ChunkDims`, `core_extent`, and `sample_extent` plus unit tests.
- Added `src/core/index.rs` with `linear_index` and `delinearize` plus unit tests covering edges and round-trip.
- Exposed modules via `src/core/mod.rs` and `src/lib.rs`.
- No changes to runtime systems yet; existing Bevy demo remains intact.

- Implemented Phase 1 voxel storage module.
- Added `src/voxels/storage.rs` with `VoxelStorage`, `AIR_ID`, `fill_default`, and mutation accessors; includes unit tests.
- Exposed via `src/voxels/mod.rs` and re-exported in `src/lib.rs`.

- Implemented initial Meshing wrapper.
- Added `src/meshing/surface_nets.rs` with `remesh_chunk_fixed::<N>` using `fast-surface-nets`, early empty/solid skip, and unit tests.
- Exposed via `src/meshing/mod.rs` and re-exported in `src/lib.rs`.

- Bevy plugin scaffolding:
- Added `src/plugin/mod.rs` with `VoxelPlugin`, components (`VoxelVolume`, `VoxelChunk`), events, sets, and startup system `spawn_volume_chunks` that creates a volume and grid of chunk children with `VoxelStorage` attached.
- Included an ECS unit test to verify entities are spawned.
- Exported module via `src/lib.rs`.

- Meshing improvements:
- Added a dispatch function for supported sizes (currently 16^3 core → 18^3 sample).
- Implemented `central_gradient` helper for normal computation parity.
- Added `buffer_to_meshes_per_material` skeleton (single-material path for now).
- Extended unit tests: empty/solid skip, simple interface, gradient sanity.

- Scheduler & apply (minimal Phase 4 stub):
- Introduced `RemeshBudget { max_chunks_per_frame, time_slice_ms }`, a global `RemeshQueue`, and a background Rayon job that runs Surface Nets and sends a `RemeshReady` event.
- Startup seeding still writes the debug sphere SDF, but now enqueues chunk entities into the remesh queue instead of meshing immediately.
- Main-thread systems pump results and apply meshes to chunk entities. Basic positioning and a shared `StandardMaterial` are used. Counters/telemetry to be expanded later.

- How to run/verify:
- `cargo test` should be green, including new meshing tests and an ECS spawn test.
- `cargo run --features bevy/dynamic_linking` will show the debug sphere via the new queue+apply path. Meshes appear after background jobs complete.

- Camera & controls:
- Replaced previous demo camera/player with a lightweight fly camera (`src/fly_cam.rs`) wired in `src/main.rs`.
- Mouse-look when RMB is held; WASD + Space/Shift for movement; Ctrl for speed boost. Adds a directional light.


