// The last passes: averaging still frames, and the drawn image (or that average) to the 8-bit
// sRGB texture egui shows, through exposure and PBR Neutral tone mapping, with light edge
// smoothing (FXAA) while things move.

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

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

@vertex
fn vs_output(@builtin(vertex_index) i: u32) -> Fullscreen {
    return fullscreen(i, 0.0);
}

// FXAA-style smoothing: where the four neighbours show a contrasty edge, blend a little
// towards the neighbour across it, more where the pixel stands out from its neighbourhood.
@fragment
fn fs_output(v: Fullscreen) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(v.clip.xy));
    let c = display(p);
    if (output.flags.y == 0u) {
        return vec4<f32>(c, 1.0);
    }
    let n = display(p + vec2<i32>(0, -1));
    let s = display(p + vec2<i32>(0, 1));
    let w = display(p + vec2<i32>(-1, 0));
    let e = display(p + vec2<i32>(1, 0));
    let lc = luma(c);
    let ln = luma(n);
    let ls = luma(s);
    let lw = luma(w);
    let le = luma(e);
    let lo = min(lc, min(min(ln, ls), min(lw, le)));
    let hi = max(lc, max(max(ln, ls), max(lw, le)));
    let range = hi - lo;
    if (range < max(0.0312, hi * 0.125)) {
        return vec4<f32>(c, 1.0);
    }
    let across_rows = abs(ln + ls - 2.0 * lc) >= abs(lw + le - 2.0 * lc);
    var other = w;
    if (across_rows) {
        other = select(s, n, abs(ln - lc) > abs(ls - lc));
    } else {
        other = select(e, w, abs(lw - lc) > abs(le - lc));
    }
    let stands_out = clamp(abs((ln + ls + lw + le) * 0.25 - lc) / range, 0.0, 1.0);
    let blend = smoothstep(0.0, 1.0, stands_out);
    return vec4<f32>(mix(c, other, 0.5 * blend * blend + 0.25), 1.0);
}

struct Accumulate {
    // x: the new frame's weight, 1 / (frames so far + 1)
    weight: vec4<f32>,
};
@group(0) @binding(0) var<uniform> accumulate: Accumulate;
@group(0) @binding(1) var previous: texture_2d<f32>;
@group(0) @binding(2) var latest: texture_2d<f32>;

// The running average of the still frames (each one shifted a fraction of a pixel).
@fragment
fn fs_accumulate(v: Fullscreen) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(v.clip.xy));
    let before = textureLoad(previous, p, 0);
    let fresh = textureLoad(latest, p, 0);
    return mix(before, fresh, accumulate.weight.x);
}
