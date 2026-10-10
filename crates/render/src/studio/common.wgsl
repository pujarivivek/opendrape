// Shared by the studio shaders: colour output and the full-screen triangle.

const PI: f32 = 3.14159265;

// Khronos PBR Neutral tone mapping (2024): colours below the highlights come out as they went
// in, less the 4 % a dielectric reflects; highlights are compressed keeping their hue.
fn pbr_neutral(colour: vec3<f32>) -> vec3<f32> {
    let start = 0.8 - 0.04;
    let desaturation = 0.15;
    let x = min(colour.r, min(colour.g, colour.b));
    var offset = 0.04;
    if (x < 0.08) {
        offset = x - 6.25 * x * x;
    }
    var c = colour - offset;
    let peak = max(c.r, max(c.g, c.b));
    if (peak < start) {
        return c;
    }
    let d = 1.0 - start;
    let new_peak = 1.0 - d * d / (peak + d - start);
    c = c * (new_peak / peak);
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return mix(c, vec3<f32>(new_peak), g);
}

// The exact sRGB curve (the 3D image is an 8-bit texture egui shows as it is).
fn encode_srgb(linear: vec3<f32>) -> vec3<f32> {
    let c = clamp(linear, vec3<f32>(0.0), vec3<f32>(1.0));
    let low = c * 12.92;
    let high = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

struct Fullscreen {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

// One triangle covering the screen, at depth `z` (no vertex buffer: 3 vertices).
fn fullscreen(i: u32, z: f32) -> Fullscreen {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    let ndc = uv * 2.0 - 1.0;
    var out: Fullscreen;
    out.clip = vec4<f32>(ndc, z, 1.0);
    out.ndc = ndc;
    return out;
}
