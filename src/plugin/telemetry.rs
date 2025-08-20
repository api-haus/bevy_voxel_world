use bevy::prelude::*;

// Telemetry resource tracking meshing stats
#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct VoxelTelemetry {
    pub(crate) total_meshed: u64,
    pub(crate) meshed_this_frame: u32,
    pub(crate) queue_len: usize,
}

// Per-frame reset/update of telemetry counters
pub(crate) fn update_telemetry_begin(
    queue: Res<super::RemeshQueue>,
    mut telemetry: ResMut<VoxelTelemetry>,
) {
    telemetry.meshed_this_frame = 0;
    telemetry.queue_len = queue.inner.len();
}
