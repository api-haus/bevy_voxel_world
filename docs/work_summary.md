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

- Dispatch extensions & tests:
  - Extended `remesh_chunk_dispatch` to support 32^3 core (34^3 sample) in addition to 16^3.
  - Added unit tests for dispatch (supported vs unsupported dims) and a single-bucket mesh conversion sanity check.

- Scheduler/Apply telemetry:
  - Added `VoxelTelemetry { total_meshed, meshed_this_frame, queue_len }` resource.
  - Wired `update_telemetry_begin` to reset per-frame counters and sample queue length.
  - Increment counters in `apply_remeshes` when meshes are applied.
  - Stubbed `VoxelEditEvent` handling to enqueue affected chunks (all chunks for now), to evolve later to precise region mapping.
  - Gated debug SDF seeding behind `cfg!(debug_assertions)` to keep release builds clean; future switch to a `dev` Cargo feature possible.

- Scheduler & apply (minimal Phase 4 stub):
- Introduced `RemeshBudget { max_chunks_per_frame, time_slice_ms }`, a global `RemeshQueue`, and a background Rayon job that runs Surface Nets and sends a `RemeshReady` event.
- Startup seeding still writes the debug sphere SDF, but now enqueues chunk entities into the remesh queue instead of meshing immediately.
- Main-thread systems pump results and apply meshes to chunk entities. Basic positioning and a shared `StandardMaterial` are used. Counters/telemetry to be expanded later.

- How to run/verify:
- `cargo test` should be green, including new meshing tests and an ECS spawn test.
- `cargo run --features bevy/dynamic_linking` will show the debug sphere via the new queue+apply path. Meshes appear after background jobs complete.

- Additional verify hints:
  - Run meshing tests directly: `cargo test --lib meshing::surface_nets::tests`.
  - Check telemetry updates by running the app; counters are maintained internally (exposed publicly in a later step).

- Camera & controls:
- Replaced previous demo camera/player with a lightweight fly camera (`src/fly_cam.rs`) wired in `src/main.rs`.
- Mouse-look when RMB is held; WASD + Space/Shift for movement; Ctrl for speed boost. Adds a directional light.



 - Demo content & seeding:
   - Switched the demo to a `16×16×16` chunk grid (`VoxelVolumeDesc::default().grid_dims`).
   - Startup SDF is now a field of random spheres distributed across the whole volume, generated in parallel with AABB culling (Rayon) and then applied to each chunk’s `VoxelStorage`.

 - Editing & input:
   - Introduced `EditOp::{Destroy, Place}` and extended `VoxelEditEvent { center_world, radius, op }`.
   - Added sphere-cast edit application that updates SDF including apron and adjusts material on sign transitions; enqueues chunks for remesh.
   - `src/fly_cam.rs` actions: E = dig (Destroy), R = place (Place), F = spawn physics ball. Dig/place use Avian3D `SpatialQuery` raycast from viewport center (max 100 units).

 - Physics colliders:
   - On mesh apply, build/replace a static `Collider::trimesh_from_mesh` (Avian3D) for each chunk alongside the render `Mesh3d` (no debounce).

 - Rendering/culling:
   - Ensure render bounds update on remesh by computing the mesh AABB before inserting the mesh asset (`MeshAabb::compute_aabb`), keeping frustum culling correct as geometry changes.

 - How to run/verify (updated):
   - `cargo test` remains green, including meshing tests.
   - `cargo run --features bevy/dynamic_linking` spawns the `16×16×16` volume seeded with random spheres. Use E/R to dig/place at the crosshair and F to spawn a ball; meshes and colliders update immediately and cull correctly.

- Module refactor (Step 1):
  - Extracted telemetry into `src/plugin/telemetry.rs` (`VoxelTelemetry`, `update_telemetry_begin`).
  - Updated `src/plugin/mod.rs` to use and re-export telemetry; behavior unchanged.

- Module refactor (Steps 2–7):
  - Extracted scheduler to `src/plugin/scheduler.rs` (`RemeshBudget`, `RemeshQueue`, `RemeshResultChannel`, `drain_queue_and_spawn_jobs`, `pump_remesh_results`).
  - Extracted mesh application and collider build to `src/plugin/apply_mesh.rs`; preserved render AABB update and collider behavior.
  - Extracted triplanar material setup to `src/plugin/materials.rs` (`TriplanarExtension`, `VoxelRenderMaterial`, `setup_voxel_material`); re-exported `TriplanarExtension` publicly for `main.rs` usage.
  - Extracted editing to `src/plugin/editing.rs` (`EditOp`, `VoxelEditEvent`, `apply_edit_events`); re-exported types via `plugin`.
  - Moved `buffer_to_meshes_per_material` to `src/meshing/bevy_mesh.rs`; updated imports.
  - Extracted volume/chunk spawn to `src/plugin/volume_spawn.rs` and wired in startup.
  - Extracted random-spheres seeding to `src/authoring/seed.rs`; wired through `plugin` startup; factored `sample_min` as `pub(crate)` and used the `RemeshQueue` re-export.

- Meshing telemetry and Perf UI (new):
  - Added tracing spans in `scheduler.rs` (queue drain, job spawn, early skip, fsn run) and `apply_mesh.rs` (apply start, AABB compute, collider build). Uses Bevy’s re-exported tracing macros for Tracy/Chrome support [`bevy profiling docs`](https://github.com/bevyengine/bevy/blob/main/docs/profiling.md#tracy-renderqueue).
  - Extended `VoxelTelemetry` with per-frame accumulators: `mesh_time_ms_frame`, `apply_time_ms_frame`, `jobs_spawned_frame`, `jobs_completed_frame`. Reset in `update_telemetry_begin`.
  - Registered custom diagnostics and publish each frame: `vox.queue_len`, `vox.meshed_this_frame`, `vox.total_meshed`, `vox.jobs_spawned`, `vox.jobs_completed`, `vox.mesh_time_ms`, `vox.apply_time_ms`. `iyes_perf_ui` picks these up automatically (see minimal example for custom diagnostics publishing: [`iyes_perf_ui custom_minimal.rs`](https://github.com/IyesGames/iyes_perf_ui/blob/main/examples/custom_minimal.rs)).
  - No behavioral changes to meshing or colliders.
