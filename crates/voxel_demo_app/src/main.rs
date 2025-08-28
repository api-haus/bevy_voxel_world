mod atmosphere;

mod camera;

mod diag;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
	voxel_demo_app::run();
}

#[cfg(target_arch = "wasm32")]
fn main() {
	use wasm_bindgen_futures::{JsFuture, spawn_local};

	spawn_local(async move {
		let threads = std::thread::available_parallelism()
			.map(|n| n.get())
			.unwrap_or(4);

		let promise = wasm_bindgen_rayon::init_thread_pool(threads);
		let _ = JsFuture::from(promise).await;

		voxel_demo_app::run();
	});
}
