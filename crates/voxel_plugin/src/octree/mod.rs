//! Octree module for LOD-based spatial subdivision.
//!
//! This module provides implicit octree data structures where leaves define
//! the tree structure. No explicit tree nodes are maintained - parent/child
//! relationships are computed on-demand via coordinate math.
//!
//! # LOD Convention
//!
//! LOD 0 = finest detail (smallest cells), higher LOD = coarser.
//!
//! ```text
//! Cell Size = VOXELS_PER_CELL * voxel_size * 2^LOD
//!           = 28 * voxel_size * 2^LOD
//! ```
//!
//! # Module Structure
//!
//! - [`node`]: `OctreeNode` - immutable value type for octree positions
//! - (future) `config`: `OctreeConfig` - coordinate math and LOD thresholds
//! - (future) `leaves`: `OctreeLeaves` - implicit bounds via
//!   HashSet<OctreeNode>
//! - (future) `transition`: `TransitionGroup` - atomic subdivide/merge
//!   operations
//! - (future) `refinement`: LOD refinement algorithm

pub mod bounds;
pub mod budget;
pub mod config;
pub mod leaves;
pub mod node;
pub mod refinement;
pub mod transition;

// Re-exports
pub use bounds::DAabb3;
pub use budget::{RefinementBudget, RefinementStats};
pub use config::OctreeConfig;
pub use leaves::OctreeLeaves;
pub use node::OctreeNode;
pub use refinement::{refine, RefinementInput, RefinementOutput};
pub use transition::{TransitionGroup, TransitionType};

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
