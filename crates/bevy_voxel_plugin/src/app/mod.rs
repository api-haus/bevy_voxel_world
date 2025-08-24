//! Application layer for orchestrating domain operations
//!
//! This layer contains use-cases that coordinate domain entities and ports.
//! It acts as the boundary between the pure domain and the infrastructure.

pub mod commands;
pub mod queries;
pub mod services;

// Re-export commonly used items
pub use commands::*;
pub use queries::*;
