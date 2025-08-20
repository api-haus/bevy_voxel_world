use bevy::prelude::*;
use fast_surface_nets::SurfaceNetsBuffer;
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::voxels::storage::VoxelStorage;

// Budget for remeshing work per frame
#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct RemeshBudget {
    pub(crate) max_chunks_per_frame: usize,
    pub(crate) time_slice_ms: u64,
}

impl Default for RemeshBudget {
    fn default() -> Self {
        Self {
            max_chunks_per_frame: 4,
            time_slice_ms: 2,
        }
    }
}

// FIFO queue of chunk entities needing remesh
#[derive(Resource, Default)]
pub(crate) struct RemeshQueue {
    pub(crate) inner: VecDeque<Entity>,
}

// Cross-thread channel to forward meshing results back to the main thread
#[derive(Resource)]
pub(crate) struct RemeshResultChannel {
    pub(crate) tx: Sender<super::RemeshReady>,
    pub(crate) rx: Arc<Mutex<Receiver<super::RemeshReady>>>,
}

// Drain the queue within the budget and spawn background meshing jobs
pub(crate) fn drain_queue_and_spawn_jobs(
    budget: Res<RemeshBudget>,
    mut queue: ResMut<RemeshQueue>,
    channels: Res<RemeshResultChannel>,
    q_storage: Query<&VoxelStorage>,
) {
    let start = Instant::now();
    let time_slice = Duration::from_millis(budget.time_slice_ms);

    let mut processed = 0usize;
    while processed < budget.max_chunks_per_frame && start.elapsed() <= time_slice {
        let Some(entity) = queue.inner.pop_front() else {
            break;
        };
        processed += 1;

        let Ok(storage) = q_storage.get(entity) else {
            continue;
        };
        let s = storage.dims.sample;
        if !(s.x == 18 && s.y == 18 && s.z == 18) {
            continue;
        }

        // Copy SDF to move into the rayon task
        let sdf: Vec<f32> = storage.sdf.iter().copied().collect();
        let tx = channels.tx.clone();

        rayon::spawn(move || {
            // Early skip
            let mut any_pos = false;
            let mut any_neg = false;
            for &v in &sdf {
                if v <= 0.0 {
                    any_neg = true;
                } else {
                    any_pos = true;
                }
                if any_pos && any_neg {
                    break;
                }
            }
            if !(any_pos && any_neg) {
                return;
            }

            let mut buffer = SurfaceNetsBuffer::default();
            fast_surface_nets::surface_nets(
                &sdf,
                &fast_surface_nets::ndshape::ConstShape3u32::<18, 18, 18>,
                [0; 3],
                [17, 17, 17],
                &mut buffer,
            );

            if buffer.positions.is_empty() {
                return;
            }

            let _ = tx.send(super::RemeshReady { entity, buffer });
        });
    }
}

// Pump results from background tasks into the Bevy event queue
pub(crate) fn pump_remesh_results(
    channels: Res<RemeshResultChannel>,
    mut evw: EventWriter<super::RemeshReady>,
) {
    loop {
        let Ok(guard) = channels.rx.lock() else { break };
        match guard.try_recv() {
            Ok(result) => {
                drop(guard);
                evw.write(result);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }
}


