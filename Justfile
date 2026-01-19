# Justfile for rust_voxelframework
# Dynamic linking enabled by default for dev builds
# Release and web builds use --no-default-features to disable it

# Default recipe
default:
    @just --list

# Run native dev build (with dynamic linking - fast incremental builds)
dev:
    cargo run -p voxel_game

# Run release build (no dynamic linking)
release:
    cargo run -p voxel_game --release --no-default-features

# Clean web build artifacts
clean-web:
    rm -rf crates/voxel_game/dist crates/voxel_game/.stage

# Serve web build with hot reload (requires trunk, no dynamic linking)
serve: clean-web
    cd crates/voxel_game && trunk serve

# Run all tests
test:
    cargo test --workspace

# Run benchmarks (if any)
bench:
    cargo bench --workspace

# Install web tooling
install-web-tools:
    cargo install trunk
    rustup target add wasm32-unknown-unknown wasm32-unknown-emscripten

# Bake terrain texture arrays (KTX2)
bake-textures:
    cargo run -p texture_baker --release -- --config assets/terrain_textures.toml --assets-dir assets
