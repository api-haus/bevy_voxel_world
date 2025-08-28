mod atmosphere;

mod camera;

mod diag;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
	voxel_demo_app::run();
}

#[cfg(all(target_arch = "wasm32", feature = "wasm_threads"))]
fn main() {
	use wasm_bindgen_futures::{JsFuture, spawn_local};

	spawn_local(async move {
		let threads = std::thread::available_parallelism()
			.map(|n| n.get())
			.unwrap_or(2);

		let promise = wasm_bindgen_rayon::init_thread_pool(threads);
		JsFuture::from(promise)
			.await
			.expect("failed to init wasm thread pool");

		voxel_demo_app::run();
	});
}

#[cfg(all(target_arch = "wasm32", not(feature = "wasm_threads")))]
fn main() {
	voxel_demo_app::run();
}
