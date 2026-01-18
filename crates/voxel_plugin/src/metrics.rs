//! Engine-agnostic metrics collection for voxel world statistics.
//!
//! Feature-gated and runtime-toggled to ensure zero overhead when disabled.
//!
//! # Usage
//!
//! ```ignore
//! use voxel_plugin::metrics::{WorldMetrics, COLLECT_METRICS};
//!
//! // Compile with --features metrics
//! // Runtime toggle:
//! COLLECT_METRICS.store(false, Ordering::Relaxed);
//!
//! // Update metrics during refinement:
//! metrics.update_from_leaves(&leaves, &config);
//!
//! // Record mesh timing:
//! metrics.record_mesh_timing(timing_us);
//! ```

use std::collections::VecDeque;
#[cfg(feature = "metrics")]
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicBool;

/// Runtime toggle for metrics collection.
/// Set to false to disable metrics gathering at runtime.
pub static COLLECT_METRICS: AtomicBool = AtomicBool::new(true);

/// Check if metrics collection is enabled (both compile-time and runtime).
#[inline]
pub fn is_enabled() -> bool {
    #[cfg(feature = "metrics")]
    {
        COLLECT_METRICS.load(Ordering::Relaxed)
    }
    #[cfg(not(feature = "metrics"))]
    {
        false
    }
}

/// Rolling window for storing recent values (e.g., timing history).
#[derive(Debug, Clone)]
pub struct RollingWindow<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> RollingWindow<T> {
    /// Create a new rolling window with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new value, evicting the oldest if at capacity.
    pub fn push(&mut self, value: T) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(value);
    }

    /// Get the number of values in the window.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the window is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Clear all values.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Iterate over values (oldest to newest).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    /// Get the most recent value.
    pub fn last(&self) -> Option<&T> {
        self.buffer.back()
    }

    /// Get all values as a slice (for histogram rendering).
    pub fn as_slice(&self) -> &VecDeque<T> {
        &self.buffer
    }
}

impl<T: Copy + Default + std::ops::Add<Output = T>> RollingWindow<T> {
    /// Compute the sum of all values.
    pub fn sum(&self) -> T {
        self.buffer.iter().copied().fold(T::default(), |acc, x| acc + x)
    }
}

impl RollingWindow<u64> {
    /// Compute the average of all values.
    pub fn average(&self) -> f64 {
        if self.buffer.is_empty() {
            0.0
        } else {
            self.sum() as f64 / self.buffer.len() as f64
        }
    }

    /// Get min and max values.
    pub fn min_max(&self) -> Option<(u64, u64)> {
        if self.buffer.is_empty() {
            None
        } else {
            let min = *self.buffer.iter().min().unwrap();
            let max = *self.buffer.iter().max().unwrap();
            Some((min, max))
        }
    }
}

impl Default for RollingWindow<u64> {
    fn default() -> Self {
        Self::new(128) // Default to 128 samples (~2 seconds at 60fps)
    }
}

/// World-level statistics updated each refinement cycle.
#[derive(Debug, Clone)]
pub struct WorldMetrics {
    // LOD distribution
    /// Count of leaf nodes at each LOD level (index = LOD).
    pub leaves_per_lod: [u32; 16],
    /// Vertex count per LOD level.
    pub vertices_per_lod: [u64; 16],
    /// Index count per LOD level.
    pub indices_per_lod: [u64; 16],

    // Visibility
    /// Number of currently visible nodes.
    pub visible_nodes: u32,
    /// Total visible triangles (indices / 3).
    pub visible_triangles: u64,

    // Memory
    /// Approximate mesh memory usage (vertices + indices).
    pub mesh_memory_bytes: u64,
    /// Approximate octree memory overhead.
    pub octree_memory_bytes: u64,

    // Timing
    /// Rolling window of mesh generation times in microseconds.
    pub mesh_timings: RollingWindow<u64>,
    /// Rolling window of refinement times in microseconds.
    pub refine_timings: RollingWindow<u64>,
    /// Rolling window of sample times in microseconds.
    pub sample_timings: RollingWindow<u64>,

    // Last frame snapshot (for UI)
    /// Last refinement time in microseconds.
    pub last_refine_us: u64,
    /// Last mesh generation time in microseconds.
    pub last_mesh_us: u64,
    /// Total chunks generated this session.
    pub total_chunks_generated: u64,
}

impl Default for WorldMetrics {
    fn default() -> Self {
        Self {
            leaves_per_lod: [0; 16],
            vertices_per_lod: [0; 16],
            indices_per_lod: [0; 16],
            visible_nodes: 0,
            visible_triangles: 0,
            mesh_memory_bytes: 0,
            octree_memory_bytes: 0,
            mesh_timings: RollingWindow::new(128),
            refine_timings: RollingWindow::new(128),
            sample_timings: RollingWindow::new(128),
            last_refine_us: 0,
            last_mesh_us: 0,
            total_chunks_generated: 0,
        }
    }
}

impl WorldMetrics {
    /// Create new metrics with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all metrics to zero.
    pub fn reset(&mut self) {
        self.leaves_per_lod.fill(0);
        self.vertices_per_lod.fill(0);
        self.indices_per_lod.fill(0);
        self.visible_nodes = 0;
        self.visible_triangles = 0;
        self.mesh_memory_bytes = 0;
        self.octree_memory_bytes = 0;
        self.mesh_timings.clear();
        self.refine_timings.clear();
        self.sample_timings.clear();
        self.last_refine_us = 0;
        self.last_mesh_us = 0;
        // Don't reset total_chunks_generated - it's cumulative
    }

    /// Record a mesh generation timing.
    pub fn record_mesh_timing(&mut self, timing_us: u64) {
        if is_enabled() {
            self.mesh_timings.push(timing_us);
            self.last_mesh_us = timing_us;
        }
    }

    /// Record a refinement timing.
    pub fn record_refine_timing(&mut self, timing_us: u64) {
        if is_enabled() {
            self.refine_timings.push(timing_us);
            self.last_refine_us = timing_us;
        }
    }

    /// Record a sample timing.
    pub fn record_sample_timing(&mut self, timing_us: u64) {
        if is_enabled() {
            self.sample_timings.push(timing_us);
        }
    }

    /// Record chunk statistics (LOD, vertex count, index count).
    pub fn record_chunk(&mut self, lod: i32, vertex_count: u32, index_count: u32) {
        if !is_enabled() {
            return;
        }

        let lod_idx = (lod as usize).min(15);
        self.leaves_per_lod[lod_idx] += 1;
        self.vertices_per_lod[lod_idx] += vertex_count as u64;
        self.indices_per_lod[lod_idx] += index_count as u64;

        // Approximate memory: 32 bytes per vertex, 4 bytes per index
        let chunk_memory = (vertex_count as u64 * 32) + (index_count as u64 * 4);
        self.mesh_memory_bytes += chunk_memory;

        self.visible_nodes += 1;
        self.visible_triangles += (index_count / 3) as u64;
        self.total_chunks_generated += 1;
    }

    /// Remove chunk statistics (when chunk is despawned).
    pub fn remove_chunk(&mut self, lod: i32, vertex_count: u32, index_count: u32) {
        if !is_enabled() {
            return;
        }

        let lod_idx = (lod as usize).min(15);
        self.leaves_per_lod[lod_idx] = self.leaves_per_lod[lod_idx].saturating_sub(1);
        self.vertices_per_lod[lod_idx] =
            self.vertices_per_lod[lod_idx].saturating_sub(vertex_count as u64);
        self.indices_per_lod[lod_idx] =
            self.indices_per_lod[lod_idx].saturating_sub(index_count as u64);

        let chunk_memory = (vertex_count as u64 * 32) + (index_count as u64 * 4);
        self.mesh_memory_bytes = self.mesh_memory_bytes.saturating_sub(chunk_memory);

        self.visible_nodes = self.visible_nodes.saturating_sub(1);
        self.visible_triangles = self.visible_triangles.saturating_sub((index_count / 3) as u64);
    }

    /// Get the total leaf count across all LODs.
    pub fn total_leaves(&self) -> u32 {
        self.leaves_per_lod.iter().sum()
    }

    /// Get the total vertex count across all LODs.
    pub fn total_vertices(&self) -> u64 {
        self.vertices_per_lod.iter().sum()
    }

    /// Get the total index count across all LODs.
    pub fn total_indices(&self) -> u64 {
        self.indices_per_lod.iter().sum()
    }

    /// Get average mesh timing in microseconds.
    pub fn avg_mesh_timing_us(&self) -> f64 {
        self.mesh_timings.average()
    }

    /// Get average refinement timing in microseconds.
    pub fn avg_refine_timing_us(&self) -> f64 {
        self.refine_timings.average()
    }

    /// Format mesh memory as a human-readable string.
    pub fn mesh_memory_mb(&self) -> f64 {
        self.mesh_memory_bytes as f64 / 1_048_576.0
    }
}

#[cfg(all(test, feature = "metrics"))]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_window() {
        let mut window = RollingWindow::new(3);
        assert!(window.is_empty());

        window.push(10u64);
        window.push(20);
        window.push(30);
        assert_eq!(window.len(), 3);
        assert_eq!(window.sum(), 60);
        assert_eq!(window.average(), 20.0);

        // Push one more, oldest should be evicted
        window.push(40);
        assert_eq!(window.len(), 3);
        assert_eq!(window.sum(), 90);
        assert_eq!(window.average(), 30.0);

        let (min, max) = window.min_max().unwrap();
        assert_eq!(min, 20);
        assert_eq!(max, 40);
    }

    #[test]
    fn test_world_metrics() {
        let mut metrics = WorldMetrics::new();

        // Record some chunks
        metrics.record_chunk(0, 1000, 3000);
        metrics.record_chunk(1, 500, 1500);
        metrics.record_chunk(0, 800, 2400);

        assert_eq!(metrics.leaves_per_lod[0], 2);
        assert_eq!(metrics.leaves_per_lod[1], 1);
        assert_eq!(metrics.total_leaves(), 3);
        assert_eq!(metrics.visible_nodes, 3);

        // Remove a chunk
        metrics.remove_chunk(0, 1000, 3000);
        assert_eq!(metrics.leaves_per_lod[0], 1);
        assert_eq!(metrics.visible_nodes, 2);
    }

    #[test]
    fn test_timing_recording() {
        let mut metrics = WorldMetrics::new();

        metrics.record_mesh_timing(1000);
        metrics.record_mesh_timing(2000);
        metrics.record_mesh_timing(3000);

        assert_eq!(metrics.mesh_timings.len(), 3);
        assert_eq!(metrics.avg_mesh_timing_us(), 2000.0);
        assert_eq!(metrics.last_mesh_us, 3000);
    }
}
