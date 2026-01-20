// Triplanar PBR extension shader for voxel terrain
// Extends StandardMaterial with triplanar texture array sampling.
// Vertex colors are used as 4-layer material blend weights.

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

// Triplanar parameters
struct TriplanarParams {
    texture_scale: f32,
    blend_sharpness: f32,
    normal_strength: f32,
    _padding: f32,
}

// Extension bindings - Bevy 0.18 uses group 3 for materials
// Using 100+ to avoid StandardMaterial binding conflicts
@group(3) @binding(100) var diffuse_array: texture_2d_array<f32>;
@group(3) @binding(101) var diffuse_sampler: sampler;
@group(3) @binding(102) var normal_array: texture_2d_array<f32>;
@group(3) @binding(103) var normal_sampler: sampler;
@group(3) @binding(104) var mask_array: texture_2d_array<f32>;
@group(3) @binding(105) var mask_sampler: sampler;
@group(3) @binding(106) var<uniform> triplanar_params: TriplanarParams;

// Compute triplanar blend weights from world normal
fn triplanar_weights(normal: vec3<f32>, sharpness: f32) -> vec3<f32> {
    var weights = abs(normal);
    weights = pow(weights, vec3<f32>(sharpness));
    let sum = weights.x + weights.y + weights.z;
    return weights / max(sum, 0.0001);
}

// Sample texture array with triplanar projection for a single layer
fn sample_triplanar_layer(
    tex: texture_2d_array<f32>,
    tex_sampler: sampler,
    layer: i32,
    world_pos: vec3<f32>,
    tri_weights: vec3<f32>,
    scale: f32
) -> vec4<f32> {
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;

    let sample_x = textureSample(tex, tex_sampler, uv_x, layer);
    let sample_y = textureSample(tex, tex_sampler, uv_y, layer);
    let sample_z = textureSample(tex, tex_sampler, uv_z, layer);

    return sample_x * tri_weights.x + sample_y * tri_weights.y + sample_z * tri_weights.z;
}

// Sample and blend all 4 layers for diffuse
fn sample_all_layers_diffuse(
    world_pos: vec3<f32>,
    tri_weights: vec3<f32>,
    mat_weights: vec4<f32>,
    scale: f32
) -> vec4<f32> {
    var result = vec4<f32>(0.0);

    if mat_weights.x > 0.001 {
        result += sample_triplanar_layer(diffuse_array, diffuse_sampler, 0, world_pos, tri_weights, scale) * mat_weights.x;
    }
    if mat_weights.y > 0.001 {
        result += sample_triplanar_layer(diffuse_array, diffuse_sampler, 1, world_pos, tri_weights, scale) * mat_weights.y;
    }
    if mat_weights.z > 0.001 {
        result += sample_triplanar_layer(diffuse_array, diffuse_sampler, 2, world_pos, tri_weights, scale) * mat_weights.z;
    }
    if mat_weights.w > 0.001 {
        result += sample_triplanar_layer(diffuse_array, diffuse_sampler, 3, world_pos, tri_weights, scale) * mat_weights.w;
    }

    return result;
}

// Sample and blend all 4 layers for mask (roughness, metallic, ao)
fn sample_all_layers_mask(
    world_pos: vec3<f32>,
    tri_weights: vec3<f32>,
    mat_weights: vec4<f32>,
    scale: f32
) -> vec3<f32> {
    var result = vec3<f32>(0.0);

    if mat_weights.x > 0.001 {
        result += sample_triplanar_layer(mask_array, mask_sampler, 0, world_pos, tri_weights, scale).rgb * mat_weights.x;
    }
    if mat_weights.y > 0.001 {
        result += sample_triplanar_layer(mask_array, mask_sampler, 1, world_pos, tri_weights, scale).rgb * mat_weights.y;
    }
    if mat_weights.z > 0.001 {
        result += sample_triplanar_layer(mask_array, mask_sampler, 2, world_pos, tri_weights, scale).rgb * mat_weights.z;
    }
    if mat_weights.w > 0.001 {
        result += sample_triplanar_layer(mask_array, mask_sampler, 3, world_pos, tri_weights, scale).rgb * mat_weights.w;
    }

    return result;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Generate PBR input from StandardMaterial
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Get world position and normal for triplanar sampling
    let world_pos = in.world_position.xyz;
    let world_normal = normalize(in.world_normal);

    // Get material blend weights from vertex color
#ifdef VERTEX_COLORS
    let mat_weights = in.color;
#else
    // Fallback: use first layer only
    let mat_weights = vec4<f32>(1.0, 0.0, 0.0, 0.0);
#endif

    // Compute triplanar blend weights
    let tri_weights = triplanar_weights(world_normal, triplanar_params.blend_sharpness);

    // Sample texture arrays
    let diffuse_sample = sample_all_layers_diffuse(world_pos, tri_weights, mat_weights, triplanar_params.texture_scale);
    let mask_sample = sample_all_layers_mask(world_pos, tri_weights, mat_weights, triplanar_params.texture_scale);

    // Override PBR inputs with triplanar-sampled values
    pbr_input.material.base_color = diffuse_sample;
    pbr_input.material.perceptual_roughness = mask_sample.r;
    pbr_input.material.metallic = mask_sample.g;
    // Note: AO (mask_sample.b) would need to be applied differently in Bevy's PBR

    // Alpha discard (respects StandardMaterial alpha settings)
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
