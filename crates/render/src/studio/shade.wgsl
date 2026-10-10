// The studio's main pass: the form, the cloth and the floor lit by the studio (soft SH light
// plus a soft key light), and the backdrop behind them.

struct Frame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
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
@group(2) @binding(5) var prepass_depth: texture_depth_2d;

const CLOTH: u32 = 0u;
const FORM: u32 = 1u;
const LINING: f32 = 0.8;
// How dark the key light's shadow makes the floor (the soft light still reaches it).
const KEY_ON_FLOOR: f32 = 0.35;

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

// The backdrop seen along `dir`: light grey at and below the horizon, a little darker above.
fn backdrop(dir: vec3<f32>) -> vec3<f32> {
    return mix(frame.horizon.rgb, frame.top.rgb, smoothstep(0.0, 0.6, dir.y));
}

// How much of the key light reaches `world` (1: all of it).
fn key_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32 {
    if (frame.flags.y == 0u) {
        return 1.0;
    }
    return 1.0;
}

// How much of the soft light reaches the floor at `world` (1: all of it).
fn contact_shadow(world: vec3<f32>) -> f32 {
    if (frame.flags.z == 0u) {
        return 1.0;
    }
    return 1.0;
}

// Soft darkening in folds for the pixel at `pixel`, with GTAO's multi-bounce correction for
// `albedo` so pale fabrics don't go grey (1: none).
fn ambient_occlusion(pixel: vec2<f32>, albedo: vec3<f32>) -> vec3<f32> {
    if (frame.flags.x == 0u) {
        return vec3<f32>(1.0);
    }
    return vec3<f32>(1.0);
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
    let shadow = key_shadow(v.world, n, v.clip.xy);
    let ao = ambient_occlusion(v.clip.xy, albedo);
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
        c = albedo * (soft * ao + key * wrap(n_dot_l) * shadow) * base
            + sheen * (d * vis * PI * key * lit * shadow + soft * ao * glow)
            + reflected;
    } else if (material == FORM) {
        c = albedo * (soft * ao + key * lit * shadow) + reflected;
    } else {
        // The floor catches shadows: it shows the backdrop behind it, darkened only where the
        // form and the garment shade it, so it meets the backdrop with no edge, like a photo
        // studio's cove. Out past the form the shading fades away.
        let near = 1.0 - smoothstep(frame.params.z, frame.params.w, length(v.world.xz));
        let shade = ao * contact_shadow(v.world) * (1.0 - KEY_ON_FLOOR * (1.0 - shadow));
        c = backdrop(-to_eye) * mix(vec3<f32>(1.0), shade, near);
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
