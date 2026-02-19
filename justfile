# Voxel Framework build commands

# Build and deploy voxel_unity plugin to Unity
# Metrics enabled by default - use voxel_world_get_metrics() to retrieve timing stats
unity-plugin:
    cargo build -p voxel_unity --release
    mkdir -p ../Packages/im.pala.voxelmission/Plugins/x86_64
    cp target/release/libvoxel_unity.so ../Packages/im.pala.voxelmission/Plugins/x86_64/
    @echo "Deployed libvoxel_unity.so to Unity Plugins (with metrics)"

# Build all crates
build-all:
    cargo build --workspace --release

# Run all tests
test-all:
    cargo test --workspace

# Run all benchmarks
bench-all:
    cargo bench --workspace

# Bake terrain texture arrays (KTX2)
bake-textures:
    cargo run -p texture_baker --release -- --config assets/terrain_textures.toml --assets-dir assets

# Install web tooling
install-web-tools:
    cargo install trunk
    rustup target add wasm32-unknown-unknown wasm32-unknown-emscripten

# Clean web build artifacts
clean-web:
    rm -rf crates/voxel_game/dist crates/voxel_game/.stage

# Serve web build with hot reload (requires trunk, no dynamic linking)
# Uses --no-default-features to disable dev feature (dynamic linking not supported on WASM)
serve: clean-web
    cd crates/voxel_game && trunk serve --no-default-features

# Serve optimized web build (release mode)
serve-release: clean-web
    cd crates/voxel_game && trunk serve --release --no-default-features

# Run native dev build (with dynamic linking - fast incremental builds)
dev:
    cargo run -p voxel_game --features dev

# Run release build (no dynamic linking)
release:
    cargo run -p voxel_game --release --no-default-features

# Run release build (no dynamic linking)
release_trace:
    cargo run -p voxel_game --release --no-default-features --features metrics,trace_tracy

# Default recipe
default:
    @just --list
