//! Infrastructure layer - external adapters and framework integration
//!
//! This layer contains all the implementation details that the domain
//! doesn't need to know about, including Bevy integration, meshing
//! algorithms, and other external dependencies.

pub mod bevy_adapters;

pub mod meshing;

pub mod random;

// Re-export commonly used items

pub use bevy_adapters::BevyVoxelPlugin;

pub use meshing::SurfaceNetsMesher;

pub use random::WyRandGenerator;
