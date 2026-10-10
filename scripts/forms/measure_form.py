"""Measures a dress form the way a patternmaker measures an industry form, and compares the results
with a target file.

    blender -b --factory-startup <form.blend> --python-exit-code 1 \
        --python scripts/forms/measure_form.py -- <form.json> [<targets.json>]

Measurements follow the definitions in Armstrong, Patternmaking for Fashion Design, ch. 2 (women)
and ch. 23 (men). A tape held taut bridges hollows, so lengths are the outer convex chain of the
surface profile along the tape's path, and girths and arcs follow the convex hull of the
horizontal section. Arcs run from centre front or back to the side seam. All values are in cm.
The targets file maps measurement names to cm; it is kept outside the repository because the
reference tables come from copyrighted books.
"""
import json
import math
import sys

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

INCH = 2.54
TOLERANCE = 0.6  # cm, about a quarter inch
BAND = 0.5 * INCH  # width of the waist tape


def to_blender(x, y, z):
    return Vector((x / 100, -z / 100, y / 100))


def from_blender(v):
    return v.x * 100, v.z * 100, -v.y * 100


def cross(o, a, b):
    return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])


def taut(samples):
    """Length of a taut tape over sampled surface points.

    samples are (s, d, point): s runs along the tape's straight chord, d is how far the surface
    stands out from it, point is the 3D surface point. The tape touches only the outer convex chain
    of (s, d), so it bridges hollows; its length is measured between the touching points in 3D.
    """
    hull = []
    for q in sorted(samples, key=lambda q: q[0]):
        while len(hull) >= 2 and cross(hull[-2][:2], hull[-1][:2], q[:2]) >= 0:
            hull.pop()
        hull.append(q)
    return sum(math.dist(a[2], b[2]) for a, b in zip(hull, hull[1:]))


class Form:
    def __init__(self, obj, data):
        self.bvh = BVHTree.FromObject(obj, bpy.context.evaluated_depsgraph_get())
        self.data = data
        self.h = [r["y"] * 100 for r in data["rings"]]
        self.zcs = [r["zc"] * 100 for r in data["rings"]]

    def _at(self, values, y):
        n = len(self.h)
        y = min(max(y, self.h[0]), self.h[-1])
        i = min(max(0, max(k for k in range(n) if self.h[k] <= y)), n - 2)
        t = (y - self.h[i]) / (self.h[i + 1] - self.h[i])
        return values[i] + t * (values[i + 1] - values[i])

    def zc(self, y):
        return self._at(self.zcs, y)

    def height(self, v):
        f = min(max(v, 0.0), 1.0) * (len(self.h) - 1)
        i = min(int(f), len(self.h) - 2)
        return self.h[i] + (self.h[i + 1] - self.h[i]) * (f - i)

    def station(self, name):
        return self.h[self.data["stations"][name]]

    def cast(self, origin, direction):
        o, d = to_blender(*origin), to_blender(*direction) - to_blender(0, 0, 0)
        hit = self.bvh.ray_cast(o, d.normalized(), 10.0)
        return None if hit[0] is None else from_blender(hit[0])

    def radial(self, phi, y):
        return self.cast((0, y, self.zc(y)), (math.sin(phi), 0, math.cos(phi)))

    def angle(self, p):
        return math.atan2(p[0], p[2] - self.zc(p[1])) % (2 * math.pi)

    def landmark(self, name):
        phi, v = self.data["landmarks"][name]
        return self.radial(phi, self.height(v))

    def tape_points(self, name):
        return [p for p in (self.radial(phi, self.height(v)) for phi, v in self.data["tapes"][name]["uv"]) if p]

    # Taut-tape lengths.
    def line(self, view, a, b, steps=300):
        """Tape from (x, y) a to b as seen from the front or back, lying over the surface."""
        sign = 1 if view == "front" else -1
        samples = []
        for i in range(steps + 1):
            t = i / steps
            x, y = a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t
            p = self.cast((x, y, 200 * sign), (0, 0, -sign))
            if p:
                samples.append((t * math.dist(a, b), sign * p[2], p))
        return taut(samples)

    def tape(self, a, b, view, n=48, sweeps=4000):
        """A tape pulled taut between two surface points, like a string over the form.

        It starts as the straight line seen from the front or back, laid on the surface, and is
        tightened: each point moves to the middle of its neighbours, unless that is inside the
        form, in which case it rests on the nearest surface point. So the tape takes the shortest
        way over the surface and bridges hollows.
        """
        sign = 1 if view == "front" else -1
        pts = [tuple(a)]
        for i in range(1, n):
            t = i / n
            p = self.cast((a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, 200 * sign), (0, 0, -sign))
            pts.append(p or tuple(a[k] + (b[k] - a[k]) * t for k in range(3)))
        pts.append(tuple(b))
        for _ in range(sweeps):
            moved = 0.0
            for i in range(1, n):
                q = self.outside([(pts[i - 1][k] + pts[i + 1][k]) / 2 for k in range(3)])
                moved = max(moved, math.dist(q, pts[i]))
                pts[i] = q
            if moved < 1e-4:
                break
        return self.polyline(pts)

    def outside(self, q):
        """q, or the nearest surface point if q is inside the form."""
        loc, normal, _, _ = self.bvh.find_nearest(to_blender(*q))
        if (to_blender(*q) - loc).dot(normal) >= 0:
            return tuple(q)
        return from_blender(loc)

    def centre_front(self, y_from, y_to, steps=200):
        """Centre front from y_from down to y_to, over the bridge between the busts."""
        samples = []
        for i in range(steps + 1):
            y = y_from + (y_to - y_from) * i / steps
            z = self.hull_z(y)
            samples.append((y_from - y, z, (0.0, y, z)))
        return taut(samples)

    def along(self, points, y_from, y_to):
        """Taut length down a near-vertical tape line between two heights."""
        lo, hi = min(y_from, y_to), max(y_from, y_to)
        inside = [p for p in points if lo <= p[1] <= hi]
        ends = [self.point_on(points, lo), self.point_on(points, hi)]
        pts = [ends[0]] + inside + [ends[1]]
        return taut([(p[1], math.hypot(p[0], p[2] - self.zc(p[1])), p) for p in pts])

    def point_on(self, points, y):
        for a, b in zip(points, points[1:]):
            if (a[1] - y) * (b[1] - y) <= 0 and a[1] != b[1]:
                t = (y - a[1]) / (b[1] - a[1])
                return tuple(a[k] + t * (b[k] - a[k]) for k in range(3))
        raise SystemExit(f"tape does not reach y={y:.1f}")

    def polyline(self, points):
        return sum(math.dist(a, b) for a, b in zip(points, points[1:]))

    # Horizontal measurements.
    def hull(self, y, n=1440):
        cz = self.zc(y)
        pts = []
        for k in range(n):
            p = self.radial(2 * math.pi * k / n, y)
            if p:
                pts.append((p[0], p[2]))
        pts = sorted(set(pts))
        lower, upper = [], []
        for p in pts:
            while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
                lower.pop()
            lower.append(p)
        for p in reversed(pts):
            while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
                upper.pop()
            upper.append(p)
        poly = lower[:-1] + upper[:-1]
        return poly, cz

    def hull_z(self, y):
        """Front of the section's convex hull at x = 0 (where a tape bridges the busts)."""
        poly, cz = self.hull(y, 720)
        best = None
        for i in range(len(poly)):
            p, q = poly[i], poly[(i + 1) % len(poly)]
            if (p[0] <= 0 <= q[0] or q[0] <= 0 <= p[0]) and p[0] != q[0]:
                z = p[1] + (q[1] - p[1]) * (0 - p[0]) / (q[0] - p[0])
                best = z if best is None else max(best, z)
        return best

    def outline_top(self, x):
        """Height of the top of the front-view outline at distance x from the centre line."""
        rings, n = self.data["rings"], self.data["angles"]
        half = [max(r["r"][k] / 10 * math.sin(math.pi * k / (n - 1)) for k in range(n)) for r in rings]
        for i in range(len(rings) - 1, 0, -1):
            if (half[i - 1] >= x > half[i]) or (half[i] >= x > half[i - 1]):
                t = (x - half[i]) / (half[i - 1] - half[i])
                return self.h[i] + t * (self.h[i - 1] - self.h[i])
        raise SystemExit(f"no outline at x={x:.1f}")

    def shoulder_angle(self, hps, tip):
        """Average slope (degrees) of the shoulder outline across its middle, seen from the front:
        from 2 cm out from the neck point to 1.5 cm in from the shoulder tip."""
        xa, xb = hps[0] + 2.0, tip[0] - 1.5
        return math.degrees(math.atan2(self.outline_top(xa) - self.outline_top(xb), xb - xa))

    def girth(self, y):
        poly, _ = self.hull(y)
        return sum(math.dist(poly[i], poly[(i + 1) % len(poly)]) for i in range(len(poly)))

    def arc(self, y, phi_a, phi_b):
        """Hull length at height y between two angles (taken the short way round)."""
        poly, cz = self.hull(y)
        ang = lambda p: math.atan2(p[0], p[1] - cz) % (2 * math.pi)
        lo, hi = sorted((phi_a % (2 * math.pi), phi_b % (2 * math.pi)))
        if hi - lo > math.pi:
            lo, hi = hi, lo + 2 * math.pi
        n = len(poly)

        def on_ray(phi):
            d = (math.sin(phi), math.cos(phi))
            for i in range(n):
                p, q = poly[i], poly[(i + 1) % n]
                # Solve cz-origin ray against edge p-q.
                ex, ez = q[0] - p[0], q[1] - p[1]
                den = d[0] * ez - d[1] * ex
                if abs(den) < 1e-12:
                    continue
                t = (p[0] * ez - (p[1] - cz) * ex) / den
                s = (p[0] * d[1] - (p[1] - cz) * d[0]) / den
                if t > 0 and -1e-9 <= s <= 1 + 1e-9:
                    return (p[0] + s * ex, p[1] + s * ez)
            return None

        inside = []
        for p in poly:
            a = ang(p)
            if a < lo:
                a += 2 * math.pi
            if lo < a < hi:
                inside.append((a, p))
        chain = [on_ray(lo)] + [p for _, p in sorted(inside)] + [on_ray(hi)]
        return sum(math.dist(a, b) for a, b in zip(chain, chain[1:]))

    def tape_angle_at(self, name, y, front=None):
        """Angle where a tape crosses height y (on the front half, back half, or anywhere)."""
        pts = self.tape_points(name)
        best = None
        for a, b in zip(pts, pts[1:]):
            if (a[1] - y) * (b[1] - y) <= 0 and a[1] != b[1]:
                t = (y - a[1]) / (b[1] - a[1])
                p = tuple(a[k] + t * (b[k] - a[k]) for k in range(3))
                is_front = p[2] - self.zc(y) > 0
                if front is None or front == is_front:
                    best = self.angle(p)
        return best


def women(f, targets):
    """Armstrong's Standard Measurement Chart, measured as on pp. 33-35."""
    m = {}
    waist, hip, bust = f.station("waist"), f.station("hip"), f.station("bust")
    cfn, cbn, hps = f.landmark("front_neck"), f.landmark("back_neck"), f.landmark("side_neck")
    tip, apex = f.landmark("shoulder_point"), f.landmark("bust_apex")
    armhole = f.tape_points("armhole")
    plate_bottom = min(p[1] for p in armhole)
    plate_mid = (plate_bottom + max(p[1] for p in armhole)) / 2
    seam = f.tape_points("side_seam")
    side = lambda y: f.angle(f.point_on(seam, y))
    side_waist = f.point_on(seam, waist)
    m["bust"] = f.girth(bust)
    m["waist"] = f.girth(waist)
    m["abdomen"] = f.girth(waist - 3 * INCH)
    m["hip"] = f.girth(hip)
    m["center_length_front"] = f.centre_front(cfn[1], waist)
    m["center_length_back"] = f.line("back", (0, cbn[1]), (0, waist))
    m["full_length_front"] = f.line("front", (hps[0], hps[1]), (hps[0], waist))
    m["full_length_back"] = f.line("back", (hps[0], hps[1]), (hps[0], waist))
    m["shoulder_slope_front"] = f.tape(f.radial(0.0, waist), tip, "front")
    m["shoulder_slope_back"] = f.tape(f.radial(math.pi, waist), tip, "back")
    # To the bottom of the waist tape (1/2 inch wide) at the side seam.
    m["new_strap"] = f.tape(hps, f.point_on(seam, waist - BAND / 2), "front")
    m["bust_depth"] = f.tape(tip, apex, "front")
    m["bust_span"] = f.arc(apex[1], 0.0, f.angle(apex))
    # The pin mark below the plate: the armhole depth for the form's size (p. 31), 1/2 inch on a
    # size 8 form unless the targets give it.
    m["side_length"] = f.along(seam, plate_bottom - targets.get("_armhole_depth", 0.5 * INCH), waist)
    back_neck = [p for p in f.tape_points("neckline_back") if p[0] >= 0]
    m["back_neck"] = f.polyline(sorted(back_neck, key=lambda p: p[0]))
    m["shoulder_length"] = f.polyline(f.tape_points("shoulder_seam"))
    m["across_shoulder_front"] = f.tape(cfn, tip, "front")
    m["across_shoulder_back"] = f.tape(cbn, tip, "back")
    level = plate_mid + 1 * INCH
    m["across_chest"] = f.arc(level, 0.0, f.tape_angle_at("armhole", level, front=True))
    m["across_back"] = f.arc(level, math.pi, f.tape_angle_at("armhole", level, front=False))
    # Bust arc ends 2 inches below the plate at the side seam; the form's bust line is level
    # with that point, so the arc is read along the bust line.
    m["bust_arc"] = f.arc(bust, 0.0, side(bust))
    m["back_arc"] = f.arc(plate_bottom, math.pi, f.angle(f.point_on(armhole, plate_bottom)))
    m["waist_arc_front"] = f.arc(waist, 0.0, side(waist))
    m["waist_arc_back"] = f.arc(waist, math.pi, side(waist))
    m["dart_placement_front"] = f.arc(waist, 0.0, f.tape_angle_at("princess_front", waist))
    m["dart_placement_back"] = f.arc(waist, math.pi, f.tape_angle_at("princess_back", waist))
    ab = waist - 3 * INCH
    m["abdomen_arc_front"] = f.arc(ab, 0.0, side(ab))
    m["abdomen_arc_back"] = f.arc(ab, math.pi, side(ab))
    m["hip_arc_front"] = f.arc(hip, 0.0, side(hip))
    m["hip_arc_back"] = f.arc(hip, math.pi, side(hip))
    m["hip_depth_front"] = f.line("front", (0, waist), (0, hip))
    m["hip_depth_back"] = f.line("back", (0, waist), (0, hip))
    m["side_hip_depth"] = f.along(seam, hip, waist)
    m["neck"] = f.girth(f.station("neck"))
    m["shoulder_angle"] = f.shoulder_angle(hps, tip)
    m["plate_height"] = max(p[1] for p in armhole) - plate_bottom
    m["plate_width"] = max(p[2] for p in armhole) - min(p[2] for p in armhole)
    m["bust_level_below_plate"] = plate_bottom - bust
    return m


def men(f):
    """Armstrong's men's form chart (New York Form Co.), measured as on pp. 495-496."""
    m = {}
    waist, hip = f.station("waist"), f.station("hip")
    cfn, cbn, hps = f.landmark("front_neck"), f.landmark("back_neck"), f.landmark("side_neck")
    tip = f.landmark("shoulder_point")
    armhole = f.tape_points("armhole")
    plate_bottom = min(p[1] for p in armhole)
    plate_mid = (plate_bottom + max(p[1] for p in armhole)) / 2
    m["chest"] = f.girth(plate_bottom - 2 * INCH)
    m["waist"] = f.girth(waist)
    m["hip"] = f.girth(hip)
    # Lengths end at the bottom of the waist tape (1/2 inch wide).
    band = waist - BAND / 2
    m["full_length_back"] = f.line("back", (hps[0], hps[1]), (hps[0], band))
    m["full_length_front"] = f.line("front", (hps[0], hps[1]), (hps[0], band))
    m["center_length_back"] = f.line("back", (0, cbn[1]), (0, band))
    m["center_length_front"] = f.centre_front(cfn[1], band)
    m["across_shoulder_back"] = f.tape(cbn, tip, "back")
    m["across_shoulder_front"] = f.tape(cfn, tip, "front")
    m["shoulder_slope_back"] = f.tape(f.radial(math.pi, band), tip, "back")
    m["shoulder_slope_front"] = f.tape(f.radial(0.0, band), tip, "front")
    m["across_back"] = f.arc(plate_mid, math.pi, f.tape_angle_at("armhole", plate_mid, front=False))
    m["across_chest"] = f.arc(plate_mid, 0.0, f.tape_angle_at("armhole", plate_mid, front=True))
    m["shoulder_length"] = f.polyline(f.tape_points("shoulder_seam"))
    m["seat_depth"] = f.line("back", (0, waist), (0, hip))
    back_neck = [p for p in f.tape_points("neckline_back") if p[0] >= 0]
    m["back_neck"] = f.polyline(sorted(back_neck, key=lambda p: p[0]))
    m["neck"] = f.girth(f.station("neck"))
    m["shoulder_angle"] = f.shoulder_angle(hps, tip)
    m["plate_height"] = max(p[1] for p in armhole) - plate_bottom
    m["plate_width"] = max(p[2] for p in armhole) - min(p[2] for p in armhole)
    return m


def main(form_path, targets_path):
    data = json.load(open(form_path))
    f = Form(bpy.data.objects["Form"], data)
    targets = json.load(open(targets_path)) if targets_path else {}
    m = men(f) if "chest" in data["stations"] else women(f, targets)
    print(f"MEASURE {data['id']}")
    for name in sorted(data["landmarks"]):
        p = f.landmark(name)
        if p:
            print(f"POINT {name:16s} x {p[0]:6.1f}  y {p[1]:6.1f}  z {p[2]:6.1f}")
    off = 0
    for name, value in m.items():
        t = targets.get(name)
        if t is None:
            print(f"MEASURE {name:24s} {value:7.1f}")
            continue
        d = value - t
        flag = "  <<" if abs(d) > TOLERANCE else ""
        off += abs(d) > TOLERANCE
        print(f"MEASURE {name:24s} {value:7.1f}   book {t:6.1f}   {d:+5.1f}{flag}")
    print(f"MEASURE {off} of {sum(1 for k in m if k in targets)} outside {TOLERANCE} cm")


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :]
    main(args[0], args[1] if len(args) > 1 else None)
