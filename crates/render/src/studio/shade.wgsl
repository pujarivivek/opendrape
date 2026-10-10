// The studio's main pass: the form, the cloth and the floor lit by the studio (soft SH light
// plus a soft key light), and the backdrop behind them.

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
    key_box: vec4<f32>,
    rim_dir: vec4<f32>,
    rim_colour: vec4<f32>,
    grid: vec4<f32>,
    grid_fade: vec4<f32>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

struct Draw {
    colour: vec4<f32>,
    material: vec4<u32>,
};
@group(1) @binding(0) var<uniform> draw: Draw;

@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var contact_map: texture_2d<f32>;
@group(2) @binding(3) var linear_sampler: sampler;
@group(2) @binding(4) var ao_map: texture_2d<f32>;
@group(2) @binding(5) var prepass_distance: texture_2d<f32>;

const CLOTH: u32 = 0u;
const FORM: u32 = 1u;
const LINING: f32 = 0.8;
// How much the folds' darkening also takes from the key light.
const AO_ON_KEY: f32 = 0.5;

// The soft studio light reaching a surface facing `n`, divided by π.
fn irradiance(n: vec3<f32>) -> vec3<f32> {
    var e = frame.sh[0].rgb * 0.282095;
    e += frame.sh[1].rgb * (0.488603 * n.y);
    e += frame.sh[2].rgb * (0.488603 * n.z);
    e += frame.sh[3].rgb * (0.488603 * n.x);
    e += frame.sh[4].rgb * (1.092548 * n.x * n.y);
    e += frame.sh[5].rgb * (1.092548 * n.y * n.z);
    e += frame.sh[6].rgb * (0.315392 * (3.0 * n.z * n.z - 1.0));
    e += frame.sh[7].rgb * (1.092548 * n.x * n.z);
    e += frame.sh[8].rgb * (0.546274 * (n.x * n.x - n.y * n.y));
    return max(e, vec3<f32>(0.0));
}

// Soft wrapped light for cloth: 1 facing the key, a little past its edge.
fn wrap(n_dot_l: f32) -> f32 {
    return max((n_dot_l + 0.5) / 1.5, 0.0);
}

// How much of the floor grid line there is at floor point `p` (0..1) for lines every `spacing`
// metres: about a pixel wide whatever the distance, and gone where the lines would crowd.
fn grid_line(p: vec2<f32>, spacing: f32) -> f32 {
    let c = p / spacing;
    let width = fwidth(c);
    let to_line = abs(fract(c - 0.5) - 0.5) / max(width, vec2<f32>(1e-5));
    let line = 1.0 - min(min(to_line.x, to_line.y), 1.0);
    let crowded = smoothstep(0.15, 0.4, max(width.x, width.y));
    return line * (1.0 - crowded);
}

// The backdrop seen along `dir`: light grey at and below the horizon, a little darker above.
fn backdrop(dir: vec3<f32>) -> vec3<f32> {
    return mix(frame.horizon.rgb, frame.top.rgb, smoothstep(0.0, 0.6, dir.y));
}

// Interleaved gradient noise: a different rotation for neighbouring pixels.
fn pixel_noise(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

// How much of the key light reaches `world` (1: all of it): the shadow map sampled at
// rotated Poisson-disc taps, so the shadow edge is soft.
fn key_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32 {
    if (frame.flags.y == 0u) {
        return 1.0;
    }
    // Moved off the surface a little towards its normal: thin cloth doesn't shadow itself.
    let lp = frame.key_view_proj * vec4<f32>(world + n * (1.5 * frame.extra.z), 1.0);
    let ndc = lp.xyz / lp.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || ndc.z >= 1.0) {
        return 1.0;
    }
    var disc = array<vec2<f32>, 12>(
        vec2<f32>(-0.326, -0.406), vec2<f32>(-0.840, -0.074), vec2<f32>(-0.696, 0.457),
        vec2<f32>(-0.203, 0.621), vec2<f32>(0.962, -0.195), vec2<f32>(0.473, -0.480),
        vec2<f32>(0.519, 0.767), vec2<f32>(0.185, -0.893), vec2<f32>(0.507, 0.064),
        vec2<f32>(0.896, 0.412), vec2<f32>(-0.322, -0.933), vec2<f32>(-0.792, -0.598)
    );
    let angle = (pixel_noise(pixel) + frame.params.y * 0.618034) * 6.2831853;
    let rotate = mat2x2<f32>(cos(angle), sin(angle), -sin(angle), cos(angle));
    // A soft studio light: the shadow edge spreads over about 4 cm.
    let radius = 0.04 / frame.key_box.x;
    // A tap reaching sideways over a surface tilted to the light finds that surface itself
    // nearer the light: allow for the tilt over the tap's reach (receiver slope bias).
    let facing = clamp(dot(n, frame.key_dir.xyz), 0.05, 1.0);
    let slope = min(sqrt(1.0 - facing * facing) / facing, 3.0);
    let taps = min(i32(frame.extra.x), 12);
    var lit = 0.0;
    for (var i = 0; i < taps; i++) {
        let offset = rotate * disc[i] * radius;
        let bias = 0.0015 + length(offset) * frame.key_box.x * slope / frame.key_box.y;
        lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + offset, ndc.z - bias);
    }
    return lit / f32(max(taps, 1));
}

// How much of the soft light reaches the floor at `world` (1: all of it): the blurred contact
// map, darkest under whatever is closest above the floor.
fn contact_shadow(world: vec3<f32>) -> f32 {
    if (frame.flags.z == 0u) {
        return 1.0;
    }
    let cp = frame.contact_view_proj * vec4<f32>(world.x, 0.0, world.z, 1.0);
    let uv = cp.xy / cp.w * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return 1.0;
    }
    let dark = textureSampleLevel(contact_map, linear_sampler, uv, 0.0).r;
    return 1.0 - frame.extra.w * dark;
}

// Distance (metres) from the camera of what the prepass drew at full-resolution pixel `p`
// (24 bits in an 8-bit RGB texture, as ao.wgsl packs it).
fn depth_at(p: vec2<i32>) -> f32 {
    let v = round(textureLoad(prepass_distance, p, 0).rgb * 255.0);
    return (v.r + (v.g + v.b / 255.0) / 255.0) / 255.0 * 20.0;
}

fn unpack_depth(gb: vec2<f32>) -> f32 {
    return (gb.x * 255.0 + gb.y) / 255.0 * 20.0;
}

// Soft darkening in folds for the pixel at `pixel` (1: none): the AO result scaled up from
// its own resolution, each of the four nearest values weighted by how close its depth is, so
// it doesn't halo at edges.
fn occlusion_at(pixel: vec2<f32>) -> f32 {
    if (frame.flags.x == 0u) {
        return 1.0;
    }
    let full = vec2<i32>(textureDimensions(prepass_distance));
    let small = vec2<i32>(textureDimensions(ao_map));
    let depth = depth_at(min(vec2<i32>(floor(pixel)), full - 1));
    let at = pixel * vec2<f32>(small) / vec2<f32>(full) - 0.5;
    let base = vec2<i32>(floor(at));
    let f = fract(at);
    var sum = 0.0;
    var total = 0.0;
    for (var j = 0; j < 2; j++) {
        for (var i = 0; i < 2; i++) {
            let q = clamp(base + vec2<i32>(i, j), vec2<i32>(0), small - 1);
            let tap = textureLoad(ao_map, q, 0);
            let wx = select(1.0 - f.x, f.x, i == 1);
            let wy = select(1.0 - f.y, f.y, j == 1);
            let w = max(wx * wy, 1e-3) / (1e-3 + abs(unpack_depth(tap.gb) - depth));
            sum += w * tap.r;
            total += w;
        }
    }
    return sum / total;
}

// The darkening for the soft light, with GTAO's multi-bounce correction for `albedo`: light
// bounces about inside a pale fold, so pale fabrics don't go grey.
fn multi_bounce(ao: f32, albedo: vec3<f32>) -> vec3<f32> {
    let a = 2.0404 * albedo - 0.3324;
    let b = -4.7951 * albedo + 0.6417;
    let c = 2.7552 * albedo + 0.6903;
    return max(vec3<f32>(ao), ((ao * a + b) * ao + c) * ao);
}

// Scene light to what the target holds: as it is (HDR), or tone-mapped and encoded (LDR).
fn finish(c: vec3<f32>) -> vec4<f32> {
    if (frame.flags.w != 0u) {
        return vec4<f32>(encode_srgb(pbr_neutral(c * frame.params.x)), 1.0);
    }
    return vec4<f32>(c, 1.0);
}

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_mesh(v: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(v.position, 1.0);
    out.world = v.position;
    out.normal = v.normal;
    return out;
}

@fragment
fn fs_mesh(v: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(v.normal);
    var albedo = draw.colour.rgb;
    let material = draw.material.x;
    if (!front) {
        // The other side: seen from inside a garment, like its lining.
        n = -n;
        if (material == CLOTH) {
            albedo = albedo * LINING;
        }
    }
    let to_eye = normalize(frame.camera_pos.xyz - v.world);
    let n_dot_v = clamp(dot(n, to_eye), 1e-4, 1.0);
    let l = frame.key_dir.xyz;
    let n_dot_l = dot(n, l);
    let lit = max(n_dot_l, 0.0);
    let soft = irradiance(n);
    let key = frame.key_colour.rgb;
    let occluded = occlusion_at(v.clip.xy);
    let ao = multi_bounce(occluded, albedo);
    // Folds also darken the key a little: the shadow map is too coarse for the small shadows
    // inside a fold (like HDRP's "direct lighting strength" for SSAO).
    let shadow = key_shadow(v.world, n, v.clip.xy) * mix(1.0, occluded, AO_ON_KEY);
    // Every surface reflects about 4 % like any dielectric; rough, so little more at the edges.
    let fresnel = 0.04 + 0.16 * pow(1.0 - n_dot_v, 5.0);
    let reflected = fresnel * (soft * ao + key * lit * shadow);
    var c: vec3<f32>;
    if (material == CLOTH) {
        // Charlie sheen (roughness 0.5) with Neubelt visibility: fabric glows at grazing angles
        // instead of shining.
        let sheen = 0.5 * albedo + vec3<f32>(0.03);
        let h = normalize(to_eye + l);
        let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
        let d = 4.0 * (1.0 - n_dot_h * n_dot_h) / (2.0 * PI);
        let vis = 1.0 / max(4.0 * (lit + n_dot_v - lit * n_dot_v), 1e-3);
        let glow = 0.6 * pow(1.0 - n_dot_v, 4.0);
        let base = 1.0 - max(sheen.r, max(sheen.g, sheen.b)) * glow;
        // The rim light, from behind: wrapped light and sheen along the edges.
        let r = frame.rim_dir.xyz;
        let n_dot_r = dot(n, r);
        let rim_lit = max(n_dot_r, 0.0);
        let hr = normalize(to_eye + r);
        let n_dot_hr = clamp(dot(n, hr), 0.0, 1.0);
        let dr = 4.0 * (1.0 - n_dot_hr * n_dot_hr) / (2.0 * PI);
        let vis_r = 1.0 / max(4.0 * (rim_lit + n_dot_v - rim_lit * n_dot_v), 1e-3);
        let rim = frame.rim_colour.rgb;
        c = albedo * (soft * ao + key * wrap(n_dot_l) * shadow + rim * wrap(n_dot_r)) * base
            + sheen * (d * vis * PI * key * lit * shadow + dr * vis_r * PI * rim * rim_lit
                + soft * ao * glow)
            + reflected;
    } else if (material == FORM) {
        let rim_lit = max(dot(n, frame.rim_dir.xyz), 0.0);
        c = albedo * (soft * ao + key * lit * shadow + frame.rim_colour.rgb * rim_lit) + reflected;
    } else {
        // The floor catches shadows: it shows the backdrop behind it, darkened only where the
        // form and the garment shade it, so it meets the backdrop with no edge, like a photo
        // studio's cove. Out past the form the shading fades away.
        let from_centre = length(v.world.xz);
        let near = 1.0 - smoothstep(frame.params.z, frame.params.w, from_centre);
        // The key's shadow darkens the floor this much (the soft light still reaches it).
        let shade = ao * contact_shadow(v.world) * (1.0 - frame.key_box.z * (1.0 - shadow));
        // The grid: faint lines every 10 cm, stronger every metre, fading out with distance.
        let grid_near = 1.0 - smoothstep(frame.grid_fade.x, frame.grid_fade.y, from_centre);
        let lines = max(
            grid_line(v.world.xz, frame.grid.x) * frame.grid.z,
            grid_line(v.world.xz, frame.grid.y) * frame.grid.w,
        );
        c = backdrop(-to_eye) * mix(vec3<f32>(1.0), shade, near) * (1.0 - lines * grid_near);
    }
    return finish(c);
}

@vertex
fn vs_backdrop(@builtin(vertex_index) i: u32) -> Fullscreen {
    return fullscreen(i, 1.0);
}

@fragment
fn fs_backdrop(v: Fullscreen) -> @location(0) vec4<f32> {
    let far = frame.inv_view_proj * vec4<f32>(v.ndc, 1.0, 1.0);
    let dir = normalize(far.xyz / far.w - frame.camera_pos.xyz);
    return finish(backdrop(dir));
}
