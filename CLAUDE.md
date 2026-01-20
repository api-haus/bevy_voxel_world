# Rust Voxel Framework

## Asset Locations

**DO NOT put shaders or assets in root `assets/` folder.**

- **Shaders**: Embed in crate source using `embedded_asset!` macro
  - Location: `crates/<crate>/src/shaders/*.wgsl`
  - Avoids WASM asset loading issues with meta files
- **Runtime assets** (textures, models): Place in `crates/voxel_game/assets/`
  - These get copied to dist by trunk via `index.html`
  - Create `.meta` files for each asset to avoid WASM deserialization errors

## WASM Limitations

- **WebGL2** (default): No storage buffers in fragment shaders, no compute shaders
- **WebGPU** (opt-in via `--features webgpu`): Full feature support but limited browser compatibility
- Atmosphere/volumetric effects require WebGPU or are disabled on WebGL2

## Build Commands

```bash
just serve          # Dev build, WebGL2, no optimizations
just serve-release  # Release build, WebGL2, optimized
just dev            # Native with dynamic linking (fastest iteration)
```

## Crate Architecture

### Dependency Graph
```
voxel_game (demo app)
    └─> voxel_bevy
            └─> voxel_plugin (core)
                    └─> voxel_noise (native only)

voxel_unity (FFI bridge)
    └─> voxel_plugin

texture_baker (standalone tool)
```

### Core Layer

#### voxel_plugin
Engine-agnostic voxel meshing library.

**Responsibilities:**
- Surface Nets meshing (SDF → triangle mesh)
- Implicit octree LOD with viewer-distance refinement
- 5-stage async pipeline (refinement → presample → mesh → compose → present)
- Platform-agnostic noise sampling

**Do:**
- Keep engine-agnostic (no bevy, no unity deps)
- Use rayon for parallelism
- Use web-time for timing (WASM-compatible)

**Don't:**
- Add rendering code
- Use std::thread directly
- Assume native-only features

#### voxel_noise
FastNoise2 wrapper for native + WASM.

**Responsibilities:**
- Rust FFI to FastNoise2 C++
- WASM bridge via Emscripten module
- Uniform API across platforms

**Do:**
- Test edge coherency between adjacent chunks
- Use preset strings for reproducible noise

**Don't:**
- Call from WASM without building the Emscripten module first

### Integration Layer

#### voxel_bevy
Reusable Bevy bridge for voxel worlds.

**Responsibilities:**
- LOD chunk entity management
- Material type definitions and bind group layouts
- Fly camera controls
- Debug UI overlay
- EntityQueue for rate-limited spawning

**Do:**
- Keep shader-agnostic (consuming games provide shaders)
- Define material bindings, let games customize appearance
- Use EntityQueue for spawning chunks
- Gate debug features behind `metrics` + `debug_ui`

**Don't:**
- Embed or hardcode shader paths
- Assume specific texture formats or paths
- Bypass voxel_plugin for meshing logic
- Add game-specific rendering code

#### voxel_unity
Unity FFI bridge (cdylib).

**Responsibilities:**
- C ABI for Unity interop
- Rust-driven LOD orchestration
- Presentation events (spawn/despawn hints)

**Do:**
- Version FFI functions (voxel_version returns 0x000300)
- Pre-calculate world positions in Rust
- Maintain backward compat for v0.2 API

**Don't:**
- Let Unity drive LOD decisions
- Expose internal Rust types without #[repr(C)]

### Applications

#### voxel_game
Bevy demo application.

**Responsibilities:**
- Showcase voxel terrain with LOD
- Provide shaders and textures for rendering
- WASM + native entry points
- Performance overlays

**Do:**
- Use `dev` feature for fast iteration (dynamic linking)
- Place shaders in `crates/voxel_game/assets/shaders/`
- Place textures in `crates/voxel_game/assets/textures/`

**Don't:**
- Add reusable logic here (put it in voxel_bevy or voxel_plugin)

#### texture_baker
CLI tool for terrain texture packing.

**Responsibilities:**
- Pack terrain textures into KTX2 arrays
- Channel packing: diffuse+height, normal, material

**Do:**
- Run from CLI with TOML config
- Output to crates/voxel_game/assets/

**Don't:**
- Modify at runtime (build-time only tool)
