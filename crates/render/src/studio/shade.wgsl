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
    label_centre: vec4<f32>,
    label_right: vec4<f32>,
    label_up: vec4<f32>,
};
@group(1) @binding(0) var<uniform> draw: Draw;
@group(1) @binding(1) var picture: texture_2d<f32>;
@group(1) @binding(2) var picture_sampler: sampler;

@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var contact_map: texture_2d<f32>;
@group(2) @binding(3) var linear_sampler: sampler;
@group(2) @binding(4) var ao_map: texture_2d<f32>;
@group(2) @binding(5) var prepass_distance: texture_2d<f32>;
@group(2) @binding(6) var key_depths: texture_2d<f32>;

const CLOTH: u32 = 0u;
const FORM: u32 = 1u;
const LINEN: u32 = 3u;
const METAL: u32 = 4u;
const LABEL: u32 = 5u;
// Linen's threads are this far apart (m)...
const THREAD: f32 = 0.0009;
// ...with slubs (thicker stretches of yarn) about this wide and long (m), and a soft mottle
// about this large (m).
const SLUB_WIDTH: f32 = 0.0018;
const SLUB_LENGTH: f32 = 0.02;
const MOTTLE: f32 = 0.015;
// How far (m) linen's threads, slubs and undulations stand proud.
const THREAD_RISE: f32 = 0.00006;
const SLUB_RISE: f32 = 0.00012;
const CLOTH_RISE: f32 = 0.0002;
// Brushed metal's roughness.
const METAL_ROUGHNESS: f32 = 0.45;
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

fn hash11(x: f32) -> f32 {
    return fract(sin(x * 127.1 + 31.7) * 43758.5453);
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Smooth value noise, 0..1.
fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), u.x);
    let b = mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x);
    return mix(a, b, u.y);
}

// How much lighter or darker linen is at cover point `t` (about 1): a plain weave of slubby
// threads, slubs of uneven yarn both ways, and a soft mottle. Each fades out where it gets
// finer than about a pixel, so it never shimmers: up close the threads show, at the usual
// distance the slubs and mottle, from far off a plain colour. `span` is how far apart (m)
// neighbouring pixels are on the cover (from the caller, in uniform control flow).
fn linen(t: vec2<f32>, span: f32) -> f32 {
    let p = t / THREAD;
    let cell = floor(p);
    let f = fract(p) - 0.5;
    // Over and under, like a chequerboard: the warp (up) on top in half the cells.
    let over = fract((cell.x + cell.y) * 0.5) < 0.25;
    // A round thread: lighter along its middle.
    let warp = 1.0 - 3.0 * f.x * f.x;
    let weft = 1.0 - 3.0 * f.y * f.y;
    let thread = select(weft, warp, over);
    // Slubs: each thread a little thicker or thinner along its length.
    let slub = select(hash11(cell.y), hash11(cell.x + 17.0), over);
    let weave = 0.10 * (thread - 0.75) + 0.06 * (slub - 0.5);
    // Uneven yarn: slubs, thicker stretches of thread a few threads wide and a couple of
    // centimetres long, along the warp (up) and the weft (across), mostly the weft.
    let warp_slubs = noise2(vec2<f32>(t.x / SLUB_WIDTH, t.y / SLUB_LENGTH)) - 0.5;
    let weft_slubs = noise2(vec2<f32>(t.x / SLUB_LENGTH + 37.0, t.y / SLUB_WIDTH)) - 0.5;
    let slubs = 0.05 * warp_slubs + 0.08 * weft_slubs;
    let mottle = 0.05 * (noise2(t / MOTTLE) - 0.5);
    return 1.0 + weave * (1.0 - smoothstep(0.25, 0.6, span / THREAD))
        + slubs * (1.0 - smoothstep(0.3, 0.8, span / SLUB_WIDTH))
        + mottle * (1.0 - smoothstep(0.3, 0.8, span / MOTTLE));
}

// How high (m) linen's surface stands at cover point `t`: its threads round over and under
// each other, slubs stand proud, and the cloth undulates a little; each part fades out as in
// `linen`, so the bump never shimmers either.
fn linen_height(t: vec2<f32>, span: f32) -> f32 {
    let p = t / THREAD;
    let cell = floor(p);
    let f = fract(p) - 0.5;
    let over = fract((cell.x + cell.y) * 0.5) < 0.25;
    let thread = select(1.0 - 4.0 * f.y * f.y, 1.0 - 4.0 * f.x * f.x, over);
    let warp_slubs = noise2(vec2<f32>(t.x / SLUB_WIDTH, t.y / SLUB_LENGTH));
    let weft_slubs = noise2(vec2<f32>(t.x / SLUB_LENGTH + 37.0, t.y / SLUB_WIDTH));
    let undulate = noise2(t / MOTTLE + vec2<f32>(5.0, 9.0));
    return THREAD_RISE * thread * (1.0 - smoothstep(0.25, 0.6, span / THREAD))
        + SLUB_RISE * (0.4 * warp_slubs + 0.6 * weft_slubs)
            * (1.0 - smoothstep(0.3, 0.8, span / SLUB_WIDTH))
        + CLOTH_RISE * undulate * (1.0 - smoothstep(0.3, 0.8, span / MOTTLE));
}

// `n` tilted by the slope of a height field across the surface (Mikkelsen's bump mapping
// without tangents): `dpx`/`dpy` are how the surface point moves to the next pixel across and
// down, `dhx`/`dhy` how the height does.
fn bumped(n: vec3<f32>, dpx: vec3<f32>, dpy: vec3<f32>, dhx: f32, dhy: f32) -> vec3<f32> {
    let r1 = cross(dpy, n);
    let r2 = cross(n, dpx);
    let det = dot(dpx, r1);
    if (abs(det) < 1e-12) {
        return n;
    }
    let grad = sign(det) * (dhx * r1 + dhy * r2);
    return normalize(abs(det) * n - grad);
}

// Where `world` is on a form's cover: across (m, round its centre line, as if 15 cm out) and up.
fn cover_coords(world: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(atan2(world.x, world.z) * 0.15, world.y);
}

// GGX's spread of microfacets, and Smith's height-correlated visibility.
fn ggx(n_dot_h: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

fn smith(n_dot_l: f32, n_dot_v: f32, a: f32) -> f32 {
    let a2 = a * a;
    let v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / max(v + l, 1e-5);
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

// Soft shadows: the shadow edge is MIN_PENUMBRA wide (metres) at a contact, and widens by
// SOFTBOX_SPREAD for each metre between the shadow and what casts it (a softbox about 17° across
// as the light sees it), up to BLOCKER_SEARCH, the furthest the search for casters looks.
const MIN_PENUMBRA: f32 = 0.008;
const SOFTBOX_SPREAD: f32 = 0.15;
const BLOCKER_SEARCH: f32 = 0.25;

fn unpack_unit(c: vec3<f32>) -> f32 {
    let v = round(c * 255.0);
    return (v.r + (v.g + v.b / 255.0) / 255.0) / 255.0;
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
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))) {
        return 1.0;
    }
    // Beyond the map's far side counts as at it: still shaded where something stands between
    // it and the light (never suddenly lit, which drew a straight edge across the floor).
    let depth = min(ndc.z, 0.99999);
    var disc = array<vec2<f32>, 12>(
        vec2<f32>(-0.326, -0.406), vec2<f32>(-0.840, -0.074), vec2<f32>(-0.696, 0.457),
        vec2<f32>(-0.203, 0.621), vec2<f32>(0.962, -0.195), vec2<f32>(0.473, -0.480),
        vec2<f32>(0.519, 0.767), vec2<f32>(0.185, -0.893), vec2<f32>(0.507, 0.064),
        vec2<f32>(0.896, 0.412), vec2<f32>(-0.322, -0.933), vec2<f32>(-0.792, -0.598)
    );
    let angle = (pixel_noise(pixel) + frame.params.y * 0.618034) * 6.2831853;
    let rotate = mat2x2<f32>(cos(angle), sin(angle), -sin(angle), cos(angle));
    // A tap reaching sideways over a surface tilted to the light finds that surface itself
    // nearer the light: allow for the tilt over the tap's reach (receiver slope bias).
    let facing = clamp(dot(n, frame.key_dir.xyz), 0.05, 1.0);
    let slope = min(sqrt(1.0 - facing * facing) / facing, 3.0);
    // Contact-hardening (PCSS): first find how far, along the light, the things shading this
    // spot are from it. A softbox lights from a spread of directions, so the further away they
    // are, the wider and softer the shadow's edge: crisp where a foot meets the floor, soft
    // at the far end of the shadow.
    let size = vec2<i32>(textureDimensions(key_depths));
    let search = BLOCKER_SEARCH / frame.key_box.x;
    var blockers = 0.0;
    var blocker_depth = 0.0;
    for (var i = 0; i < 8; i++) {
        let offset = rotate * disc[i] * search;
        let texel = clamp(vec2<i32>((uv + offset) * vec2<f32>(size)), vec2<i32>(0), size - 1);
        let d = unpack_unit(textureLoad(key_depths, texel, 0).rgb);
        let bias = 0.0015 + length(offset) * frame.key_box.x * slope / frame.key_box.y;
        if (d < depth - bias) {
            blockers += 1.0;
            blocker_depth += d;
        }
    }
    if (blockers == 0.0) {
        return 1.0;
    }
    let gap = (depth - blocker_depth / blockers) * frame.key_box.y;
    let penumbra = clamp(MIN_PENUMBRA + gap * SOFTBOX_SPREAD, MIN_PENUMBRA, BLOCKER_SEARCH);
    let radius = penumbra / frame.key_box.x;
    let taps = min(i32(frame.extra.x), 12);
    var lit = 0.0;
    for (var i = 0; i < taps; i++) {
        let offset = rotate * disc[i] * radius;
        let bias = 0.0015 + length(offset) * frame.key_box.x * slope / frame.key_box.y;
        lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + offset, depth - bias);
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
    @location(2) seam: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) seam: f32,
};

@vertex
fn vs_mesh(v: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = frame.view_proj * vec4<f32>(v.position, 1.0);
    out.world = v.position;
    out.normal = v.normal;
    out.seam = v.seam;
    return out;
}

// How a sewn seam stands at `d` mm from the stitch line: a narrow groove where the stitches
// pull the fabric in, and a slight rise either side where the allowance underneath lifts it.
// The light then draws each seam as a fine crest with a shadow beside it, as a real one
// reads. The slope (mm per mm) is what bends the normal; the heights are in mm.
const SEAM_GROOVE_MM: f32 = 0.45;
const SEAM_GROOVE_WIDTH_MM: f32 = 0.75;
const SEAM_CREST_MM: f32 = 0.2;
const SEAM_CREST_AT_MM: f32 = 1.7;
const SEAM_CREST_WIDTH_MM: f32 = 0.9;
// Past this (mm) a seam leaves the shading alone.
const SEAM_REACH_MM: f32 = 6.0;

// The seam profile's slope at `d` mm from the stitch line (mm per mm).
fn seam_slope(d: f32) -> f32 {
    let gw = SEAM_GROOVE_WIDTH_MM * SEAM_GROOVE_WIDTH_MM;
    let groove = -SEAM_GROOVE_MM * exp(-(d * d) / gw);
    let c = d - SEAM_CREST_AT_MM;
    let cw = SEAM_CREST_WIDTH_MM * SEAM_CREST_WIDTH_MM;
    let crest = SEAM_CREST_MM * exp(-(c * c) / cw);
    return -2.0 * d / gw * groove - 2.0 * c / cw * crest;
}

@fragment
fn fs_mesh(v: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Taken first, in uniform control flow (they need neighbouring pixels): the cover's weave
    // coordinates and how fine they are here, and the label's picture.
    let cover = cover_coords(v.world);
    let spans = fwidth(cover);
    let span = max(spans.x, spans.y);
    let off = v.world - draw.label_centre.xyz;
    let label_uv = vec2<f32>(
        dot(off, draw.label_right.xyz) / draw.label_right.w + 0.5,
        0.5 - dot(off, draw.label_up.xyz) / draw.label_up.w,
    );
    let pictured = textureSample(picture, picture_sampler, label_uv).rgb;
    let height = linen_height(cover, span) * frame.key_box.w;
    let dpx = dpdx(v.world);
    let dpy = dpdy(v.world);
    let dhx = dpdx(height);
    let dhy = dpdy(height);
    var n = normalize(v.normal);
    var albedo = draw.colour.rgb;
    let material = draw.material.x;
    // The seam's groove bends the normal (bump mapping from a height, Mikkelsen 2010). The
    // seam distance changes by `per_px` mm from one pixel to the next: seen from far enough
    // away that the groove would fall between pixels, the whole profile is scaled up to stay
    // about a pixel wide (its slope, so its strength, unchanged), as a drawn line would.
    // Its derivatives too come first, where control flow is still uniform.
    let d_dx = dpdx(v.seam);
    let d_dy = dpdy(v.seam);
    let per_px = abs(d_dx) + abs(d_dy);
    let scale = max(1.0, 0.75 * per_px / SEAM_GROOVE_WIDTH_MM);
    let d = v.seam / scale;
    let slope = seam_slope(d);
    let dh_dx = slope * d_dx * 0.001;
    let dh_dy = slope * d_dy * 0.001;
    if (material == CLOTH && d < SEAM_REACH_MM) {
        n = bumped(n, dpx, dpy, dh_dx, dh_dy);
        // Stitches shade the groove a little as well.
        albedo = albedo * (1.0 - 0.16 * exp(-(d * d) / 0.8));
    } else if (material == LINEN) {
        albedo = albedo * linen(cover, span);
    } else if (material == LABEL) {
        // A woven label: its picture, with a little of the weave.
        albedo = pictured * (1.0 + 0.5 * (linen(cover, span) - 1.0));
    }
    if (!front) {
        // The other side: seen from inside a garment, like its lining.
        n = -n;
        if (material == CLOTH) {
            albedo = albedo * LINING;
        }
    }
    if (material == LINEN || material == LABEL) {
        // The weave's relief: its slubs and folds catch the light.
        n = bumped(n, dpx, dpy, dhx, dhy);
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
    } else if (material == FORM || material == LINEN || material == LABEL) {
        let rim_lit = max(dot(n, frame.rim_dir.xyz), 0.0);
        c = albedo * (soft * ao + key * lit * shadow + frame.rim_colour.rgb * rim_lit) + reflected;
    } else if (material == METAL) {
        // Brushed metal: the studio's soft light seen in the reflection (blurred, as the SH
        // light is), and the key and the rim as soft highlights. Its colour is its reflectance.
        let a = METAL_ROUGHNESS * METAL_ROUGHNESS;
        let f = albedo + (vec3<f32>(1.0) - albedo) * pow(1.0 - n_dot_v, 5.0);
        let env = irradiance(reflect(-to_eye, n));
        let h = normalize(to_eye + l);
        let spec = ggx(max(dot(n, h), 0.0), a) * smith(lit, n_dot_v, a);
        let r = frame.rim_dir.xyz;
        let rim_lit = max(dot(n, r), 0.0);
        let hr = normalize(to_eye + r);
        let spec_r = ggx(max(dot(n, hr), 0.0), a) * smith(rim_lit, n_dot_v, a);
        c = f * (env * ao + PI * spec * key * lit * shadow
            + PI * spec_r * frame.rim_colour.rgb * rim_lit);
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
