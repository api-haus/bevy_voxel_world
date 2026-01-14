//! WASM binary entry point for Emscripten builds.
//!
//! This file is only used when building with `wasm32-unknown-emscripten`
//! target. It re-exports the C API functions and provides the required main
//! function.

use std::ffi::CStr;
use std::os::raw::c_char;

// Re-use the native NoiseNode implementation
use voxel_noise::NoiseNode;

/// Create a noise node from an encoded node tree string.
#[no_mangle]
pub extern "C" fn vx_noise_create(encoded: *const c_char) -> usize {
  if encoded.is_null() {
    return 0;
  }

  let encoded_str = match unsafe { CStr::from_ptr(encoded) }.to_str() {
    Ok(s) => s,
    Err(_) => return 0,
  };

  match NoiseNode::from_encoded(encoded_str) {
    Some(node) => Box::into_raw(Box::new(node)) as usize,
    None => 0,
  }
}

/// Generate noise values on a uniform 3D grid.
#[no_mangle]
pub extern "C" fn vx_noise_gen_3d(
  handle: usize,
  output: *mut f32,
  x_off: f32,
  y_off: f32,
  z_off: f32,
  x_cnt: i32,
  y_cnt: i32,
  z_cnt: i32,
  x_step: f32,
  y_step: f32,
  z_step: f32,
  seed: i32,
) {
  if handle == 0 || output.is_null() {
    return;
  }

  let node = unsafe { &*(handle as *const NoiseNode) };
  let count = (x_cnt * y_cnt * z_cnt) as usize;
  let output_slice = unsafe { std::slice::from_raw_parts_mut(output, count) };

  node.gen_uniform_grid_3d(
    output_slice,
    x_off,
    y_off,
    z_off,
    x_cnt,
    y_cnt,
    z_cnt,
    x_step,
    y_step,
    z_step,
    seed,
  );
}

/// Generate noise values on a uniform 2D grid.
#[no_mangle]
pub extern "C" fn vx_noise_gen_2d(
  handle: usize,
  output: *mut f32,
  x_off: f32,
  y_off: f32,
  x_cnt: i32,
  y_cnt: i32,
  x_step: f32,
  y_step: f32,
  seed: i32,
) {
  if handle == 0 || output.is_null() {
    return;
  }

  let node = unsafe { &*(handle as *const NoiseNode) };
  let count = (x_cnt * y_cnt) as usize;
  let output_slice = unsafe { std::slice::from_raw_parts_mut(output, count) };

  node.gen_uniform_grid_2d(
    output_slice,
    x_off,
    y_off,
    x_cnt,
    y_cnt,
    x_step,
    y_step,
    seed,
  );
}

/// Destroy a noise node and free its memory.
#[no_mangle]
pub extern "C" fn vx_noise_destroy(handle: usize) {
  if handle == 0 {
    return;
  }

  unsafe {
    drop(Box::from_raw(handle as *mut NoiseNode));
  }
}

/// Required main function for Emscripten.
fn main() {}
