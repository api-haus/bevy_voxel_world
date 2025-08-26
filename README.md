## Bevister

### Overview
Bevister is a voxel experimentation workspace built around Bevy. It provides:
- A reusable voxel engine plugin for Bevy (`bevy_voxel_plugin`)
- A runnable demo application (`voxel_demo_app`)
- iOS integration via Xcode/Metal (`VoxelGame`) and helper scripts

The repository emphasizes feature-scoped Bevy plugins and a domain-first design to keep engine logic clean, testable, and portable across desktop and iOS.

### Repository layout
- **`Cargo.toml`**: Workspace manifest for all crates
- **`rust-toolchain.toml`**: Pinned Rust toolchain
- **`Taskfile.yml`**: Common tasks for building/running/testing

- **`crates/asset_prep/`**: Small utility for preparing assets (binary crate)
- **`crates/bevy_voxel_plugin/`**: Bevy voxel engine/plugin (library crate)
- **`crates/voxel_demo_app/`**: Demo Bevy app showcasing the voxel plugin (binary crate)
  - `assets/`: Textures, shaders, and other runtime assets

- **`docs/`**: Design notes, specifications, and diagrams
- **`scripts/`**: Development helpers (e.g., formatting/spacing tooling)

- **`ios-src/`**: Shell scripts to build and run on iOS devices/simulators
- **`VoxelGame/`**: Xcode project (Swift + Metal) for iOS integration

### Using Taskfile
Tasks are orchestrated with the Task CLI. See the official docs for install and usage: [Task (go-task) CLI](https://taskfile.dev).

Common tasks (from `Taskfile.yml`):

- **Default task**
  - Runs on the first connected iOS device.
  ```bash
  task
  ```

- **Run the demo app on desktop**
  - No diagnostics:
  ```bash
  task run
  ```
  - With diagnostics UI (debug):
  ```bash
  task run:diag
  ```
  - Debug with diagnostics (alias of `run:diag` in this project):
  ```bash
  task run:debug
  ```
  - Release builds:
  ```bash
  task run:release
  task run:release:diag
  ```

- **iOS build/run**
  - Real device:
  ```bash
  task ios:run
  ```
  - Simulator:
  ```bash
  task ios:run:sim
  ```
  - Stream logs while running (device/simulator):
  ```bash
  task ios:run:device:logs
  task ios:run:sim:logs
  ```

- **Testing**
  - Voxel plugin tests only:
  ```bash
  task test:plugin
  ```
  - All workspace tests:
  ```bash
  task test:all
  ```

- **Cleaning**
  ```bash
  task clean
  ```

Notes:
- iOS tasks require Xcode and appropriate signing/device setup.
- Desktop runs rely on your Rust toolchain as defined by `rust-toolchain.toml`.


