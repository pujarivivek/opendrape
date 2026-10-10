// Soft darkening in folds (ambient occlusion): a prepass of depth and view-space normals,
// a hemisphere search at half (or full) resolution, and a depth-aware blur.

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    key_view_proj: mat4x4<f32>,
    contact_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    key_dir: vec4<f32>,
    key_colour: vec4<f32>,
    sh: array<vec4<f32>, 9>,
    horizon: vec4<f32>,
    top: vec4<f32>,
    params: vec4<f32>,
    flags: vec4<u32>,
    screen: vec4<f32>,
    extra: vec4<f32>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

// The search reaches this far (metres), and ignores occluders less than BIAS in front of a
// sample (depth is 32-bit float: precise enough for 1.5 mm at a few metres).
const RADIUS: f32 = 0.06;
const BIAS: f32 = 0.0015;
// Linear depth is packed into two 8-bit channels over this range (metres).
const DEPTH_RANGE: f32 = 20.0;

struct PrepassOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) distance: f32,
};

@vertex
fn vs_prepass(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>) -> PrepassOut {
    var out: PrepassOut;
    out.clip = frame.view_proj * vec4<f32>(position, 1.0);
    out.normal = (frame.view * vec4<f32>(normal, 0.0)).xyz;
    out.distance = -(frame.view * vec4<f32>(position, 1.0)).z;
    return out;
}

// Distance from the camera (metres) in 24 bits of an 8-bit RGB texture: OpenGL can't read a
// depth texture as numbers, so the prepass keeps its own copy.
fn pack_distance(d: f32) -> vec3<f32> {
    let x = clamp(d / DEPTH_RANGE, 0.0, 1.0) * 255.0;
    let r = floor(x);
    let y = fract(x) * 255.0;
    let g = floor(y);
    let b = round(fract(y) * 255.0);
    return vec3<f32>(r, g, b) / 255.0;
}

fn unpack_distance(c: vec3<f32>) -> f32 {
    let v = round(c * 255.0);
    return (v.r + (v.g + v.b / 255.0) / 255.0) / 255.0 * DEPTH_RANGE;
}

struct PrepassTargets {
    @location(0) normal: vec4<f32>,
    @location(1) distance: vec4<f32>,
};

@fragment
fn fs_prepass(v: PrepassOut, @builtin(front_facing) front: bool) -> PrepassTargets {
    var n = normalize(v.normal);
    if (!front) {
        n = -n;
    }
    var out: PrepassTargets;
    out.normal = vec4<f32>(n * 0.5 + 0.5, 1.0);
    out.distance = vec4<f32>(pack_distance(v.distance), 1.0);
    return out;
}

struct AoParams {
    // x: full-resolution pixels per AO pixel along each side (1 or 2).
    scale: vec4<i32>,
};
@group(1) @binding(0) var scene_distance: texture_2d<f32>;
@group(1) @binding(1) var scene_normals: texture_2d<f32>;
@group(1) @binding(2) var<uniform> ao_params: AoParams;

// View-space position of full-resolution pixel `p`, `distance` metres in front of the camera.
fn view_position(p: vec2<i32>, distance: f32) -> vec3<f32> {
    let uv = (vec2<f32>(p) + 0.5) * frame.screen.zw;
    let far = frame.inv_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 1.0, 1.0);
    let ray = far.xyz / far.w;
    return ray * (distance / -ray.z);
}

fn distance_at(p: vec2<i32>) -> f32 {
    return unpack_distance(textureLoad(scene_distance, p, 0).rgb);
}

fn pack_depth(linear: f32) -> vec2<f32> {
    let x = clamp(linear / DEPTH_RANGE, 0.0, 1.0) * 255.0;
    return vec2<f32>(floor(x) / 255.0, fract(x));
}

fn noise(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> Fullscreen {
    return fullscreen(i, 0.0);
}

@fragment
fn fs_ao(v: Fullscreen) -> @location(0) vec4<f32> {
    let full = vec2<i32>(textureDimensions(scene_distance));
    let here = vec2<i32>(floor(v.clip.xy));
    let p = min(here * ao_params.scale.x, full - 1);
    let d = distance_at(p);
    if (d >= DEPTH_RANGE * 0.999) {
        return vec4<f32>(1.0, pack_depth(DEPTH_RANGE), 1.0);
    }
    let centre = view_position(p, d);
    let n = normalize(textureLoad(scene_normals, p, 0).xyz * 2.0 - 1.0);
    // A frame round the normal, turned differently at each pixel (and each still frame).
    let angle = (noise(v.clip.xy) + frame.params.y * 0.618034) * 6.2831853;
    let seed = vec3<f32>(cos(angle), sin(angle), 0.37);
    let t = normalize(seed - n * dot(seed, n));
    let b = cross(n, t);
    let samples = max(i32(frame.extra.y), 1);
    var occlusion = 0.0;
    for (var i = 0; i < samples; i++) {
        let u = (f32(i) + 0.5) / f32(samples);
        let phi = f32(i) * 2.3999632;
        let r = sqrt(u);
        // Cosine-weighted directions; how far each reaches comes from a separate sequence, so
        // steep and shallow samples both reach near and far (more of them near).
        let v = fract(f32(i) * 0.618034 + 0.5);
        let reach = mix(0.15, 1.0, v * v) * RADIUS;
        let dir = t * (r * cos(phi)) + b * (r * sin(phi)) + n * sqrt(1.0 - u);
        let s = centre + dir * reach;
        let clip = frame.proj * vec4<f32>(s, 1.0);
        let ndc = clip.xy / clip.w;
        let at = vec2<i32>(floor(vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * vec2<f32>(full)));
        if (any(at < vec2<i32>(0)) || any(at >= full)) {
            continue;
        }
        let scene = view_position(at, distance_at(at)).z;
        // Something in front of the sample point (nearer the camera), close enough to count.
        let close = smoothstep(0.0, 1.0, RADIUS / max(abs(centre.z - scene), 1e-4));
        if (scene >= s.z + BIAS) {
            occlusion += close;
        }
    }
    // Squared: this estimate runs light in open folds (as hemisphere AO does); flat surfaces
    // stay at exactly 1.
    let open = 1.0 - occlusion / f32(samples);
    return vec4<f32>(open * open, pack_depth(-centre.z), 1.0);
}

struct Blur {
    // xy: one texel along the blur direction.
    step: vec4<i32>,
};
@group(1) @binding(0) var<uniform> blur: Blur;
@group(1) @binding(1) var blur_source: texture_2d<f32>;

fn unpack_depth(gb: vec2<f32>) -> f32 {
    return (gb.x * 255.0 + gb.y) / 255.0 * DEPTH_RANGE;
}

// Five taps along one direction, each weighted down the further its depth is from the
// centre's: the darkening doesn't bleed across edges.
@fragment
fn fs_blur(v: Fullscreen) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(blur_source));
    let here = vec2<i32>(floor(v.clip.xy));
    let centre = textureLoad(blur_source, here, 0);
    let depth = unpack_depth(centre.gb);
    var sum = 0.0;
    var total = 0.0;
    for (var i = -2; i <= 2; i++) {
        let at = clamp(here + blur.step.xy * i, vec2<i32>(0), size - 1);
        let tap = textureLoad(blur_source, at, 0);
        let w = exp(-f32(i * i) / 4.0) * exp(-abs(unpack_depth(tap.gb) - depth) / 0.02);
        sum += w * tap.r;
        total += w;
    }
    return vec4<f32>(sum / total, centre.gb, 1.0);
}
