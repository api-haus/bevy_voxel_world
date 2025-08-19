# Agent Prompt — Implement the Voxel Meshing System (Rust + Bevy)

**Role:** You are a senior Rust/Bevy implementation agent. Your job is to **ship working code**, tests, and minimal docs for a grid-based, destructible voxel meshing system.

**Audience:** Rust engine programmers.

**Output format (very important):**

* Keep explanations brief; prioritize compiling code.

---

## 1) System you must implement (fixed requirements)

* **Data model:** Dense voxels per chunk, **SoA** arrays: `sdf[]: f32` and `mat[]: u8` (0=air).
* **SDF semantics:** negative=inside, positive=outside, **iso=0.0**, distances in world units.
* **Chunks:** axis-aligned in **volume-local space**; interior size `N×N×N`, allocated with **+1 voxel apron** on each face → arrays `(N+2)^3`.
* **Transforms:** each **volume** is attached to a Bevy entity; volume has a world **TRS**; chunks are addressed in volume-local; queries/edits transform world↔volume.
* **Meshing:** **Surface Nets** (CPU) using `fast-surface-nets`. Normals from SDF central differences.
* **Editing:** runtime shape casts (**Sphere**, **Box**, **Cone**) with ops **Place** and **Destroy**:

    * Place: `s_new = min(s_old, s_brush)`; if sign flips to negative → set `mat = brush_mat`.
    * Destroy: `s_new = max(s_old, -s_brush)`; if sign flips to positive → set `mat = AIR`.
* **Authoring:** SDF primitives + blend ops (hard & smooth) **and** mesh SDF from `mesh_to_sdf`. Evaluate at load to produce initial voxels; optionally **bake to disk**; use baked data on next run.
* **Rendering:** Bevy PBR; triplanar acceptable; **mesh colliders** are derived **from the render mesh** (avian3d).
* **Concurrency:** **Rayon** for authoring + remeshing; cap per-frame work (time-slice).
* **No LOD** in v1; RAM-only at runtime.

### External crates to use

* `bevy` — engine/ECS/rendering & assets.
* `rayon` — parallel per-chunk authoring/remesh.
* `fast-surface-nets` — fast CPU Surface Nets for regular grids.
* `mesh_to_sdf` — triangle mesh → SDF sampling for authoring.
* `fastnoise2` — procedural noise graphs for SDF modulation during authoring.

Pin reasonable versions; expose a single Cargo workspace.

---

## 2) Deliverables & structure

Create a Cargo **workspace**:

```
/voxel/                (workspace root)
  Cargo.toml
  rust-toolchain.toml  (optional; stable)
  /voxel-core/         (data model, math, indexing, SDF ops, apron helpers)
  /voxel-author/       (authoring: SDF shapes, CSG, mesh_to_sdf, baking I/O)
  /voxel-mesh/         (Surface Nets integration + normals, buffers)
  /voxel-bevy/         (Bevy plugin: ECS comps, systems, queues, uploads)
  /voxel-demo/         (example Bevy app with a tiny scene & tests)
```

**You must provide:**

* Compiling code with minimal docs (`README.md` per crate).
* Unit tests for indexing/apron, edit ops, and CSG composition.
* An integration test or demo scene that spawns 1–2 volumes, authoring content, and allows edits.
* Basic telemetry (counters for meshed chunks, triangles, timings).
* A minimal bake format (header + LZ4 body) and loader.

---

## 3) Milestone plan (execute in order)

### M1 — Workspace bootstrap

* Create workspace and crates, add dependencies, set up `clippy`/`rustfmt`.
* Define common `types.rs`: `VolumeId`, `ChunkKey`, `UVec3`, `Aabb`, etc.
* Implement **indexing** with apron: `(x,y,z) → idx`, interior span `1..=N`.

**Definition of Done (DoD):** `cargo test` passes for indexing; `cargo run -p voxel-demo` creates a window.

---

### M2 — Core data & SDF ops (voxel-core)

* `ChunkDims { nx, ny, nz }`, `Apron { extent: 1 }`.
* `VoxelChunk` storing `Box<[f32]>` and `Box<[u8]>` with apron; constructors & views for interior/apron.
* SDF utilities: `union`, `intersect`, `subtract`, smooth variants with `k`.
* Brush SDFs: sphere, box, cone (world & volume-local eval).
* Material write rules (nearest |s| for authoring; brush rules for editing).

**DoD:** unit tests for SDF ops and brush semantics.

---

### M3 — Authoring & baking (voxel-author)

* ECS-agnostic author graph: `AuthorNode { shape|mesh, op, material_id, priority, transform }`.
* Chunk culling by **AABB expanded by apron**.
* Mesh SDF via `mesh_to_sdf` (narrow band if available; else clamp).
* `author_volume(...) → HashMap<ChunkKey, VoxelChunk>`.
* **Bake**: `.vxb` per chunk (header: version, voxel\_size, dims, apron, crc; body: LZ4 of `[sdf_f32][mat_u8]`).
* **Load** baked or fallback to authoring; deterministic ordering.

**DoD:** author a small scene; write/read bake; golden test compares hashes.

---

### M4 — Meshing (voxel-mesh)

* Wrap `fast-surface-nets` with `extract_surface_nets(chunk, iso=0.0) → MeshBuffers`.
* Skip empty/solid quickly; compute normals via central differences (tri-linear samples).
* Provide conversion to Bevy `Mesh`.

**DoD:** unit test on synthetic SDF (sphere) yields reasonable tri count and normal directions.

---

### M5 — Bevy plugin & systems (voxel-bevy)

* Components: `Volume`, `ChunkRef`, `VoxelChunk`, `Dirty`, `RenderMeshHandle`, etc.
* Resources/queues: `RemeshQueue` (MPSC), budgets (time-slice ms, max per frame).
* Systems:

    1. **Startup**: load bakes or author volumes.
    2. **EditApply**: apply brushes, mark `Dirty`, enqueue chunk keys.
    3. **RemeshDispatch** (Rayon): drain queue within budget, produce meshes.
    4. **MeshUpload**: update Bevy mesh/material handles.
    5. **ColliderUpdate**: regenerate collider from render mesh (debounced 1 frame).
    6. Telemetry HUD (optional).

**DoD:** demo lets you press keys/click to place/destroy and see meshes update; colliders follow.

---

### M6 — Demo & tests (voxel-demo)

* Spawn 1–2 volumes (e.g., `32³` cubes, a `64×64×8` slab).
* Authoring: a few shapes + one mesh-to-SDF object; bake, reload.
* Input: left-click destroy sphere; right-click place sphere with selected material; number keys switch brush (sphere/box/cone).
* Perf counters printed each second.

**DoD:** runs at interactive framerate; edit-to-visible latency looks ≤ \~33ms typical on desktop.

---

## 4) Performance & acceptance

* **Budgets:** time-slice remeshing to **≤ 2 ms CPU/frame**; cap N chunks/frame (start with 8).
* **Crack-free:** apron mirrors neighbors; both sides remesh after cross-border edits.
* **Determinism:** same inputs → same meshes on the same platform (document any float instabilities).
* **Telemetry:** p50/p95 mesh times, triangles/frame, dirty queue depth.

---

## 5) Coding conventions

* Rust 2021+, no panics on user input paths.
* Separate pure logic (voxel-core/author/mesh) from Bevy integration.
* Zero UB; keep `unsafe` minimal and documented.
* Use `#[cfg(feature="dev")]` for debug HUD & hot-reload.
* Small, focused modules; top-level docs with diagrams where useful.

---

## 6) Initial API sketches (you should implement)

```rust
// voxel-core
pub struct VoxelChunk { /* sdf, mat, dims_with_apron, ... */ }
pub fn idx_with_apron(x:u32,y:u32,z:u32,d:Dims)->usize;
pub fn central_gradient(sample: impl Fn(Vec3)->f32, p: Vec3, h:f32)->Vec3;

// voxel-author
pub enum AuthorShape { Sphere{r:f32}, Box{extents:Vec3, round:f32}, Cone{h:f32,r1:f32,r2:f32}, Mesh{mesh:MeshHandle} }
pub enum AuthorOp { Union, Intersect, Subtract, SmoothUnion{ k:f32 }, SmoothIntersect{ k:f32 }, SmoothSubtract{ k:f32 } }
pub struct AuthorNode { shape:AuthorShape, op:AuthorOp, mat:u8, priority:u8, xform:Affine3A }
pub fn author_volume(desc:&VolumeDesc, nodes:&[AuthorNode]) -> HashMap<ChunkKey, VoxelChunk>;
pub fn bake_write(volume:&VolumeDesc, chunks:&HashMap<ChunkKey,VoxelChunk>) -> Result<()>;
pub fn bake_read(volume:&VolumeDesc) -> Option<HashMap<ChunkKey,VoxelChunk>>;

// voxel-mesh
pub struct MeshBuffers { pub positions:Vec<Vec3>, pub normals:Vec<Vec3>, pub indices:Vec<u32> }
pub fn extract_surface_nets(chunk:&VoxelChunk, iso:f32)->Option<MeshBuffers>;

// voxel-bevy
pub struct VoxelPlugin;
pub fn apply_brush(world: &mut World, volume: Entity, brush: Brush, op: Op);
```

---

## 7) What to do next (your first actions)

1. **Create workspace & crates**; add `bevy`, `rayon`, `fast-surface-nets`, `mesh_to_sdf`, `fastnoise2`, `lz4_flex` (for baking).
2. Implement indexing + apron in `voxel-core` with tests.
3. Scaffold `VoxelPlugin` and open a window in `voxel-demo`.

Return your first patch set now.
