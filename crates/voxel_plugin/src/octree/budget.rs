//! Rate limiting configuration for octree refinement.
//!
//! Prevents frame spikes from unbounded cascading operations by limiting
//! the number of subdivisions and collapses per frame.

/// Rate limiting configuration for octree refinement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefinementBudget {
	/// Maximum subdivisions per frame (0 = unlimited).
	pub max_subdivisions: usize,
	/// Maximum collapses per frame (0 = unlimited).
	pub max_collapses: usize,
	/// Maximum LOD difference between adjacent cells (1 = no T-junctions).
	/// Set to 0 to disable neighbor enforcement.
	pub max_relative_lod: i32,
	/// Maximum neighbor enforcement iterations per frame.
	/// Prevents runaway cascading when many neighbors need fixing.
	pub max_neighbor_iterations: usize,
}

impl RefinementBudget {
	/// Default budget with reasonable limits.
	pub const DEFAULT: Self = Self {
		max_subdivisions: 32,
		max_collapses: 32,
		max_relative_lod: 1,
		max_neighbor_iterations: 4,
	};

	/// Unlimited budget for testing or special cases.
	pub const UNLIMITED: Self = Self {
		max_subdivisions: usize::MAX,
		max_collapses: usize::MAX,
		max_relative_lod: 1,
		max_neighbor_iterations: usize::MAX,
	};

	/// Budget with neighbor enforcement disabled.
	pub const NO_NEIGHBOR_ENFORCEMENT: Self = Self {
		max_subdivisions: 32,
		max_collapses: 32,
		max_relative_lod: 0,
		max_neighbor_iterations: 0,
	};

	/// Check if neighbor enforcement is enabled.
	#[inline]
	pub fn neighbor_enforcement_enabled(&self) -> bool {
		self.max_relative_lod > 0
	}

	/// Check if more subdivisions can be performed.
	#[inline]
	pub fn can_subdivide(&self, performed: usize) -> bool {
		self.max_subdivisions == 0 || performed < self.max_subdivisions
	}

	/// Check if more collapses can be performed.
	#[inline]
	pub fn can_collapse(&self, performed: usize) -> bool {
		self.max_collapses == 0 || performed < self.max_collapses
	}
}

impl Default for RefinementBudget {
	fn default() -> Self {
		Self::DEFAULT
	}
}

/// Statistics from refinement execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefinementStats {
	/// Number of user-requested subdivisions performed.
	pub subdivisions_performed: usize,
	/// Number of collapses performed.
	pub collapses_performed: usize,
	/// Number of neighbor enforcement subdivisions performed.
	/// Tracked separately - these are mandatory for correctness.
	pub neighbor_subdivisions_performed: usize,
}

impl RefinementStats {
	/// Total number of transitions performed.
	#[inline]
	pub fn total_transitions(&self) -> usize {
		self.subdivisions_performed + self.collapses_performed + self.neighbor_subdivisions_performed
	}

	/// Total subdivisions including neighbor enforcement.
	#[inline]
	pub fn total_subdivisions(&self) -> usize {
		self.subdivisions_performed + self.neighbor_subdivisions_performed
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_budget() {
		let budget = RefinementBudget::default();
		assert_eq!(budget.max_subdivisions, 32);
		assert_eq!(budget.max_collapses, 32);
		assert_eq!(budget.max_relative_lod, 1);
		assert_eq!(budget.max_neighbor_iterations, 4);
		assert!(budget.neighbor_enforcement_enabled());
	}

	#[test]
	fn test_unlimited_budget() {
		let budget = RefinementBudget::UNLIMITED;
		assert_eq!(budget.max_subdivisions, usize::MAX);
		assert_eq!(budget.max_collapses, usize::MAX);
		assert!(budget.neighbor_enforcement_enabled());
	}

	#[test]
	fn test_no_neighbor_enforcement() {
		let budget = RefinementBudget::NO_NEIGHBOR_ENFORCEMENT;
		assert!(!budget.neighbor_enforcement_enabled());
		assert_eq!(budget.max_relative_lod, 0);
		assert_eq!(budget.max_neighbor_iterations, 0);
	}

	#[test]
	fn test_can_subdivide() {
		let budget = RefinementBudget {
			max_subdivisions: 5,
			..Default::default()
		};
		assert!(budget.can_subdivide(0));
		assert!(budget.can_subdivide(4));
		assert!(!budget.can_subdivide(5));
		assert!(!budget.can_subdivide(10));
	}

	#[test]
	fn test_can_collapse() {
		let budget = RefinementBudget {
			max_collapses: 3,
			..Default::default()
		};
		assert!(budget.can_collapse(0));
		assert!(budget.can_collapse(2));
		assert!(!budget.can_collapse(3));
	}

	#[test]
	fn test_unlimited_budget_always_allows() {
		let budget = RefinementBudget {
			max_subdivisions: 0, // 0 = unlimited
			max_collapses: 0,
			..Default::default()
		};
		assert!(budget.can_subdivide(1000));
		assert!(budget.can_collapse(1000));
	}

	#[test]
	fn test_stats_totals() {
		let stats = RefinementStats {
			subdivisions_performed: 10,
			collapses_performed: 5,
			neighbor_subdivisions_performed: 3,
		};
		assert_eq!(stats.total_transitions(), 18);
		assert_eq!(stats.total_subdivisions(), 13);
	}
}
