// The last pass: the drawn image (or the average of still frames) to the 8-bit sRGB texture
// egui shows, through exposure and PBR Neutral tone mapping.

struct Output {
    // x exposure
    params: vec4<f32>,
    // x the source is already tone-mapped and encoded (LDR path), y FXAA
    flags: vec4<u32>,
};
@group(0) @binding(0) var<uniform> output: Output;
@group(0) @binding(1) var source: texture_2d<f32>;

fn display(p: vec2<i32>) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(source));
    let c = textureLoad(source, clamp(p, vec2<i32>(0), size - 1), 0).rgb;
    if (output.flags.x != 0u) {
        return c;
    }
    return encode_srgb(pbr_neutral(c * output.params.x));
}

@vertex
fn vs_output(@builtin(vertex_index) i: u32) -> Fullscreen {
    return fullscreen(i, 0.0);
}

@fragment
fn fs_output(v: Fullscreen) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(v.clip.xy));
    return vec4<f32>(display(p), 1.0);
}
