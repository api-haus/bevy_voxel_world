# voxel_plugin

Framework and engine independent voxel world meshing library.

## Purpose

This crate provides the core infrastructure for voxel world meshing without coupling to any specific game engine or
graphics framework. It can be integrated into Unity (via C# bindings), Bevy, custom engines, or used standalone for
testing.

## Planned Modules

### Routines

Stateless functions for voxel operations:

- Chunk meshing algorithms (greedy meshing, naive, marching cubes)
- LOD generation
- Mesh optimization passes

### Schedules

Task orchestration primitives:

- Dependency graph for chunk updates
- Priority queuing (distance-based, visibility-based)
- Batching strategies

### Tasks

Concrete work units:

- ChunkMeshTask - generate mesh for a single chunk
- ChunkUpdateTask - handle voxel modifications
- LODTransitionTask - manage LOD boundaries

## Design Philosophy

- **No allocations in hot paths** - pre-allocated buffers, arena allocation
- **Data-oriented** - structs of arrays, cache-friendly layouts
- **Deterministic** - same input always produces same output
- **Testable** - all core logic is pure functions with no side effects
