struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = u.view_proj * vec4<f32>(v.position, 1.0);
    out.normal = v.normal;
    return out;
}

@fragment
fn fs_main(v: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(v.normal);
    let diffuse = max(dot(n, normalize(u.light_dir.xyz)), 0.0);
    let base = vec3<f32>(0.85, 0.45, 0.30);
    let linear = base * (0.25 + 0.75 * diffuse);
    // The target is Rgba8Unorm (egui requirement), so encode to sRGB here.
    return vec4<f32>(pow(linear, vec3<f32>(1.0 / 2.2)), 1.0);
}
