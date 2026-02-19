//! Simple egui-based performance window.
//!
//! Replaces iyes_perf_ui with a minimal egui implementation.
//! Displays FPS, frame time, and custom counters.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

/// Plugin for the performance UI window.
pub struct EguiPerfPlugin;

impl Plugin for EguiPerfPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<PerfWindow>()
      .init_resource::<PerfEntries>()
      .add_systems(EguiPrimaryContextPass, render_perf_window);
  }
}

/// Configuration for the performance window.
#[derive(Resource)]
pub struct PerfWindow {
  /// Window title.
  pub title: &'static str,
  /// Which corner to anchor to.
  pub anchor: Anchor,
  /// Offset from the anchor corner.
  pub offset: egui::Vec2,
  /// Whether the window is visible.
  pub visible: bool,
}

impl Default for PerfWindow {
  fn default() -> Self {
    Self {
      title: "Perf",
      anchor: Anchor::TopLeft,
      offset: egui::vec2(10.0, 10.0),
      visible: true,
    }
  }
}

/// Corner anchor for the window.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Anchor {
  #[default]
  TopLeft,
  TopRight,
  BottomLeft,
  BottomRight,
}

impl Anchor {
  fn to_egui(self) -> egui::Align2 {
    match self {
      Anchor::TopLeft => egui::Align2::LEFT_TOP,
      Anchor::TopRight => egui::Align2::RIGHT_TOP,
      Anchor::BottomLeft => egui::Align2::LEFT_BOTTOM,
      Anchor::BottomRight => egui::Align2::RIGHT_BOTTOM,
    }
  }

  fn pivot_offset(self, offset: egui::Vec2) -> egui::Vec2 {
    match self {
      Anchor::TopLeft => offset,
      Anchor::TopRight => egui::vec2(-offset.x, offset.y),
      Anchor::BottomLeft => egui::vec2(offset.x, -offset.y),
      Anchor::BottomRight => egui::vec2(-offset.x, -offset.y),
    }
  }
}

/// Custom performance entries to display.
#[derive(Resource, Default)]
pub struct PerfEntries {
  entries: Vec<PerfEntry>,
}

#[allow(dead_code)]
impl PerfEntries {
  /// Clear all entries. Call at start of frame before adding new values.
  pub fn clear(&mut self) {
    self.entries.clear();
  }

  /// Add a labeled value entry.
  pub fn add(&mut self, label: impl Into<String>, value: impl Into<String>) {
    self.entries.push(PerfEntry {
      label: label.into(),
      value: value.into(),
    });
  }

  /// Add a timing entry in microseconds.
  pub fn add_time_us(&mut self, label: impl Into<String>, us: u64) {
    let label = label.into();
    if us >= 1000 {
      self.add(label, format!("{:.2} ms", us as f64 / 1000.0));
    } else {
      self.add(label, format!("{} us", us));
    }
  }

  /// Add a timing entry in milliseconds.
  pub fn add_time_ms(&mut self, label: impl Into<String>, ms: f64) {
    self.add(label, format!("{:.2} ms", ms));
  }

  /// Add a count entry.
  pub fn add_count(&mut self, label: impl Into<String>, count: usize) {
    self.add(label, format!("{}", count));
  }
}

struct PerfEntry {
  label: String,
  value: String,
}

fn render_perf_window(
  mut contexts: EguiContexts,
  window: Res<PerfWindow>,
  entries: Res<PerfEntries>,
  diagnostics: Res<DiagnosticsStore>,
) {
  if !window.visible {
    return;
  }

  let Ok(ctx) = contexts.ctx_mut() else {
    return;
  };

  let anchor = window.anchor.to_egui();
  let offset = window.anchor.pivot_offset(window.offset);

  egui::Window::new(window.title)
    .anchor(anchor, offset)
    .resizable(false)
    .collapsible(false)
    .title_bar(false)
    .show(ctx, |ui| {
      ui.set_min_width(120.0);

      // Built-in diagnostics
      if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
          ui.horizontal(|ui| {
            ui.label("FPS:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
              ui.label(format!("{:.0}", value));
            });
          });
        }
      }

      if let Some(frame_time) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        if let Some(value) = frame_time.smoothed() {
          ui.horizontal(|ui| {
            ui.label("Frame:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
              ui.label(format!("{:.2} ms", value));
            });
          });
        }
      }

      // Custom entries
      if !entries.entries.is_empty() {
        ui.separator();
        for entry in &entries.entries {
          ui.horizontal(|ui| {
            ui.label(&entry.label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
              ui.label(&entry.value);
            });
          });
        }
      }
    });
}
