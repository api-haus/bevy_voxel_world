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

// Extension bindings for albedo array and parameters
@group(2) @binding(100)
var albedo_array: texture_2d_array<f32>;
@group(2) @binding(101)
var albedo_array_sampler: sampler;
@group(2) @binding(102)
var<uniform> triplanar_tiling_scale: f32;
@group(2) @binding(103)
var<uniform> albedo_layer_count: u32;

// Small MVP arrays for per-material params
const TINT_COUNT: u32 = 8u;
const SCALE_COUNT: u32 = 4u;
const TINTS: array<vec3<f32>, 8u> = array<vec3<f32>, 8u>(
    vec3<f32>(1.00, 1.00, 1.00),
    vec3<f32>(1.00, 0.95, 0.90),
    vec3<f32>(0.95, 1.00, 0.95),
    vec3<f32>(0.95, 0.95, 1.00),
    vec3<f32>(1.00, 0.90, 0.90),
    vec3<f32>(0.90, 1.00, 1.00),
    vec3<f32>(0.90, 0.90, 1.00),
    vec3<f32>(1.00, 1.00, 0.90),
);
const SCALES: array<f32, 4u> = array<f32, 4u>(0.06, 0.08, 0.10, 0.12);

// Triplanar texture sampling helper
fn triplanar_sample_linear(
    map: texture_2d_array<f32>,
    samp: sampler,
    pos: vec3<f32>,
    normal: vec3<f32>,
    scale: f32,
    layer: i32,
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
        out_color = out_color + textureSample(map, samp, uvx, layer) * weights.x;
    }
    if weights.y > 0.01 {
        out_color = out_color + textureSample(map, samp, uvy, layer) * weights.y;
    }
    if weights.z > 0.01 {
        out_color = out_color + textureSample(map, samp, uvz, layer) * weights.z;
    }
    return out_color;
}

fn hash_to_rgb(n: u32) -> vec3<f32> {
    // Simple integer hash to color
    var x = n;
    x = (x ^ (x >> 16u)) * 0x7feb352du;
    x = (x ^ (x >> 15u)) * 0x846ca68bu;
    x = (x ^ (x >> 16u));
    let r = f32((x & 0xFFu)) / 255.0;
    let g = f32(((x >> 8u) & 0xFFu)) / 255.0;
    let b = f32(((x >> 16u) & 0xFFu)) / 255.0;
    return vec3<f32>(r, g, b);
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

    // Decode material id from vertex color red channel
    let mat_id: u32 = u32(round(clamp(in.color.r, 0.0, 1.0) * 255.0));

    // MVP params by mat_id
    let tint = TINTS[mat_id % TINT_COUNT];
    let scale = SCALES[mat_id % SCALE_COUNT] * triplanar_tiling_scale;

    // Triplanar blend using world-space position and normal
    let pos = in.world_position.xyz;
    let n = normalize(pbr_input.world_normal);
    let layer = i32(mat_id % albedo_layer_count);
    let tri = triplanar_sample_linear(albedo_array, albedo_array_sampler, pos, n, scale, layer);

#ifdef DEBUG_MAT_VIS
    let dbg_rgb = hash_to_rgb(mat_id);
    pbr_input.material.base_color = vec4<f32>(dbg_rgb, pbr_input.material.base_color.a);
#else
    // Override base color with triplanar result, modulate by tint, preserve alpha
    pbr_input.material.base_color = vec4<f32>(tri.rgb * tint, pbr_input.material.base_color.a);
#endif

    // apply lighting
    out.color = apply_pbr_lighting(pbr_input);

    // apply in-shader post processing (fog, alpha-premultiply, and also tonemapping, debanding if the camera is non-hdr)
    // note this does not include fullscreen postprocessing effects like bloom.
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
