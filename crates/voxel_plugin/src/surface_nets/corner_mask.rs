//! Corner mask computation using portable SIMD.
//!
//! The corner mask is an 8-bit value where each bit indicates whether a corner
//! of the 2x2x2 cell is inside the surface (negative SDF value).

use std::simd::{cmp::SimdPartialOrd, i8x8};

/// Build corner mask from 8 SDF samples using SIMD.
///
/// Each bit in the result corresponds to one corner:
/// - Bit 0: corner (0,0,0)
/// - Bit 1: corner (1,0,0)
/// - Bit 2: corner (0,1,0)
/// - Bit 3: corner (1,1,0)
/// - Bit 4: corner (0,0,1)
/// - Bit 5: corner (1,0,1)
/// - Bit 6: corner (0,1,1)
/// - Bit 7: corner (1,1,1)
///
/// A bit is set if the corresponding sample is negative (inside surface).
#[inline]
pub fn build(samples: [i8; 8]) -> u8 {
  let simd_samples = i8x8::from_array(samples);
  let zero = i8x8::splat(0);

  // Compare all 8 samples against zero simultaneously
  // mask[i] = true if samples[i] < 0
  let mask = simd_samples.simd_lt(zero);

  // Convert boolean mask to bitmask (each lane becomes 1 bit)
  mask.to_bitmask() as u8
}

#[cfg(test)]
#[path = "corner_mask_test.rs"]
mod corner_mask_test;
