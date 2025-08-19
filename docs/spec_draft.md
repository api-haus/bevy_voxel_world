Here’s a compact **first-draft technical specification** you can hand to your team (and iterate on). It bakes in Rust + Bevy, Rayon, fast-surface-nets, mesh\_to\_sdf, and fastnoise2; axis-aligned chunks; SDF f32 + material u8; +1 apron; runtime edits via sphere/box/cone; mesh colliders from the render mesh; per-volume transform.

---

# Voxel Meshing System — First-Draft Technical Spec (Rust / Bevy)

## 0) Overview & Goals

**Goal.** A destructible, grid-based voxel system that:

* Stores voxels as **SDF f32** (separate array) and **material u8** (separate array).
* Extracts renderable meshes via **Surface Nets** (CPU) for many small/medium independent volumes.
* Supports runtime edits via **shape casts** (sphere, box, cone) with **place/destroy** ops.
* Integrates with **Bevy** for rendering/ECS and **avian3d** for colliders (from the render mesh).
* Uses **Rayon** for parallel authoring and remeshing.
* **Authoring** with SDF shapes + blending and **mesh\_to\_sdf** providers; can **bake** to disk.

**Non-Goals (v1).** LOD, streaming, sparse voxel compression, non-uniform scaling of volumes, GPU meshing.

---

## 1) Terminology & Coordinates

* **Volume**: A logical voxel space bound to a Bevy entity; has a world **TRS transform**.
* **Chunk**: A dense, axis-aligned 3D block of voxels in **volume-local** space (no per-chunk rotation).
* **Voxel size**: Constant in **volume-local units** (1 voxel = `voxel_size` meters).
* **Apron**: **+1 voxel/face** in each chunk for borderless meshing and stable neighbor sampling.
* **SDF sign**: negative=inside, positive=outside; **iso** = 0.0. Distances in world/volume units.

---

## 2) Data Model

### 2.1 Volume

```rust
struct Volume {
    id: VolumeId,
    transform_world: Affine3A,      // bevy transform at the attached entity
    voxel_size: f32,                // constant per volume
    chunk_shape: ChunkShape,        // e.g., Cube{n=32} or Slab{nx=64, ny=64, nz=8}
    bounds: Aabb,                   // in volume-local units (integer multiples of voxel_size)
    material_table: Arc<MaterialTable>,
    // runtime
    chunks: FxHashMap<ChunkKey, ChunkHandle>,
}
```

### 2.2 Chunk storage (SoA)

* `sdf[]: f32` (dense)
* `mat[]: u8` (0..=255 material ID; 0 reserved for AIR)
* Layout: **Z-major or X-major**, but consistent across system.
* **With apron**: allocate `(nx+2)*(ny+2)*(nz+2)` cells; interior is `[1..nx] × [1..ny] × [1..nz]`.

Per-chunk bytes (example `32×32×32`):

* SDF: 34³ × 4B ≈ **157 KB**
* MAT: 34³ × 1B ≈ **39 KB**
* Total ≈ **196 KB/chunk** (not including mesh buffers)

*(Adjust for chosen dims.)*

---

## 3) Materials

* **Count**: up to **256**.
* **Schema**:

```rust
struct Material {
    pbr: PbrParams,     // albedo/rough/metal/emissive/… (hook to bevy PBR)
    sturdiness: f32,    // influences authoring/edits if needed
    flags: u8,          // reserved (opaque, emissive, etc.)
}
```

* Authoring overlaps: material chosen from contributor with **smallest |s|** (nearest surface); tie-break by **priority**, else last-writer.

---

## 4) Authoring & Baking

### 4.1 Authoring entities (Bevy ECS)

* **SDF primitives**: Sphere, Box (optionally rounded), Cone; (extensible: Cylinder, Capsule).
* **Mesh SDF provider**: references a triangle mesh + transform (via **mesh\_to\_sdf**).
* **FastNoise2 modifiers**: optional field modifiers (domain warp, masks, FBM).

Each authoring entity has:

```rust
enum AuthorOp { Union, Intersect, Subtract, SmoothUnion{ k:f32 }, SmoothIntersect{ k:f32 }, SmoothSubtract{ k:f32 } }

#[derive(Component)]
struct SdfAuthor {
    shape: SdfShape,               // Sphere{r}, Box{extents,round}, Cone{h,r1,r2}, Mesh{handle}, …
    op: AuthorOp,
    material_id: u8,
    priority: u8,
    local_to_world: Affine3A,      // Bevy transform (usually entity's)
}
```

### 4.2 Composition & sampling

* Evaluate shapes/providers in a **deterministic order** (entity stable sort by priority→entity id).
* **CSG rules**:
  `union(a,b)=min(a,b)`; `inter(a,b)=max(a,b)`; `sub(a,b)=max(a,-b)`; smooth variants use parameter `k`.
* **Sampling point**: voxel centers (authoring may supersample ×4 near iso for quality in bakes).
* **Culling**: per chunk, only process shapes/providers whose **AABB expanded by apron** intersects the chunk AABB.

### 4.3 Mesh → SDF

* Use **mesh\_to\_sdf** on chunk-aligned grids; prefer **narrow band** (±N voxels) if available; else clamp far field.
* Non-manifold/open meshes: flood-fill or winding fallback (define one consistent policy).

### 4.4 Baking

* **Format** (per chunk file `.vxb` in a per-volume directory):

  * Header: `{version:u32, voxel_size:f32, dims:u16[3], apron:u8, sdf_type='f32', mat_bits=8, crc32:u32}`
  * Body: `LZ4( [sdf_f32 raw], [mat_u8 raw] )`
* **Load policy**: if bake exists → load; else author at startup and (dev builds) write bake.
* **Determinism**: authoring must be reproducible given identical inputs.

---

## 5) Editing (Runtime Shape Casts)

### 5.1 Operations

* **place**: `s_new = min(s_old, s_brush)`; if sign flips to negative, set `mat = brush_mat`.
* **destroy**: `s_new = max(s_old, -s_brush)`; if sign flips to positive, set `mat = AIR_ID(0)`.

### 5.2 Shapes & queries

* Shapes: **Sphere**, **Box**, **Cone** (world-space).
* For each **volume**: transform the shape into **volume-local** via `inv(volume.transform_world)`.
* Compute affected chunk keys via shape’s **AABB** (expanded by brush radius and **apron**); mark as **dirty**.

### 5.3 Dirty regions & throttling

* Each dirty chunk records a **voxel AABB** of edits (for potential partial meshing later; v1 can remesh whole chunk).
* **Remesh queue** (lock-free MPSC).
* **Throttling**: cap **N meshed chunks/frame** or **time-slice ≤ 2 ms CPU/frame** (whichever comes first), priority by **camera distance** (on-screen first).

---

## 6) Meshing (Surface Nets on CPU)

### 6.1 Inputs

* Chunk SDF (`f32`) with apron; iso = 0; neighbor reads allowed into apron cells.

### 6.2 Algorithm notes (Surface Nets)

* For each active cell (sign change among corners), compute vertex via linear interpolation of SDF (Surface Nets’ central position) and output quads turned into triangles.
* **Empty/solid fast paths**: skip if all-<0 or all->0 (including apron checks).
* **Vertex welding**: inherent to Surface Nets (one vertex per cell); ensure consistent indexing order.

### 6.3 Normals / UVs

* **Normals**: central differences on SDF (sample gradient at vertex position; tri-linear within cell).
* **UVs/texturing**: default to **triplanar** in shader; UV/tangents optional in v1.

### 6.4 Output

```rust
struct MeshBuffers {
    positions: Vec<Vec3>,
    normals:   Vec<Vec3>,
    indices:   Vec<u32>,
    // optional: tangents, material_ids per-vertex if needed
}
```

---

## 7) Bevy Integration

### 7.1 ECS components

```rust
#[derive(Component)] struct VolumeRef(VolumeId);
#[derive(Component)] struct ChunkRef { volume: VolumeId, key: ChunkKey }
#[derive(Component)] struct VoxelChunk { sdf: Box<[f32]>, mat: Box<[u8]>, dims: UVec3 }  // includes apron
#[derive(Component)] struct Dirty;                   // tag for chunks needing remesh
#[derive(Resource)] struct RemeshQueue { /* MPSC */ }
#[derive(Component)] struct RenderMeshHandle(Handle<Mesh>);
#[derive(Component)] struct RenderMaterialHandle(Handle<StandardMaterial>);
```

### 7.2 Systems (order)

1. **Authoring/Load** (startup): load bake or author → populate chunks.
2. **EditApply**: apply shape casts, mark Dirty, enqueue remesh jobs.
3. **RemeshDispatch** (parallel via Rayon): pop from queue (budgeted), run Surface Nets.
4. **MeshUpload**: create/update `bevy::render::mesh::Mesh` and material handles.
5. **ColliderUpdate**: regenerate **mesh collider** from the render mesh (**avian3d**), **debounced 1 frame**.
6. **Cleanup/GC** (optional): recycle scratch buffers.

---

## 8) Physics (avian3d)

* **Collider generation**: from render mesh (triangle soup).
* **Update cadence**: on successful remesh; debounce 1 frame to avoid thrash under bursts.
* **Broadphase hints**: maintain a per-chunk AABB (volume-local → world via volume transform).

---

## 9) Concurrency & Scheduling (Rayon)

* **Authoring**: per-chunk voxelization in `par_iter()`, culling shapes/providers by chunk AABB.
* **Remeshing**: parallel per chunk, limited by **frame budget** (≤ 2 ms) and **max\_in\_flight**.
* **Scratch arenas**: thread-local buffers for SDF windows and meshing temporaries to reduce allocs.

---

## 10) API Surface (Rust)

### 10.1 Creation / loading

```rust
fn create_volume(params: VolumeParams) -> VolumeId;
fn load_or_author_and_bake(volume: VolumeId, author_ctx: &AuthorCtx) -> Result<()>;
```

### 10.2 Editing

```rust
enum Brush { Sphere{r:f32}, Box{extents:Vec3}, Cone{h:f32, r1:f32, r2:f32} }
enum Op { Place{mat:u8}, Destroy }

fn apply_brush(volume: VolumeId, brush_world: Brush, op: Op);
```

### 10.3 Queries / utilities

```rust
fn query_sdf(volume: VolumeId, p_world: Vec3) -> f32;          // transforms to volume-local
fn chunk_key_from_point(volume: VolumeId, p_local: Vec3) -> ChunkKey;
```

---

## 11) Performance Targets & Budgets (initial)

* **Frame rate**: 60/120 FPS targets.
* **Remesh budget**: **≤ 2.0 ms CPU/frame** total (parallel), hard cap **N=8** chunks/frame (tunable).
* **Edit latency** (brush→visible): **≤ 33 ms** typical, **≤ 100 ms** in bursts.
* **Collider freshness**: **≤ 66 ms** typical after mesh update.
* **Memory**: target ≤ **1.0–1.5 GB** total for 300–500 active `32³` chunks including meshes (tune with real scenes).

---

## 12) Chunk Boundaries & Cracks

* **Apron** (+1) guarantees cross-border sampling; chunks **overlap at surface** by design (Surface Nets stable).
* **Authority**: interior voxels `[1..n]` are owned by the chunk; apron cells are read-only from neighbors or duplicated during authoring.
* **After edits crossing borders**: mark **both** chunks dirty; meshing uses each chunk’s apron to stitch seamlessly.

---

## 13) Diagnostics & Tooling (v1 minimal)

* Counters: chunks edited/meshed/frame, triangles/frame, queue lengths, mesh time p50/p95.
* Debug views: show chunk bounds, apron cells (toggle), dirty regions.
* Golden scenes: fixed random seed authoring; image diff on output meshes.

---

## 14) Risks & Future Work

* **Non-uniform scale**: not supported in v1 (distance invariance breaks); future: handle via metric scaling/dual grids.
* **Streaming/compression**: consider zstd bakes + sparse chunk activation.
* **GPU meshing**: future (compute Surface Nets).
* **Partial remesh**: sub-chunk windows to cut latency for small edits.
* **Material-aware meshing**: per-material sharpness/skirt.

---

## 15) Pseudocode (key paths)

### 15.1 Authoring (startup / bake)

```rust
fn author_volume(vol: &Volume, authors: &[SdfAuthor]) -> Chunks {
    let keys = enumerate_chunk_keys(vol.bounds, vol.chunk_shape);
    keys.par_iter().map(|key| {
        let aabb_local = chunk_aabb_local(*key, vol.chunk_shape, vol.voxel_size, apron=true);
        // gather contributors overlapping chunk
        let contributors = authors.iter()
            .filter(|a| overlaps(a.world_aabb(), aabb_local.transformed(vol.transform_world)))
            .collect::<Vec<_>>();
        // allocate arrays with apron
        let mut sdf = alloc_f32_with_apron(vol.chunk_shape);
        let mut mat = alloc_u8_with_apron(vol.chunk_shape);
        // sample voxel centers
        for v in voxels_in_chunk_with_apron(vol.chunk_shape) {
            let p_local = voxel_center_local(v, vol.voxel_size);
            let p_world = vol.transform_world.transform_point3(p_local);
            // compose SDF
            let (s, m) = compose_sdf_at(p_world, &contributors);
            sdf[v] = s; mat[v] = m;
        }
        Chunk { key: *key, sdf, mat }
    }).collect()
}
```

`compose_sdf_at` applies CSG ops in order; material chosen by smallest `|s|` with priority tie-break.

### 15.2 Edit apply (runtime)

```rust
fn apply_brush(volume: &mut Volume, brush_world: Brush, op: Op) {
    for (key, aabb) in affected_chunk_keys(volume, &brush_world) {
        let chunk = volume.chunks.get_mut(&key).unwrap();
        for v in interior_voxels(chunk) {
            let p_local = voxel_center_local(v, volume.voxel_size);
            let p_world = volume.transform_world.transform_point3(p_local);
            let s_brush = eval_brush_sdf_world(&brush_world, p_world);
            match op {
                Op::Place{mat: m} => {
                    let s0 = chunk.sdf[v];
                    let s1 = s0.min(s_brush);
                    if s0 >= 0.0 && s1 < 0.0 { chunk.mat[v] = m; }
                    chunk.sdf[v] = s1;
                }
                Op::Destroy => {
                    let s0 = chunk.sdf[v];
                    let s1 = s0.max(-s_brush);
                    if s0 < 0.0 && s1 >= 0.0 { chunk.mat[v] = AIR_ID; }
                    chunk.sdf[v] = s1;
                }
            }
        }
        mark_dirty_and_enqueue(key);
    }
}
```

### 15.3 Remesh (Rayon worker)

```rust
fn remesh_chunk(chunk: &VoxelChunk) -> MeshBuffers {
    if is_uniform_sign(chunk.sdf) { return MeshBuffers::empty(); }
    surface_nets_extract(chunk, iso=0.0)  // using fast-surface-nets
}
```

### 15.4 Mesh upload & collider

```rust
fn upload_mesh(cmds: &mut Commands, entity: Entity, buffers: MeshBuffers) {
    let mesh_handle = meshes.add(buffers.into_bevy_mesh());
    cmds.entity(entity).insert(RenderMeshHandle(mesh_handle));
    // collider from render mesh (avian3d)
    schedule_collider_update(entity, mesh_handle);
}
```

---

## 16) Dependency Guidance (Crates)

* **bevy** — engine/ECS/rendering; primary runtime and asset handling.
* **rayon** — thread-pool & `par_iter` for authoring/remeshing.
* **fast-surface-nets** — CPU Surface Nets extraction for regular grids.
* **mesh\_to\_sdf** — triangle mesh → SDF sampling for authoring.
* **fastnoise2** — noise graphs for terrain modulation/masks during authoring.

**Cargo baseline (pin exact versions later):**

```toml
[dependencies]
bevy = "0.16"
rayon = "1"
fast-surface-nets = "0.2"
mesh_to_sdf = "0.4"
fastnoise2 = "0.3"
```

---

## 17) Acceptance Criteria (initial; measure & tune)

* Crack-free across chunk borders with +1 apron; no visible seams in standard test scenes.
* Deterministic meshes for fixed inputs (same OS/CPU).
* At 60/120 FPS scenes: **≤ 2 ms CPU/frame** for remeshing (typical), **≤ 33 ms** edit-to-visible latency (typical).
* Collider refresh **≤ 66 ms** after mesh update (debounced).
* Memory fit within project limits; telemetry proves triangle counts & budgets are met.

---

### Open Decisions to Confirm

* Final **allowed chunk sizes** (recommend: `16³`, `32³`, `64×64×8` for slabs).
* **Narrow band** width for mesh\_to\_sdf (e.g., ±3 voxels).
* Smooth blend default `k` (recommend `1.5 * voxel_size`).
* Exact **remesh cap** (`N` chunks/frame) and/or time-slice budget.
* Policy for open meshes in mesh\_to\_sdf (flood-fill vs winding).

---

If you want, I can now tailor this to your exact chunk sizes and budgets (or sketch a Bevy system schedule and minimal module layout to start coding).
