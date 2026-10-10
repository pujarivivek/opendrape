"""Samples a shaped form in a .blend into OpenDrape's ring format and writes <id>.form.json.

    blender -b --factory-startup <form.blend> --python-exit-code 1 \
        --python scripts/forms/export_form.py -- <meta.json> <out.form.json>

The mesh object "Form" (or "Arm") is read with its modifiers applied. At each ring height, rays
from the centre line at 49 angles (centre front 0 deg to centre back 180 deg, the form's left
side) give the radii; the right side is the mirror image. Rings sit exactly at every station
("st.<name>" empties) and are spread between stations by surface length, so flat areas such as
the shoulder tops get more rings. Landmarks ("lm.<name>" empties) are stored as (angle, v), where
v runs 0 (bottom ring) to 1 (top ring). Tape lines are designed in the meta file in the view a
designer would draw them (princess lines from the front and back, side seam and armhole from the
side, shoulder seam from above) and are stored as dense (angle, v) samples on the surface.
Everything else (name, inputs, ranges...) comes from the meta file too. Exits non-zero, naming
the height, if the shape is not star-shaped around its centre line (a ray leaves the surface and
re-enters it), or naming the tape if a tape point misses the form.
"""
import json
import math
import sys

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

ANGLES = 49  # samples from 0 to 180 degrees inclusive
RINGS = 100  # about this many rings in the exported file; the count depends on the form (99 on the men's)
SCAN_STEP = 0.002  # m between scan slices when measuring surface length
TAU5 = round(2 * math.pi, 5)  # 6.28319: an angle that rounds to this is 2 pi, i.e. 0


def angle5(phi):
    """phi (radians, 0 <= phi < 2 pi) rounded to 5 decimals for the file. A value just under 2 pi
    rounds up to 6.28319, which is past 2 pi and refused by the loader, so it is stored as 0, the
    same direction."""
    phi = round(phi, 5)
    return 0.0 if phi >= TAU5 else phi


def od_to_blender(x, y, z):
    return Vector((x, -z, y))


def blender_to_od(v):
    return v.x, v.z, -v.y


class Sampler:
    def __init__(self, obj):
        dg = bpy.context.evaluated_depsgraph_get()
        self.bvh = BVHTree.FromObject(obj, dg)
        ys = [blender_to_od(obj.matrix_world @ v.co)[1] for v in obj.evaluated_get(dg).data.vertices]
        self.y_min, self.y_max = min(ys), max(ys)

    def cast(self, origin, d):
        hit = self.bvh.ray_cast(origin, d, 2.0)
        if hit[0] is None:
            return None
        # A second hit within 2 mm is a ray grazing a thin sliver (the top of the slanted neck
        # cut), not an overhang.
        again = self.bvh.ray_cast(hit[0] + d * 1e-4, d, 2.0)
        if again[0] is not None and (again[0] - hit[0]).length > 0.002:
            y, z = blender_to_od(hit[0])[1:]
            raise SystemExit(f"not star-shaped at y={y:.4f} m: a ray meets the surface twice")
        return hit[3]

    def hit_od(self, origin, direction):
        """First surface point (OpenDrape coordinates) along a ray, or None."""
        o = od_to_blender(*origin)
        d = od_to_blender(*direction) - od_to_blender(0, 0, 0)
        hit = self.bvh.ray_cast(o, d, 5.0)
        return None if hit[0] is None else blender_to_od(hit[0])

    def nearest_od(self, p):
        """The closest surface point (OpenDrape coordinates)."""
        return blender_to_od(self.bvh.find_nearest(od_to_blender(*p))[0])

    def centre_z(self, y):
        """Midpoint of the front and back surface along x = 0, found from outside the form."""
        front = self.hit_od((0, y, 2.0), (0, 0, -1))
        back = self.hit_od((0, y, -2.0), (0, 0, 1))
        if front is None or back is None:
            raise SystemExit(f"no surface in front of or behind the centre line at y={y:.4f} m")
        return (front[2] + back[2]) / 2

    def ring(self, y):
        zc = self.centre_z(y)
        o = od_to_blender(0, y, zc)
        radii = []
        for k in range(ANGLES):
            a = math.pi * k / (ANGLES - 1)
            d = od_to_blender(math.sin(a), 0, math.cos(a)) - od_to_blender(0, 0, 0)
            r = self.cast(o, d)
            if r is None:
                raise SystemExit(f"ray missed the form at y={y:.4f} m, angle {math.degrees(a):.1f} deg")
            radii.append(r)
        return zc, radii


def ring_heights(s, stations):
    """About RINGS heights: one exactly at each station, the rest spread by surface length.

    Each stretch between two fixed heights gets a whole number of rings, so the total depends on
    the form (the women's has 100, the men's 99).
    """
    lo, hi = s.y_min + 0.001, s.y_max - 0.001
    ys = [lo + (hi - lo) * i / int((hi - lo) / SCAN_STEP) for i in range(int((hi - lo) / SCAN_STEP) + 1)]
    rings = [s.ring(y) for y in ys]
    arc = [0.0]
    for i in range(1, len(ys)):
        dr = max(abs(a - b) for a, b in zip(rings[i][1], rings[i - 1][1]))
        arc.append(arc[-1] + math.hypot(ys[i] - ys[i - 1], dr))

    def arc_at(y):
        i = min(max(0, int((y - lo) / (hi - lo) * (len(ys) - 1))), len(ys) - 2)
        t = (y - ys[i]) / (ys[i + 1] - ys[i])
        return arc[i] + t * (arc[i + 1] - arc[i])

    def y_at(a):
        i = next(k for k in range(1, len(arc)) if arc[k] >= a) if a < arc[-1] else len(arc) - 1
        t = (a - arc[i - 1]) / max(arc[i] - arc[i - 1], 1e-12)
        return ys[i - 1] + t * (ys[i] - ys[i - 1])

    fixed = sorted({lo, hi, *stations.values()})
    per = (RINGS - 1) / arc[-1]
    out = [fixed[0]]
    for a, b in zip(fixed, fixed[1:]):
        n = max(1, round((arc_at(b) - arc_at(a)) * per))
        out += [y_at(arc_at(a) + (arc_at(b) - arc_at(a)) * j / n) for j in range(1, n)] + [b]
    return out


def catmull_rom(ctrl, closed, wrap=(0.0, 0.0), per_span=48):
    """Uniform Catmull-Rom through 2D points; a closed curve wraps by `wrap` per turn."""
    n = len(ctrl)

    def get(k):
        if closed:
            turns = k // n
            p = ctrl[k % n]
            return (p[0] + wrap[0] * turns, p[1] + wrap[1] * turns)
        return ctrl[min(max(k, 0), n - 1)]

    out = []
    for s in range(n if closed else n - 1):
        p0, p1, p2, p3 = get(s - 1), get(s), get(s + 1), get(s + 2)
        for q in range(per_span):
            t = q / per_span
            out.append(tuple(0.5 * (2 * b + (c - a) * t + (2 * a - 5 * b + 4 * c - d) * t * t
                                    + (3 * b - a - 3 * c + d) * t ** 3) for a, b, c, d in zip(p0, p1, p2, p3)))
    if not closed:
        out.append(tuple(ctrl[-1]))
    return out


def project(s, view, a, b, zc_at):
    """The surface point a designer would mark at (a, b) in one view, in OpenDrape metres.

    view "front"/"back": (x, y) in cm seen from the front or back, projected along z.
    view "side": (z offset from the centre line, y), projected along x onto the left side.
    view "top": (x, z), projected straight down (the shoulder seam).
    view "around": (degrees from centre front, y), cast out from the centre line (CF, CB).
    """
    if view == "front":
        return s.hit_od((a / 100, b / 100, 2.0), (0, 0, -1))
    if view == "back":
        return s.hit_od((a / 100, b / 100, -2.0), (0, 0, 1))
    if view == "side":
        y = b / 100
        return s.hit_od((2.0, y, zc_at(y) + a / 100), (-1, 0, 0))
    if view == "top":
        return s.hit_od((a / 100, 3.0, b / 100), (0, -1, 0))
    y, phi = b / 100, math.radians(a)
    return s.hit_od((0, y, zc_at(y)), (math.sin(phi), 0, math.cos(phi)))


def sample_tape(s, name, spec, zc_at, v_of):
    """A tape designed in the view a designer would draw it in, laid on the surface.

    A tape drawn in one view is a smooth curve in that view, projected onto the surface. A
    "mixed" tape lists [view, a, b] points, each marked in its own view (a neckline is marked
    from the front, from above where it turns over the shoulder, and from the back); the curve
    runs smoothly through those surface points and is pressed onto the surface.
    Returns [angle, v] samples; angles are 0..2pi so a tape may cross centre front or back.
    """
    view, closed = spec["view"], spec.get("closed", False)
    if view == "mixed":
        anchors = []
        for pv, a, b in spec["points"]:
            hit = project(s, pv, a, b, zc_at)
            if hit is None:
                raise SystemExit(f"tape {name}: the {pv} point ({a}, {b}) misses the form")
            anchors.append(hit)
        points = [s.nearest_od(p) for p in catmull_rom(anchors, closed)]
    else:
        wrap = (360.0, 0.0) if view == "around" else (0.0, 0.0)
        points = []
        for a, b in catmull_rom([tuple(p) for p in spec["points"]], closed, wrap):
            hit = project(s, view, a, b, zc_at)
            if hit is None:
                raise SystemExit(f"tape {name}: the point ({a}, {b}) misses the form")
            points.append(hit)
    out = []
    for x, y, z in points:
        phi = math.atan2(x, z - zc_at(y)) % (2 * math.pi)
        out.append([angle5(phi), round(v_of(y), 6)])
    return out


def main(meta_path, out_path):
    meta = json.load(open(meta_path))
    obj = bpy.data.objects.get("Form") or bpy.data.objects["Arm"]
    s = Sampler(obj)
    stations = {o.name[3:]: blender_to_od(o.location)[1] for o in bpy.data.objects if o.name.startswith("st.")}
    heights = ring_heights(s, stations)
    rings = [s.ring(y) for y in heights]
    n = len(heights)
    station_index = {k: min(range(n), key=lambda i: abs(heights[i] - y)) for k, y in stations.items()}

    def v_of(y):
        y = min(max(y, heights[0]), heights[-1])
        i = min(max(0, max(k for k in range(n) if heights[k] <= y)), n - 2)
        return (i + (y - heights[i]) / (heights[i + 1] - heights[i])) / (n - 1)

    def zc_at(y):
        y = min(max(y, heights[0]), heights[-1])
        i = min(max(0, max(k for k in range(n) if heights[k] <= y)), n - 2)
        t = (y - heights[i]) / (heights[i + 1] - heights[i])
        return rings[i][0] + t * (rings[i + 1][0] - rings[i][0])

    # The stand: where the pole runs (x, z) and the plane the neck is cut by (its height on the
    # pole axis and its tilt, lower at the front).
    stand = {}
    if "pole_xz" in obj:
        stand["pole_xz"] = [round(v, 5) for v in obj["pole_xz"]]
    if "neck_cut" in obj:
        cut_y, cut_deg = obj["neck_cut"]
        stand["neck_cut"] = {"y": round(cut_y, 5), "tilt_deg": round(cut_deg, 2)}
    tapes = {}
    for name, spec in meta["tapes"].items():
        if "ring" in spec:
            tapes[name] = {"ring": spec["ring"]}
            continue
        if spec.get("curve"):
            # A tape line the form builder worked out from the shape, stored as a curve.
            curve = bpy.data.objects.get(f"tape.{name}")
            if curve is None:
                raise SystemExit(f"tape {name}: the form has no curve tape.{name}")
            uv = []
            for p in curve.data.splines[0].points:
                x, y, z = blender_to_od(curve.matrix_world @ p.co.xyz)
                uv.append([angle5(math.atan2(x, z - zc_at(y)) % (2 * math.pi)), round(v_of(y), 6)])
        else:
            uv = sample_tape(s, name, spec, zc_at, v_of)
        tapes[name] = {"uv": uv, "closed": spec.get("closed", False), "mirror": spec.get("mirror", False)}

    landmarks = {}
    for o in bpy.data.objects:
        if not o.name.startswith("lm."):
            continue
        name = o.name[3:]
        x, y, z = blender_to_od(o.location)
        zc = rings[min(range(n), key=lambda i: abs(heights[i] - y))][0]
        phi = math.atan2(abs(x), z - zc)
        v = v_of(y)
        # A landmark on a station's tape (side_waist, cf_bottom...) sits exactly on its ring.
        for st, i in station_index.items():
            if name.endswith("_" + st) and abs(heights[i] - y) < 0.005:
                v = i / (n - 1)
        landmarks[name] = [round(phi, 5), round(v, 6)]
    # Landmarks that are defined by a tape (the shoulder-at-neck point is where the shoulder seam
    # starts) are taken from it, so the two always agree.
    for name, ref in meta.get("tape_landmarks", {}).items():
        uv = tapes[ref["tape"]]["uv"]
        phi, v = {"start": uv[0], "end": uv[-1], "lowest": min(uv, key=lambda q: q[1])}[ref["at"]]
        landmarks[name] = [round(phi if phi <= math.pi else 2 * math.pi - phi, 5), v]
    form = {
        "format": 1,
        **{k: meta[k] for k in ("id", "kind", "name", "suits", "licence", "base_size")},
        "angles": ANGLES,
        "rings": [{"y": round(y, 5), "zc": round(zc, 5), "r": [round(r * 1000, 1) for r in radii]}
                  for y, (zc, radii) in zip(heights, rings)],
        "stations": dict(sorted(station_index.items(), key=lambda kv: kv[1])),
        "landmarks": dict(sorted(landmarks.items())),
        "tapes": tapes,
        **({"stand": stand} if stand else {}),
        **{k: meta[k] for k in ("inputs", "ranges", "collision") if k in meta},
    }
    with open(out_path, "w") as f:
        json.dump(form, f, indent=1)
        f.write("\n")
    print(f"wrote {out_path}: {n} rings, {len(landmarks)} landmarks, stations {form['stations']}")


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :]
    main(args[0], args[1])
