//! Pure voxel domain
//!
//! This module contains all voxel logic without any external dependencies
//! or side effects. All I/O and framework-specific code is accessed through
//! ports (traits).

pub mod entities;

pub mod ports;

pub mod services;

pub mod types;

// Re-export commonly used items

pub use entities::*;

pub use ports::*;

pub use types::*;
