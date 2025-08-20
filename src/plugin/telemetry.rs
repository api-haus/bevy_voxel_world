use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};
use bevy::prelude::*;

// Telemetry resource tracking meshing stats
#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct VoxelTelemetry {
    pub(crate) total_meshed: u64,
    pub(crate) meshed_this_frame: u32,
    pub(crate) queue_len: usize,
    pub(crate) mesh_time_ms_frame: f32,
    pub(crate) apply_time_ms_frame: f32,
    pub(crate) jobs_spawned_frame: u32,
    pub(crate) jobs_completed_frame: u32,
}

// Per-frame reset/update of telemetry counters
pub(crate) fn update_telemetry_begin(
    queue: Res<super::RemeshQueue>,
    mut telemetry: ResMut<VoxelTelemetry>,
) {
    telemetry.meshed_this_frame = 0;
    telemetry.queue_len = queue.inner.len();
    telemetry.mesh_time_ms_frame = 0.0;
    telemetry.apply_time_ms_frame = 0.0;
    telemetry.jobs_spawned_frame = 0;
    telemetry.jobs_completed_frame = 0;
}

// Diagnostic IDs for perf UI overlay
pub(crate) const DIAG_VOX_QUEUE_LEN: DiagnosticPath = DiagnosticPath::const_new("vox.queue_len");
pub(crate) const DIAG_VOX_MESHED_THIS_FRAME: DiagnosticPath =
    DiagnosticPath::const_new("vox.meshed_this_frame");
pub(crate) const DIAG_VOX_TOTAL_MESHED: DiagnosticPath =
    DiagnosticPath::const_new("vox.total_meshed");
pub(crate) const DIAG_VOX_JOBS_SPAWNED: DiagnosticPath =
    DiagnosticPath::const_new("vox.jobs_spawned");
pub(crate) const DIAG_VOX_JOBS_COMPLETED: DiagnosticPath =
    DiagnosticPath::const_new("vox.jobs_completed");
pub(crate) const DIAG_VOX_MESH_TIME_MS: DiagnosticPath =
    DiagnosticPath::const_new("vox.mesh_time_ms");
pub(crate) const DIAG_VOX_APPLY_TIME_MS: DiagnosticPath =
    DiagnosticPath::const_new("vox.apply_time_ms");

// Register our diagnostics so iyes_perf_ui can display them
pub(crate) fn register_voxel_diagnostics(app: &mut App) {
    app.register_diagnostic(Diagnostic::new(DIAG_VOX_QUEUE_LEN));
    app.register_diagnostic(Diagnostic::new(DIAG_VOX_MESHED_THIS_FRAME));
    app.register_diagnostic(Diagnostic::new(DIAG_VOX_TOTAL_MESHED));
    app.register_diagnostic(Diagnostic::new(DIAG_VOX_JOBS_SPAWNED));
    app.register_diagnostic(Diagnostic::new(DIAG_VOX_JOBS_COMPLETED));
    app.register_diagnostic(Diagnostic::new(DIAG_VOX_MESH_TIME_MS));
    app.register_diagnostic(Diagnostic::new(DIAG_VOX_APPLY_TIME_MS));
}

// Publish the current telemetry to Bevy Diagnostics each frame
pub(crate) fn publish_diagnostics(telemetry: Res<VoxelTelemetry>, mut diagnostics: Diagnostics) {
    diagnostics.add_measurement(&DIAG_VOX_QUEUE_LEN, || telemetry.queue_len as f64);
    diagnostics.add_measurement(&DIAG_VOX_MESHED_THIS_FRAME, || {
        telemetry.meshed_this_frame as f64
    });
    diagnostics.add_measurement(&DIAG_VOX_TOTAL_MESHED, || telemetry.total_meshed as f64);
    diagnostics.add_measurement(&DIAG_VOX_JOBS_SPAWNED, || {
        telemetry.jobs_spawned_frame as f64
    });
    diagnostics.add_measurement(&DIAG_VOX_JOBS_COMPLETED, || {
        telemetry.jobs_completed_frame as f64
    });
    diagnostics.add_measurement(&DIAG_VOX_MESH_TIME_MS, || {
        telemetry.mesh_time_ms_frame as f64
    });
    diagnostics.add_measurement(&DIAG_VOX_APPLY_TIME_MS, || {
        telemetry.apply_time_ms_frame as f64
    });
}
