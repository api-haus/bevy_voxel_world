//! Reusable egui debug UI components for voxel world metrics.
//!
//! This module provides portable egui widgets that can be embedded in any
//! egui context, independent of Bevy systems.
//!
//! # Usage
//!
//! ```ignore
//! use voxel_bevy::debug_ui::voxel_metrics_ui;
//!
//! egui::Window::new("Voxel Debug").show(ctx, |ui| {
//!     voxel_metrics_ui(ui, &metrics);
//! });
//! ```

#[cfg(feature = "debug_ui")]
use egui::{Color32, RichText, Ui};

#[cfg(feature = "debug_ui")]
use voxel_plugin::metrics::WorldMetrics;

/// Render a simple histogram from a slice of u64 values.
#[cfg(feature = "debug_ui")]
fn render_histogram(ui: &mut Ui, values: &std::collections::VecDeque<u64>, label: &str) {
    if values.is_empty() {
        ui.label(format!("{}: No data", label));
        return;
    }

    let min = *values.iter().min().unwrap();
    let max = *values.iter().max().unwrap();
    let avg = values.iter().sum::<u64>() as f64 / values.len() as f64;

    ui.horizontal(|ui| {
        ui.label(format!("{}: ", label));
        ui.label(
            RichText::new(format!("{:.0}µs", avg))
                .color(Color32::LIGHT_GREEN)
                .strong(),
        );
        ui.label(format!("(min: {}µs, max: {}µs)", min, max));
    });

    // Simple bar chart visualization
    let height = 40.0;
    let width = ui.available_width().min(300.0);
    let (response, painter) = ui.allocate_painter(egui::vec2(width, height), egui::Sense::hover());
    let rect = response.rect;

    if max > 0 {
        let bar_width = width / values.len() as f32;
        let range = (max - min).max(1) as f32;

        for (i, &value) in values.iter().enumerate() {
            let normalized = if range > 0.0 {
                (value - min) as f32 / range
            } else {
                0.5
            };
            let bar_height = normalized * height * 0.9 + height * 0.1;

            // Color based on value (green = fast, red = slow)
            let color = if avg > 0.0 {
                let ratio = value as f32 / avg as f32;
                if ratio < 0.5 {
                    Color32::from_rgb(100, 200, 100) // Fast (green)
                } else if ratio < 1.5 {
                    Color32::from_rgb(200, 200, 100) // Normal (yellow)
                } else {
                    Color32::from_rgb(200, 100, 100) // Slow (red)
                }
            } else {
                Color32::GRAY
            };

            let x = rect.left() + i as f32 * bar_width;
            let bar_rect = egui::Rect::from_min_max(
                egui::pos2(x, rect.bottom() - bar_height),
                egui::pos2(x + bar_width - 1.0, rect.bottom()),
            );
            painter.rect_filled(bar_rect, 0.0, color);
        }
    }
}

/// Render full voxel world metrics UI.
///
/// This is a reusable egui widget that can be embedded in any egui context.
/// It displays:
/// - Timing histogram (mesh generation times)
/// - LOD distribution
/// - Memory usage
/// - Visibility stats
#[cfg(feature = "debug_ui")]
pub fn voxel_metrics_ui(ui: &mut Ui, metrics: &WorldMetrics) {
    // Summary line
    ui.horizontal(|ui| {
        ui.label("Nodes:");
        ui.label(
            RichText::new(format!("{}", metrics.visible_nodes))
                .color(Color32::LIGHT_BLUE)
                .strong(),
        );
        ui.separator();
        ui.label("Triangles:");
        ui.label(
            RichText::new(format!("{}", metrics.visible_triangles))
                .color(Color32::LIGHT_BLUE)
                .strong(),
        );
        ui.separator();
        ui.label("Memory:");
        ui.label(
            RichText::new(format!("{:.2} MB", metrics.mesh_memory_mb()))
                .color(Color32::LIGHT_BLUE)
                .strong(),
        );
    });

    ui.separator();

    // Timing section
    egui::CollapsingHeader::new("Timing")
        .default_open(true)
        .show(ui, |ui| {
            render_histogram(ui, metrics.mesh_timings.as_slice(), "Mesh");
            ui.add_space(4.0);
            render_histogram(ui, metrics.refine_timings.as_slice(), "Refine");

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Last frame:");
                ui.label(format!(
                    "mesh: {}µs, refine: {}µs",
                    metrics.last_mesh_us, metrics.last_refine_us
                ));
            });
        });

    // LOD distribution section
    egui::CollapsingHeader::new("LOD Distribution")
        .default_open(false)
        .show(ui, |ui| {
            let total = metrics.total_leaves();
            if total == 0 {
                ui.label("No leaves");
                return;
            }

            // Find active LOD range
            let mut first_active = None;
            let mut last_active = 0;
            for (lod, &count) in metrics.leaves_per_lod.iter().enumerate() {
                if count > 0 {
                    if first_active.is_none() {
                        first_active = Some(lod);
                    }
                    last_active = lod;
                }
            }

            let Some(first) = first_active else {
                ui.label("No active LODs");
                return;
            };

            // Show active LODs in a grid
            egui::Grid::new("lod_grid")
                .num_columns(4)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("LOD").strong());
                    ui.label(RichText::new("Nodes").strong());
                    ui.label(RichText::new("Vertices").strong());
                    ui.label(RichText::new("Triangles").strong());
                    ui.end_row();

                    for lod in first..=last_active {
                        let count = metrics.leaves_per_lod[lod];
                        if count == 0 {
                            continue;
                        }

                        let verts = metrics.vertices_per_lod[lod];
                        let tris = metrics.indices_per_lod[lod] / 3;

                        // Color by percentage of total
                        let pct = count as f32 / total as f32;
                        let color = if pct > 0.3 {
                            Color32::from_rgb(200, 150, 100) // High concentration
                        } else if pct > 0.1 {
                            Color32::from_rgb(150, 180, 150) // Medium
                        } else {
                            Color32::LIGHT_GRAY // Low
                        };

                        ui.label(RichText::new(format!("{}", lod)).color(color));
                        ui.label(RichText::new(format!("{}", count)).color(color));
                        ui.label(format!("{}", verts));
                        ui.label(format!("{}", tris));
                        ui.end_row();
                    }
                });

            // Totals
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Total:");
                ui.label(format!(
                    "{} nodes, {} verts, {} tris",
                    total,
                    metrics.total_vertices(),
                    metrics.total_indices() / 3
                ));
            });
        });

    // Session stats
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Session chunks:");
        ui.label(
            RichText::new(format!("{}", metrics.total_chunks_generated))
                .color(Color32::LIGHT_GRAY),
        );
    });
}

/// Compact version of metrics UI showing only essential stats.
#[cfg(feature = "debug_ui")]
pub fn voxel_metrics_compact(ui: &mut Ui, metrics: &WorldMetrics) {
    ui.horizontal(|ui| {
        ui.label(format!("{} nodes", metrics.visible_nodes));
        ui.separator();
        ui.label(format!("{:.1}MB", metrics.mesh_memory_mb()));
        ui.separator();
        ui.label(format!(
            "mesh: {:.0}µs",
            metrics.avg_mesh_timing_us()
        ));
        ui.separator();
        ui.label(format!(
            "refine: {:.0}µs",
            metrics.avg_refine_timing_us()
        ));
    });
}
