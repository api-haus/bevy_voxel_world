# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-01-14

### Added
- GitHub Pages deployment workflow with Emscripten + Trunk
- PolyForm Noncommercial 1.0.0 license
- Per-worker FastNoise2 via JS bridge for WASM builds
- Async mesh processing pipeline with ghost cell prevention
- Presample stage and pipeline architecture
- LOD seam handling with enhanced documentation
- egui performance UI and voxel_bevy crate scaffolding
- Surface nets optimization with direct edge iteration and SIMD

### Fixed
- FastNoise2 SIMD level and node lookup pointer passing
- Precision issues in mesh generation

### Changed
- Extracted tests to separate `_test.rs` files
