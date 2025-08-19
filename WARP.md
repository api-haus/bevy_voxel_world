# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

**bevister** is a Rust project using Cargo as the build system. The project is currently in early development (version 0.1.0) and uses Rust edition 2024.

## Essential Commands

### Building & Running
```bash
# Build the project
cargo build

# Build in release mode (optimized)
cargo build --release

# Run the application
cargo run

# Run with release optimizations
cargo run --release
```

### Testing
```bash
# Run all tests
cargo test

# Run tests with output shown
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Run tests in a specific module
cargo test module_name
```

### Development Tools
```bash
# Check code without building
cargo check

# Format code
cargo fmt

# Lint code (if clippy is installed)
cargo clippy

# Clean build artifacts
cargo clean

# Show project dependencies
cargo tree
```

## Project Structure

This is a minimal Rust binary project with the following structure:

- **`src/main.rs`** - Main entry point and primary application logic
- **`Cargo.toml`** - Project manifest defining dependencies, metadata, and build configuration
- **`.gitignore`** - Excludes `/target` directory (build artifacts)

## Architecture Notes

- **Single Binary**: Currently structured as a simple binary crate with all logic in `main.rs`
- **No Dependencies**: The project has no external dependencies, keeping it lightweight
- **Rust 2024 Edition**: Uses the latest Rust edition for modern language features

## Development Environment

- **Rust Version**: 1.89.0
- **Cargo Version**: 1.89.0
- **Edition**: 2024
- **Target Directory**: All build artifacts are placed in `/target` (gitignored)

## Adding Dependencies

Dependencies should be added to `Cargo.toml` under the `[dependencies]` section:

```toml
[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }
```

Then run `cargo build` to fetch and compile new dependencies.
