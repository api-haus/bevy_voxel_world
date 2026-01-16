use voxel_noise::{NoiseNode, presets};

fn main() {
    // Test with SIMPLE_TERRAIN
    println!("Testing SIMPLE_TERRAIN preset...");
    let node = NoiseNode::from_encoded(presets::SIMPLE_TERRAIN).unwrap();
    let mut output = vec![0.0f32; 8];
    node.gen_uniform_grid_3d(&mut output, 0.0, 0.0, 0.0, 2, 2, 2, 1.0, 1.0, 1.0, 1337);
    println!("SIMPLE_TERRAIN output: {:?}", output);
    
    // Try a basic OpenSimplex2 noise preset (simpler)
    // This is just FBm OpenSimplex2
    let simplex_preset = "DQAFAAAAAAAAQAgAAAAAAD8AAAAAAA==";
    if let Some(node2) = NoiseNode::from_encoded(simplex_preset) {
        node2.gen_uniform_grid_3d(&mut output, 0.0, 0.0, 0.0, 2, 2, 2, 0.1, 0.1, 0.1, 1337);
        println!("Basic Simplex output: {:?}", output);
    } else {
        println!("Basic Simplex preset failed to parse");
    }
    
    // Try 2D mode
    println!("Testing 2D grid...");
    let mut output_2d = vec![0.0f32; 4];
    node.gen_uniform_grid_2d(&mut output_2d, 0.0, 0.0, 2, 2, 0.1, 0.1, 1337);
    println!("2D output: {:?}", output_2d);
}
