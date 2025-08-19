#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

// Extension bindings for albedo map and tiling scale
@group(2) @binding(100)
var triplanar_albedo: texture_2d<f32>;
@group(2) @binding(101)
var triplanar_albedo_sampler: sampler;
@group(2) @binding(102)
var<uniform> triplanar_tiling_scale: f32;

// Triplanar texture sampling helper
fn triplanar_sample_linear(
    map: texture_2d<f32>,
    samp: sampler,
    pos: vec3<f32>,
    normal: vec3<f32>,
    scale: f32,
) -> vec4<f32> {
    let a = abs(normal);
    let wx = pow(a.x, 8.0);
    let wy = pow(a.y, 8.0);
    let wz = pow(a.z, 8.0);
    let sum = max(wx + wy + wz, 1e-5);
    let weights = vec3<f32>(wx, wy, wz) / sum;

    // Projected UVs
    let uvx = fract(pos.yz * scale);
    let uvy = fract(pos.zx * scale);
    let uvz = fract(pos.xy * scale);

    var out_color = vec4<f32>(0.0);
    if weights.x > 0.01 {
        out_color = out_color + textureSample(map, samp, uvx) * weights.x;
    }
    if weights.y > 0.01 {
        out_color = out_color + textureSample(map, samp, uvy) * weights.y;
    }
    if weights.z > 0.01 {
        out_color = out_color + textureSample(map, samp, uvz) * weights.z;
    }
    return out_color;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // generate a PbrInput struct from the StandardMaterial bindings
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // alpha discard first
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    // in deferred mode we can't modify anything after that, as lighting is run in a separate fullscreen shader.
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    // Triplanar blend using world-space position and normal
    let pos = in.world_position.xyz;
    let n = normalize(pbr_input.world_normal);
    let tri = triplanar_sample_linear(triplanar_albedo, triplanar_albedo_sampler, pos, n, triplanar_tiling_scale);

    // Override base color with triplanar result, preserve alpha
    pbr_input.material.base_color = vec4<f32>(tri.rgb, pbr_input.material.base_color.a);

    // apply lighting
    out.color = apply_pbr_lighting(pbr_input);

    // apply in-shader post processing (fog, alpha-premultiply, and also tonemapping, debanding if the camera is non-hdr)
    // note this does not include fullscreen postprocessing effects like bloom.
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
