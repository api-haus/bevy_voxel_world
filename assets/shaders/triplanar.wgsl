// Triplanar PBR shader for voxel terrain
// Standalone Material implementation with custom PBR lighting
// Material blend weights are read from vertex colors (RGBA = 4 layer weights)
//
// Channel packing:
// - Diffuse array (4 layers): RGB=Diffuse, A=Height
// - Normal array (4 layers): RGB=Normal
// - Mask array (4 layers): R=Roughness, G=Metallic, B=AO

#import bevy_pbr::mesh_functions

// Use Bevy's view bindings for camera data
#import bevy_pbr::mesh_view_bindings::view

// Triplanar parameters uniform
struct TriplanarParams {
    texture_scale: f32,
    blend_sharpness: f32,
    normal_strength: f32,
    _padding: f32,
}

// Material bind group (group 2) - bindings 0-6
@group(2) @binding(0) var diffuse_array: texture_2d_array<f32>;
@group(2) @binding(1) var diffuse_sampler: sampler;
@group(2) @binding(2) var normal_array: texture_2d_array<f32>;
@group(2) @binding(3) var normal_sampler: sampler;
@group(2) @binding(4) var mask_array: texture_2d_array<f32>;
@group(2) @binding(5) var mask_sampler: sampler;
@group(2) @binding(6) var<uniform> triplanar_params: TriplanarParams;

// Custom vertex input - uses standard Bevy mesh attributes
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Material blend weights from vertex color (RGBA = 4 layer weights)
    @location(5) color: vec4<f32>,
}

// Interpolated data passed to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) material_weights: vec4<f32>,
}

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

// Sample and blend normal with triplanar projection
fn sample_triplanar_normal(
    tex: texture_2d_array<f32>,
    tex_sampler: sampler,
    layer: i32,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    tri_weights: vec3<f32>,
    scale: f32,
    strength: f32
) -> vec3<f32> {
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;

    // Sample and unpack normals from [0,1] to [-1,1]
    var tnormal_x = textureSample(tex, tex_sampler, uv_x, layer).xyz * 2.0 - 1.0;
    var tnormal_y = textureSample(tex, tex_sampler, uv_y, layer).xyz * 2.0 - 1.0;
    var tnormal_z = textureSample(tex, tex_sampler, uv_z, layer).xyz * 2.0 - 1.0;

    // Apply strength
    tnormal_x = vec3<f32>(tnormal_x.xy * strength, tnormal_x.z);
    tnormal_y = vec3<f32>(tnormal_y.xy * strength, tnormal_y.z);
    tnormal_z = vec3<f32>(tnormal_z.xy * strength, tnormal_z.z);

    // Get axis signs for correct orientation
    let axis_sign = sign(world_normal);

    // Swizzle to world space
    let wn_x = vec3<f32>(tnormal_x.z * axis_sign.x, tnormal_x.y, tnormal_x.x);
    let wn_y = vec3<f32>(tnormal_y.x, tnormal_y.z * axis_sign.y, tnormal_y.y);
    let wn_z = vec3<f32>(tnormal_z.x, tnormal_z.y, tnormal_z.z * axis_sign.z);

    // Blend and add to geometry normal
    let blended = wn_x * tri_weights.x + wn_y * tri_weights.y + wn_z * tri_weights.z;
    return normalize(world_normal + blended);
}

// Sample all 4 layers and blend by material weights
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

fn sample_all_layers_normal(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    tri_weights: vec3<f32>,
    mat_weights: vec4<f32>,
    scale: f32,
    strength: f32
) -> vec3<f32> {
    var result = vec3<f32>(0.0);

    if mat_weights.x > 0.001 {
        result += sample_triplanar_normal(normal_array, normal_sampler, 0, world_pos, world_normal, tri_weights, scale, strength) * mat_weights.x;
    }
    if mat_weights.y > 0.001 {
        result += sample_triplanar_normal(normal_array, normal_sampler, 1, world_pos, world_normal, tri_weights, scale, strength) * mat_weights.y;
    }
    if mat_weights.z > 0.001 {
        result += sample_triplanar_normal(normal_array, normal_sampler, 2, world_pos, world_normal, tri_weights, scale, strength) * mat_weights.z;
    }
    if mat_weights.w > 0.001 {
        result += sample_triplanar_normal(normal_array, normal_sampler, 3, world_pos, world_normal, tri_weights, scale, strength) * mat_weights.w;
    }

    return normalize(result);
}

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

// PBR lighting functions
const PI: f32 = 3.14159265359;

// GGX normal distribution
fn D_GGX(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// Schlick-GGX geometry function
fn G_SchlickGGX(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// Smith's method for geometry
fn G_Smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return G_SchlickGGX(n_dot_v, roughness) * G_SchlickGGX(n_dot_l, roughness);
}

// Fresnel-Schlick approximation
fn F_Schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let model = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = model * vec4<f32>(vertex.position, 1.0);
    let world_normal = normalize((model * vec4<f32>(vertex.normal, 0.0)).xyz);

    out.clip_position = view.clip_from_world * world_position;
    out.world_position = world_position;
    out.world_normal = world_normal;
    // Material weights from vertex color
    out.material_weights = vertex.color;

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let world_normal = normalize(in.world_normal);
    let mat_weights = in.material_weights;

    // Compute triplanar blend weights
    let tri_weights = triplanar_weights(world_normal, triplanar_params.blend_sharpness);

    // Sample all texture arrays
    let diffuse_sample = sample_all_layers_diffuse(world_pos, tri_weights, mat_weights, triplanar_params.texture_scale);
    let normal_sample = sample_all_layers_normal(world_pos, world_normal, tri_weights, mat_weights, triplanar_params.texture_scale, triplanar_params.normal_strength);
    let mask_sample = sample_all_layers_mask(world_pos, tri_weights, mat_weights, triplanar_params.texture_scale);

    let albedo = diffuse_sample.rgb;
    let roughness = mask_sample.r;
    let metallic = mask_sample.g;
    let ao = mask_sample.b;

    // PBR setup
    let N = normal_sample;
    let V = normalize(view.world_position.xyz - world_pos);
    let n_dot_v = max(dot(N, V), 0.0001);

    // Base reflectivity (F0)
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    // Directional light
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let light_color = vec3<f32>(1.0, 0.98, 0.95) * 2.5;

    let L = light_dir;
    let H = normalize(V + L);
    let n_dot_l = max(dot(N, L), 0.0);
    let n_dot_h = max(dot(N, H), 0.0);
    let v_dot_h = max(dot(V, H), 0.0);

    // Cook-Torrance BRDF
    let D = D_GGX(n_dot_h, roughness);
    let G = G_Smith(n_dot_v, n_dot_l, roughness);
    let F = F_Schlick(v_dot_h, f0);

    let numerator = D * G * F;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;

    // Energy conservation
    let ks = F;
    let kd = (1.0 - ks) * (1.0 - metallic);

    // Direct lighting
    let diffuse = kd * albedo / PI;
    let direct = (diffuse + specular) * light_color * n_dot_l;

    // Ambient lighting (simple hemisphere)
    let sky_color = vec3<f32>(0.4, 0.5, 0.7);
    let ground_color = vec3<f32>(0.15, 0.12, 0.1);
    let hemisphere_factor = N.y * 0.5 + 0.5;
    let ambient_light = mix(ground_color, sky_color, hemisphere_factor) * 0.3;
    let ambient = ambient_light * albedo * ao;

    // Final color
    let lit_color = direct + ambient;

    // Reinhard tone mapping
    let mapped = lit_color / (lit_color + 1.0);

    // Gamma correction
    let gamma_corrected = pow(mapped, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(gamma_corrected, 1.0);
}
