use super::*;

fn make_sphere_volume() -> (
  Box<[SdfSample; SAMPLE_SIZE_CB]>,
  Box<[MaterialId; SAMPLE_SIZE_CB]>,
) {
  use crate::{coord_to_index, SAMPLE_SIZE};

  let mut volume = Box::new([0i8; SAMPLE_SIZE_CB]);
  let materials = Box::new([0u8; SAMPLE_SIZE_CB]);

  let center = SAMPLE_SIZE as f32 / 2.0;
  let radius = 10.0;

  for x in 0..SAMPLE_SIZE {
    for y in 0..SAMPLE_SIZE {
      for z in 0..SAMPLE_SIZE {
        let dx = x as f32 - center;
        let dy = y as f32 - center;
        let dz = z as f32 - center;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt() - radius;
        let idx = coord_to_index(x, y, z);
        volume[idx] = dist.clamp(-127.0, 127.0) as i8;
      }
    }
  }

  (volume, materials)
}

#[test]
fn test_single_request() {
  let mut stage = MeshingStage::new();
  let (volume, materials) = make_sphere_volume();

  let id = stage.enqueue(volume, materials, MeshConfig::default());
  assert_eq!(id, 0);
  assert_eq!(stage.pending_count(), 1);

  let processed = stage.tick();
  assert_eq!(processed, 1);
  assert_eq!(stage.pending_count(), 0);
  assert_eq!(stage.completed_count(), 1);

  let completions = stage.drain_completions();
  assert_eq!(completions.len(), 1);
  assert_eq!(completions[0].id, 0);
  assert!(!completions[0].output.vertices.is_empty());
}

#[test]
fn test_multiple_requests() {
  let mut stage = MeshingStage::new();

  // Enqueue 4 requests
  for _ in 0..4 {
    let (volume, materials) = make_sphere_volume();
    stage.enqueue(volume, materials, MeshConfig::default());
  }

  assert_eq!(stage.pending_count(), 4);

  let processed = stage.tick();
  assert_eq!(processed, 4);
  assert_eq!(stage.completed_count(), 4);

  let completions = stage.drain_completions();
  assert_eq!(completions.len(), 4);

  // Verify all IDs are unique
  let ids: Vec<u64> = completions.iter().map(|c| c.id).collect();
  assert_eq!(ids, vec![0, 1, 2, 3]);
}

#[test]
fn test_empty_tick() {
  let mut stage = MeshingStage::new();
  assert!(stage.is_idle());

  let processed = stage.tick();
  assert_eq!(processed, 0);
  assert!(stage.is_idle());
}
