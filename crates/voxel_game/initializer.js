// Trunk initializer - runs before main WASM module loads
// Initializes the FastNoise2 Emscripten module via JS bridge

export default async function() {
    const loadingEl = document.getElementById('loading');

    // Import the JS bridge
    const bridge = await import('/voxel_noise_bridge.js');

    if (loadingEl) {
        loadingEl.textContent = 'Initializing FastNoise2...';
    }

    // Initialize the FastNoise2 Emscripten module (throws on failure)
    await bridge.vx_init();

    console.log('[initializer] FastNoise2 ready');

    // Hide loading indicator
    if (loadingEl) {
        loadingEl.style.display = 'none';
    }
}
