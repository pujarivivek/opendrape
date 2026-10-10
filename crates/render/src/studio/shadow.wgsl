// Shadow maps: depth from the key light, and the floor's contact map (how close something is
// above each spot of floor), blurred so the contact shadow is soft.

struct Light {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> light: Light;

@vertex
fn vs_depth(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>) -> @builtin(position) vec4<f32> {
    return light.view_proj * vec4<f32>(position, 1.0);
}

// The key map's depth (0..1) in 24 bits of an 8-bit RGB texture, beside the depth buffer:
// OpenGL can't read a depth texture as numbers, and the soft shadows need to know how far
// above a spot the thing shading it is.
fn pack_unit(x: f32) -> vec3<f32> {
    let a = clamp(x, 0.0, 1.0) * 255.0;
    let r = floor(a);
    let b = fract(a) * 255.0;
    let g = floor(b);
    return vec3<f32>(r, g, round(fract(b) * 255.0)) / 255.0;
}

@fragment
fn fs_key(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(pack_unit(position.z), 1.0);
}

// Seen from just under the floor looking up, depth 0 is the floor and 1 is the top of the
// range: darkness is how close the nearest thing above is.
@fragment
fn fs_contact(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0 - position.z, 0.0, 0.0, 1.0);
}

struct Blur {
    // xy: the step between taps, in texture coordinates.
    step: vec4<f32>,
};
@group(0) @binding(0) var<uniform> blur: Blur;
@group(0) @binding(1) var blur_source: texture_2d<f32>;
@group(0) @binding(2) var blur_sampler: sampler;

@vertex
fn vs_blur(@builtin(vertex_index) i: u32) -> Fullscreen {
    return fullscreen(i, 0.0);
}

// A 9-tap Gaussian along one direction.
@fragment
fn fs_blur(v: Fullscreen) -> @location(0) vec4<f32> {
    let uv = v.ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    var sum = 0.0;
    var total = 0.0;
    for (var i = -4; i <= 4; i++) {
        let w = exp(-f32(i * i) / 8.0);
        sum += w * textureSampleLevel(blur_source, blur_sampler, uv + f32(i) * blur.step.xy, 0.0).r;
        total += w;
    }
    return vec4<f32>(sum / total, 0.0, 0.0, 1.0);
}
