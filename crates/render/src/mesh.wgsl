struct Uniforms {
    view_proj: mat4x4<f32>,
    color: vec4<f32>,
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
fn fs_main(v: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Two-sided: cloth is seen from inside too.
    var n = normalize(v.normal);
    if (!front) {
        n = -n;
    }
    let key = max(dot(n, normalize(vec3<f32>(0.3, 0.8, 0.6))), 0.0);
    let fill = max(dot(n, normalize(vec3<f32>(-0.5, 0.2, -0.4))), 0.0);
    let sky = 0.5 + 0.5 * n.y;
    let light = 0.18 + 0.22 * sky + 0.6 * key + 0.15 * fill;
    let linear = u.color.rgb * light;
    // Target is Rgba8Unorm (egui requirement): encode to sRGB here.
    return vec4<f32>(pow(linear, vec3<f32>(1.0 / 2.2)), 1.0);
}
