// Triplanar PBR shader for voxel terrain
// Supports 4 texture layers with per-vertex blend weights

#import bevy_pbr::{
    mesh_functions,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions,
    pbr_types,
    mesh_view_bindings::view,
}

// Custom vertex input with material weights
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) material_weights: vec4<f32>,
};

// Interpolated data passed to fragment shader
struct TriplanarVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) material_weights: vec4<f32>,
};

// Triplanar parameters uniform
struct TriplanarParams {
    texture_scales: vec4<f32>,
    blend_sharpness: f32,
    normal_strength: f32,
    _padding: vec2<f32>,
};

// Bind groups
@group(2) @binding(0) var albedo_0: texture_2d<f32>;
@group(2) @binding(1) var albedo_0_sampler: sampler;
@group(2) @binding(2) var normal_0: texture_2d<f32>;
@group(2) @binding(3) var normal_0_sampler: sampler;
@group(2) @binding(4) var arm_0: texture_2d<f32>;
@group(2) @binding(5) var arm_0_sampler: sampler;

@group(2) @binding(6) var albedo_1: texture_2d<f32>;
@group(2) @binding(7) var albedo_1_sampler: sampler;
@group(2) @binding(8) var normal_1: texture_2d<f32>;
@group(2) @binding(9) var normal_1_sampler: sampler;
@group(2) @binding(10) var arm_1: texture_2d<f32>;
@group(2) @binding(11) var arm_1_sampler: sampler;

@group(2) @binding(12) var albedo_2: texture_2d<f32>;
@group(2) @binding(13) var albedo_2_sampler: sampler;
@group(2) @binding(14) var normal_2: texture_2d<f32>;
@group(2) @binding(15) var normal_2_sampler: sampler;
@group(2) @binding(16) var arm_2: texture_2d<f32>;
@group(2) @binding(17) var arm_2_sampler: sampler;

@group(2) @binding(18) var albedo_3: texture_2d<f32>;
@group(2) @binding(19) var albedo_3_sampler: sampler;
@group(2) @binding(20) var normal_3: texture_2d<f32>;
@group(2) @binding(21) var normal_3_sampler: sampler;
@group(2) @binding(22) var arm_3: texture_2d<f32>;
@group(2) @binding(23) var arm_3_sampler: sampler;

@group(2) @binding(24) var<uniform> params: TriplanarParams;

@vertex
fn vertex(vertex: Vertex) -> TriplanarVertexOutput {
    var out: TriplanarVertexOutput;

    // Get mesh transforms
    let model = mesh_functions::get_world_from_local(vertex.instance_index);

    // Transform to world space
    let world_position = model * vec4<f32>(vertex.position, 1.0);
    let world_normal = normalize((model * vec4<f32>(vertex.normal, 0.0)).xyz);

    // Transform to clip space
    out.clip_position = view.clip_from_world * world_position;
    out.world_position = world_position;
    out.world_normal = world_normal;
    out.material_weights = vertex.material_weights;

    return out;
}

// Compute triplanar blend weights from normal
fn triplanar_weights(normal: vec3<f32>, sharpness: f32) -> vec3<f32> {
    var weights = abs(normal);
    weights = pow(weights, vec3<f32>(sharpness));
    weights = weights / (weights.x + weights.y + weights.z);
    return weights;
}

// Sample texture with triplanar projection
fn sample_triplanar(
    tex: texture_2d<f32>,
    tex_sampler: sampler,
    world_pos: vec3<f32>,
    weights: vec3<f32>,
    scale: f32
) -> vec4<f32> {
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;

    let sample_x = textureSample(tex, tex_sampler, uv_x);
    let sample_y = textureSample(tex, tex_sampler, uv_y);
    let sample_z = textureSample(tex, tex_sampler, uv_z);

    return sample_x * weights.x + sample_y * weights.y + sample_z * weights.z;
}

// Sample and blend normal map with triplanar projection
fn sample_triplanar_normal(
    tex: texture_2d<f32>,
    tex_sampler: sampler,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    weights: vec3<f32>,
    scale: f32,
    strength: f32
) -> vec3<f32> {
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;

    // Sample normal maps (assuming DXT5nm or standard RGB normals)
    var normal_x = textureSample(tex, tex_sampler, uv_x).xyz * 2.0 - 1.0;
    var normal_y = textureSample(tex, tex_sampler, uv_y).xyz * 2.0 - 1.0;
    var normal_z = textureSample(tex, tex_sampler, uv_z).xyz * 2.0 - 1.0;

    // Apply strength
    normal_x.xy *= strength;
    normal_y.xy *= strength;
    normal_z.xy *= strength;

    // Swizzle normals to world space orientation
    // X projection: normal is in ZY plane
    let tn_x = vec3<f32>(normal_x.z, normal_x.y, normal_x.x);
    // Y projection: normal is in XZ plane
    let tn_y = vec3<f32>(normal_y.x, normal_y.z, normal_y.y);
    // Z projection: normal is in XY plane
    let tn_z = vec3<f32>(normal_z.x, normal_z.y, normal_z.z);

    // Blend and normalize
    let blended = tn_x * weights.x + tn_y * weights.y + tn_z * weights.z;
    return normalize(blended + world_normal);
}

// Sample a single layer with triplanar projection
fn sample_layer(
    albedo_tex: texture_2d<f32>,
    albedo_samp: sampler,
    normal_tex: texture_2d<f32>,
    normal_samp: sampler,
    arm_tex: texture_2d<f32>,
    arm_samp: sampler,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    tri_weights: vec3<f32>,
    scale: f32,
    normal_strength: f32,
) -> LayerSample {
    var sample: LayerSample;
    sample.albedo = sample_triplanar(albedo_tex, albedo_samp, world_pos, tri_weights, scale);
    sample.normal = sample_triplanar_normal(normal_tex, normal_samp, world_pos, world_normal, tri_weights, scale, normal_strength);
    sample.arm = sample_triplanar(arm_tex, arm_samp, world_pos, tri_weights, scale);
    return sample;
}

struct LayerSample {
    albedo: vec4<f32>,
    normal: vec3<f32>,
    arm: vec4<f32>,  // AO, Roughness, Metallic
};

@fragment
fn fragment(in: TriplanarVertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let world_normal = normalize(in.world_normal);
    let mat_weights = in.material_weights;

    // Compute triplanar blend weights
    let tri_weights = triplanar_weights(world_normal, params.blend_sharpness);

    // Sample all 4 layers
    let layer0 = sample_layer(
        albedo_0, albedo_0_sampler,
        normal_0, normal_0_sampler,
        arm_0, arm_0_sampler,
        world_pos, world_normal, tri_weights,
        params.texture_scales.x, params.normal_strength
    );
    let layer1 = sample_layer(
        albedo_1, albedo_1_sampler,
        normal_1, normal_1_sampler,
        arm_1, arm_1_sampler,
        world_pos, world_normal, tri_weights,
        params.texture_scales.y, params.normal_strength
    );
    let layer2 = sample_layer(
        albedo_2, albedo_2_sampler,
        normal_2, normal_2_sampler,
        arm_2, arm_2_sampler,
        world_pos, world_normal, tri_weights,
        params.texture_scales.z, params.normal_strength
    );
    let layer3 = sample_layer(
        albedo_3, albedo_3_sampler,
        normal_3, normal_3_sampler,
        arm_3, arm_3_sampler,
        world_pos, world_normal, tri_weights,
        params.texture_scales.w, params.normal_strength
    );

    // Blend layers by material weights
    let albedo = layer0.albedo * mat_weights.x
               + layer1.albedo * mat_weights.y
               + layer2.albedo * mat_weights.z
               + layer3.albedo * mat_weights.w;

    let normal = normalize(
        layer0.normal * mat_weights.x
      + layer1.normal * mat_weights.y
      + layer2.normal * mat_weights.z
      + layer3.normal * mat_weights.w
    );

    let arm = layer0.arm * mat_weights.x
            + layer1.arm * mat_weights.y
            + layer2.arm * mat_weights.z
            + layer3.arm * mat_weights.w;

    // Extract PBR properties from ARM texture
    let ao = arm.r;
    let roughness = arm.g;
    let metallic = arm.b;

    // Simple lighting (directional + ambient)
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let n_dot_l = max(dot(normal, light_dir), 0.0);

    // View direction for specular
    let view_dir = normalize(view.world_position.xyz - world_pos);
    let half_dir = normalize(light_dir + view_dir);
    let n_dot_h = max(dot(normal, half_dir), 0.0);

    // Simple PBR approximation
    let diffuse = albedo.rgb * (1.0 - metallic);
    let specular_color = mix(vec3<f32>(0.04), albedo.rgb, metallic);
    let specular_power = mix(8.0, 256.0, 1.0 - roughness);
    let specular = specular_color * pow(n_dot_h, specular_power);

    // Combine lighting
    let ambient = vec3<f32>(0.1, 0.12, 0.15) * ao;
    let lit_color = diffuse * n_dot_l + specular * n_dot_l + ambient * albedo.rgb;

    // Tone mapping (simple Reinhard)
    let mapped = lit_color / (lit_color + vec3<f32>(1.0));

    return vec4<f32>(mapped, 1.0);
}
