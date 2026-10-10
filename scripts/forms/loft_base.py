"""Builds a dress form in Blender, headless, the way a form is made: a padded body, a neck set on
it, and flat armhole plates.

    blender -b --factory-startup --python-exit-code 1 --python scripts/forms/loft_base.py -- <form-id> <out.blend>

The body is lofted from control rows (half-width, front and back depth, squareness, centre) by a
clamped cubic B-spline, so it is smooth in curvature, with soft bumps for the bust and shoulder
blades. Its top closes over the shoulders. The neck is a forward-leaning elliptic tube joined to the
body with a smooth fillet, and cut on a slant at the top, where the metal cap sits. Each side is
cut by a vertical plane, which leaves a flat oval armhole plate where the arm would be. Every
horizontal ring of the mesh is found by casting rays out from the centre line against this shape,
written as a signed distance, so each ring is star-shaped around the centre line.

The tape lines are worked out from the shape:
- the neckline runs where the neck meets the body (the base of the neck);
- the shoulder seam follows the top of the shoulder, from the neckline to the top of the plate;
- the armhole is the edge of the plate; the side seam drops straight down from its lowest point;
- the front princess line runs from mid-shoulder through the bust point to the waist and hip, the
  back one through the shoulder-blade point;
- centre front and centre back run straight down the middle.

Coordinates are OpenDrape's (cm here, y up, front +z, the pole on x = 0, z = 0) and become
Blender's (metres, z up, front -y) only when vertices are created. References: Armstrong,
Patternmaking for Fashion Design, ch. 2 (form landmarks and measurements) and ch. 3 (fitting).
"""
import math
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Vector

# Body control rows: y, half-width w, front depth f, back depth b, squareness n, centre z (cm).
# Around the arms the body is wider than the plates; the plate cut makes it flat there.
ROWS = {
    "women-torso": [
        # Proportions checked against a studio photo of an industry form; lengths follow
        # Armstrong's size 8 form (ch. 2).
        (58.4, 8.4, 5.2, 6.2, 2.0, 0.5),
        (59.0, 13.4, 8.0, 9.6, 2.1, 0.49),
        (60.2, 15.2, 9.0, 10.8, 2.1, 0.47),
        (65.5, 16.5, 9.89, 11.61, 2.1, 0.2),
        (73.8, 17.46, 10.68, 12.21, 2.1, -0.12),
        (83.6, 17.87, 10.98, 12.29, 2.1, -0.17),
        (89.8, 17.12, 10.78, 11.91, 2.05, 0.04),
        (96.4, 15.69, 10.59, 11.21, 2.0, 0.43),
        (103.0, 10.78, 8.9, 9.0, 2.0, 0.81),
        (110.0, 13.1, 9.25, 9.41, 2.0, 1.09),
        # Under the bust the front is flat like the ribcage (the seventh value), so the bust shows
        # as two mounds with a shallow valley between them.
        (115.5, 15.17, 9.39, 10.13, 2.02, 1.37, 2.4),
        (120.5, 15.93, 10.75, 9.18, 2.05, 0.48, 2.8),
        # Round the arms, flatter at the sides and widest low down: the plate cut leaves a flat
        # egg-shaped oval, about 9.4 cm wide and 12 cm tall (Armstrong ch. 2).
        (124.0, 16.47, 10.84, 9.02, 2.33, -0.06, 2.6),
        (127.5, 17.89, 10.14, 8.78, 2.61, -0.57, 2.55),
        (131.0, 18.8, 8.72, 8.39, 2.56, -0.75),
        (134.5, 18.49, 7.23, 8.19, 2.56, -0.35),
        # The shoulders fall from the neck at about 22 degrees (Armstrong's ideal is about 25) and
        # round into the plate; the top of the body slopes from the front of the neck up to the back.
        (136.87, 17.82, 6.35, 7.68, 2.33, -0.63),
        (138.41, 16.71, 5.54, 7.01, 2.15, -1.09),
        (139.31, 15.7, 4.88, 6.58, 2.08, -1.38),
        # Near the neck the top of the shoulder (where each row is widest, at its centre value)
        # stays about 2 cm behind the neck's centre, so the shoulder seam meets the neck just behind
        # its side; the front falls from it towards the chest, the back towards the blades.
        (140.11, 14.51, 4.17, 6.22, 2.06, -1.6),
        (140.81, 13.23, 3.47, 5.91, 2.05, -1.76),
        (141.61, 11.55, 2.66, 5.54, 2.03, -1.96),
        (142.41, 9.71, 1.92, 5.11, 2.02, -2.19),
        (143.21, 7.85, 1.34, 4.56, 2, -2.52),
        # The upper back rises higher than the side of the neck, so the base of the neck climbs
        # steadily from the front to the back. Above it the body runs on into the neck and closes
        # well inside it, out of reach of the fillet, so the neck rises cleanly from the shoulders.
        (144.01, 6.09, 0.96, 3.87, 2, -2.95),
        (144.81, 4.55, 0.75, 3.09, 2, -3.45),
        (145.61, 3.3, 0.6, 2.35, 2.0, -3.9),
        (146.41, 2.4, 0.4, 1.78, 2.0, -4.2),
        (147.21, 1.6, 0.25, 1.34, 2.0, -4.3),
        (148.01, 0.9, 0.2, 0.87, 2.0, -4.35),
        (148.61, 0.4, 0.12, 0.37, 2.0, -4.45),
        (148.91, 0.05, 0.05, 0.1, 2.0, -4.5),
    ],
    # Lengths from the New York Form Company Young Men's 40 form (Armstrong ch. 23), neck from
    # Kershaw's US 40 chart.
    "men-torso": [
        (61.4, 9.6, 5.8, 6.8, 2.0, 0.5),
        (62.0, 14.6, 8.8, 10.4, 2.1, 0.49),
        (63.2, 16.6, 9.8, 11.8, 2.1, 0.47),
        (68.0, 17.6, 10.4, 12.6, 2.1, 0.41),
        (75.0, 18.9, 11.0, 13.4, 2.1, 0.31),
        (82.0, 19.64, 11.2, 13.7, 2.1, 0.31),
        (90.0, 18.6, 11.0, 12.7, 2.05, 0.59),
        (96.0, 16.8, 10.8, 11.5, 2.05, 0.81),
        (103.0, 15.18, 10.4, 10.6, 2.05, 0.91),
        (110.0, 16.3, 10.9, 11.2, 2.05, 1.11),
        (118.0, 17.66, 11.39, 12.53, 2.08, 1.1),
        (126.0, 18.85, 11.79, 13.57, 2.12, 0.98),
        (130.24, 19.66, 11.8, 12.96, 2.14, 0.39),
        (134.53, 21.79, 11.19, 11.71, 2.16, -0.22),
        (138.22, 23.02, 10.38, 10.46, 2.16, -0.72),
        (141.45, 22.77, 9.14, 9.55, 2.14, -1.16),
        (143.53, 21.2, 8.08, 8.51, 2.12, -1.68),
        (144.8, 19.59, 7.34, 7.62, 2.1, -2.13),
        (145.8, 17.93, 6.65, 6.84, 2.08, -2.49),
        (146.61, 16.38, 6.0, 6.18, 2.06, -2.79),
        (147.41, 14.68, 5.29, 5.5, 2.04, -3.07),
        (148.21, 12.9, 4.52, 4.83, 2.02, -3.34),
        (149.01, 11.03, 3.68, 4.16, 2.0, -3.6),
        (150.05, 8.51, 2.27, 3.45, 2.0, -3.74),
        (150.85, 6.78, 1.39, 2.87, 2.0, -3.92),
        (151.65, 4.85, 0.79, 2.23, 2.0, -4.18),
        (152.45, 3.16, 0.45, 1.58, 2.0, -4.46),
        (153.25, 1.71, 0.28, 0.99, 2.0, -4.67),
        (154.05, 0.87, 0.26, 0.47, 2.0, -4.82),
        (154.65, 0.41, 0.35, 0.13, 2.0, -4.86),
        (154.95, 0.24, 0.43, -0.02, 2.0, -4.86),
    ],
}
# Bumps added to the depth: (front?, x, y, sigma x, sigma y above, sigma y below, amplitude), cm.
# The bust is one soft mass with a shallow valley at centre front and a fuller underside.
BUMPS = {
    "women-torso": [
        (True, 11.97, 120.14, 6.0, 5.74, 5.5, 3.45),
        (False, 8.5, 130.5, 7.0, 7.0, 7.0, 0.7),
    ],
    "men-torso": [
        (True, 9.5, 126.0, 8.5, 8.0, 5.0, 0.69),
        (False, 9.5, 135.05, 8.0, 7.24, 7.24, 0.75),
    ],
}
# The neck: an elliptic tube leaning forward, its axis on the pole at the height of the neck cut.
# Half-width a and half-depth b at the base (y_base), narrowing by `taper` per cm upwards; the
# fillet where it meets the body has radius about `blend` at the sides, `blend_front` at the front
# and `blend_back` at the back: there the chest and the upper back run into the neck at a shallow
# angle, and a wide fillet would swell out past both.
NECK = {
    "women-torso": {"a": 5.56, "b": 5.99, "y_base": 142.0, "taper": 0.018, "lean": 0.0614, "blend": 2.24,
                    "blend_front": 1.0, "blend_back": 1.2, "bottom": 134.0},
    "men-torso": {"a": 6.08, "b": 6.8, "y_base": 146.4, "taper": 0.02, "lean": 0.09, "blend": 2.46, "blend_front": 1.1, "blend_back": 1.3, "bottom": 138.4},
}
# The armhole plates: the body is cut by the planes |x| = x; `edge` rounds the plate's rim.
PLATE = {
    "women-torso": {"x": 16.92, "edge": 1.0},
    "men-torso": {"x": 19.95, "edge": 1.0},
}
# The neck is cut by a plane lower at the front: (height where it meets the pole axis, tilt in
# degrees), as on a real form, where a metal cap sits on the cut.
NECK_CUT = {"women-torso": (152.0, 17.0), "men-torso": (156.5, 17.0)}
# Princess lines below the bust and blade points: where they cross the waist (arc from centre
# front or back, cm: Armstrong's dart placement) and their distance from the centre line in the
# front or back view at the hip and the bottom.
PRINCESS = {
    "women-torso": {"front": {"waist_arc": 7.94, "hip_x": 8.8, "bottom_x": 9.0},
                    "back": {"waist_arc": 7.94, "hip_x": 9.3, "bottom_x": 9.5}},
    "men-torso": {"front": {"waist_arc": 9.6, "hip_x": 9.9, "bottom_x": 10.1},
                  "back": {"waist_arc": 9.6, "hip_x": 10.5, "bottom_x": 10.7}},
}
# Stations (ring heights); the women's "bust" station is set at the bust point's height.
STATION_HEIGHTS = {
    "women-torso": {"bottom": 61.0, "hip": 83.6, "high_hip": 96.0, "waist": 103.0, "under_bust": 112.5,
                    "shoulder": 136.0, "neck": 148.0},
    "men-torso": {"bottom": 64.0, "hip": 82.0, "high_hip": 95.0, "waist": 103.0, "chest": 126.91, "shoulder": 140.03, "neck": 152.4},
}


def bspline(ctrl):
    """Clamped uniform cubic B-spline through the control rows; returns row(y)."""
    n, p = len(ctrl) - 1, 3
    knots = [0.0] * (p + 1) + [i / (n - p + 1) for i in range(1, n - p + 1)] + [1.0] * (p + 1)

    def at(t):
        if t >= 1.0:
            return list(ctrl[-1])
        k = max(i for i in range(p, n + 1) if knots[i] <= t)
        d = [list(ctrl[j + k - p]) for j in range(p + 1)]
        for r in range(1, p + 1):
            for j in range(p, r - 1, -1):
                lo, hi = knots[j + k - p], knots[j + 1 + k - r]
                a = (t - lo) / (hi - lo)
                d[j] = [(1 - a) * u + a * v for u, v in zip(d[j - 1], d[j])]
        return d[p]

    table = [at(i / 4000) for i in range(4001)]

    def row(y):
        lo, hi = 0, len(table) - 1
        while hi - lo > 1:
            mid = (lo + hi) // 2
            if table[mid][0] <= y:
                lo = mid
            else:
                hi = mid
        a, b = table[lo], table[hi]
        s = 0.0 if b[0] == a[0] else (y - a[0]) / (b[0] - a[0])
        return [u + (v - u) * s for u, v in zip(a, b)]

    return row


def section_point(form, row, phi):
    """Point (x, z) in cm on the body section `row`, for parameter phi from the front.

    A row may carry a seventh value, a squareness for the front half alone: a flatter front
    (like the ribcage under the bust) lets the bust stand out as two mounds.
    """
    y, w, f, b, n, zc = row[:6]
    n_front = row[6] if len(row) > 6 else n
    s, c = math.sin(phi), math.cos(phi)
    x = w * math.copysign(abs(s) ** (2.0 / n), s)
    depth = (f + b) / 2 + (f - b) / 2 * c
    z = depth * math.copysign(abs(c) ** (2.0 / (n_front if c > 0 else n)), c)
    for front, bx, by, sx, sy_up, sy_down, amp in BUMPS.get(form, []):
        cc = c if front else -c
        weight = min(max((cc - 0.25) / 0.5, 0.0), 1.0)
        weight = weight * weight * (3 - 2 * weight)
        if weight:
            sy = sy_up if y > by else sy_down
            # A mirrored pair, so the two mounds meet in a soft valley at the centre line.
            g = (math.exp(-((x - bx) / sx) ** 2) + math.exp(-((x + bx) / sx) ** 2)) * math.exp(-((y - by) / sy) ** 2)
            z += (1.0 if front else -1.0) * amp * g * weight
    return x, z + zc


def smin(a, b, k):
    """Smooth minimum (union) of two distances, with a fillet of about k."""
    h = np.clip(0.5 + 0.5 * (b - a) / k, 0.0, 1.0)
    return b + (a - b) * h - k * h * (1.0 - h)


def smin_c2(a, b, k):
    """Smooth minimum whose curvature also fades out smoothly at the edge of the blend (cubic), so
    the edge of a long, shallow blend leaves no line on the surface."""
    h = np.maximum(k - np.abs(a - b), 0.0) / k
    return np.minimum(a, b) - h ** 3 * k / 6.0


class Form:
    """The form as a signed distance (cm, negative inside), evaluated on numpy arrays."""

    def __init__(self, form, dy=0.1, nth=720):
        self.form = form
        # Every row gets a front squareness (its own squareness unless given), so all blend alike.
        rows = [tuple(r) if len(r) > 6 else tuple(r) + (r[4],) for r in ROWS[form]]
        self.row = bspline(rows)
        self.y0, self.y1 = rows[0][0], rows[-1][0]
        self.dy, self.nth = dy, nth
        self.ys = np.arange(self.y0, self.y1 + 1e-9, dy)
        th = 2 * np.pi * np.arange(nth) / nth
        self.cz = np.empty(len(self.ys))
        self.R = np.empty((len(self.ys), nth))
        for i, y in enumerate(self.ys):
            r = self.row(y)
            pts = np.array([section_point(form, r, 2 * math.pi * j / 360) for j in range(360)])
            cz = r[5] + (r[2] - r[3]) / 2
            ang = np.arctan2(pts[:, 0], pts[:, 1] - cz) % (2 * np.pi)
            rad = np.hypot(pts[:, 0], pts[:, 1] - cz)
            order = np.argsort(ang)
            ang, rad = ang[order], rad[order]
            self.R[i] = np.interp(th, np.concatenate([ang - 2 * np.pi, ang, ang + 2 * np.pi]), np.tile(rad, 3))
            self.cz[i] = cz
        self.dR = np.gradient(self.R, dy, axis=0)
        self.neck = NECK[form]
        self.cut_y, cut_deg = NECK_CUT[form]
        self.cut_tan = math.tan(math.radians(cut_deg))
        self.cos_lean = 1 / math.sqrt(1 + self.neck["lean"] ** 2)
        self.plate_x, self.plate_edge = PLATE[form]["x"], PLATE[form]["edge"]

    # The parts.
    def _lookup(self, table, y, th):
        fi = np.clip((y - self.y0) / self.dy, 0, len(self.ys) - 1.0001)
        i = fi.astype(int)
        t = fi - i
        fj = th / (2 * np.pi) * self.nth
        j = np.floor(fj).astype(int) % self.nth
        u = fj - np.floor(fj)
        j1 = (j + 1) % self.nth
        a = table[i, j] * (1 - u) + table[i, j1] * u
        b = table[i + 1, j] * (1 - u) + table[i + 1, j1] * u
        return a * (1 - t) + b * t

    def body_centre(self, y):
        return np.interp(y, self.ys, self.cz)

    def d_body(self, x, y, z):
        cz = self.body_centre(y)
        th = np.arctan2(x, z - cz) % (2 * np.pi)
        rho = np.hypot(x, z - cz)
        R = self._lookup(self.R, y, th)
        dR = self._lookup(self.dR, y, th)
        d = (rho - R) / np.sqrt(1 + dR * dR)
        return np.maximum(np.maximum(d, y - self.y1), self.y0 - y)

    def neck_axis(self, y):
        return self.neck["lean"] * (y - self.cut_y)

    def neck_ab(self, y):
        nk = self.neck
        shrink = nk["taper"] * (y - nk["y_base"])
        return nk["a"] - shrink, nk["b"] - shrink

    def d_neck(self, x, y, z):
        a, b = self.neck_ab(y)
        zr = (z - self.neck_axis(y)) * self.cos_lean
        e = np.sqrt((x / a) ** 2 + (zr / b) ** 2)
        d = (e - 1.0) * (a + b) / 2
        return np.maximum(d, self.neck["bottom"] - y)

    def blend(self, x, y, z):
        """Fillet size round the neck: `blend` at the sides, easing to `blend_front` at the front
        and `blend_back` at the back."""
        nk = self.neck
        dz = z - self.neck_axis(y)
        c = dz / np.maximum(np.hypot(x, dz), 1e-6)
        wf, wb = (np.clip((v - 0.2) / 0.6, 0.0, 1.0) for v in (c, -c))
        wf, wb = wf * wf * (3 - 2 * wf), wb * wb * (3 - 2 * wb)
        return nk["blend"] + (nk["blend_front"] - nk["blend"]) * wf + (nk["blend_back"] - nk["blend"]) * wb

    def d_joined(self, x, y, z):
        """Body and neck joined with a fillet, before the plates are cut."""
        return smin(self.d_body(x, y, z), self.d_neck(x, y, z), self.blend(x, y, z))

    def d(self, x, y, z):
        """The finished form (the neck-top cut is applied to the rings separately). Near the top of
        the plate the body meets the plate's plane at a shallow angle, so the rim's rounding spreads
        a few cm onto the shoulder; it fades out with its curvature, leaving no crease."""
        du = self.d_joined(x, y, z)
        dp = np.abs(x) - self.plate_x
        return -smin_c2(-du, -dp, self.plate_edge)

    def grad(self, x, y, z, h=0.02):
        gx = (self.d(x + h, y, z) - self.d(x - h, y, z)) / (2 * h)
        gy = (self.d(x, y + h, z) - self.d(x, y - h, z)) / (2 * h)
        gz = (self.d(x, y, z + h) - self.d(x, y, z - h)) / (2 * h)
        return gx, gy, gz

    # Rays and projections.
    def ray(self, ox, oy, oz, ux, uy, uz, t_max=60.0, step=0.25, inside_start=False):
        """First crossing of the surface along rays o + t u (arrays broadcast together). From
        outside (default) to the first point inside; from inside to the first point outside.
        Returns t, nan where there is no crossing."""
        shape = np.broadcast(ox, oy, oz, ux, uy, uz).shape
        ox, oy, oz, ux, uy, uz = (np.broadcast_to(np.asarray(v, float), shape) for v in (ox, oy, oz, ux, uy, uz))
        sign = -1.0 if inside_start else 1.0
        a = np.zeros(shape)
        b = np.full(shape, np.nan)
        found = np.zeros(shape, bool)
        t = 0.0
        while t < t_max and not found.all():
            t2 = t + step
            crossed = (~found) & (sign * self.d(ox + ux * t2, oy + uy * t2, oz + uz * t2) < 0)
            a[crossed], b[crossed] = t, t2
            found |= crossed
            t = t2
        for _ in range(30):
            m = np.where(found, (a + b) / 2, 0.0)
            past = sign * self.d(ox + ux * m, oy + uy * m, oz + uz * m) < 0
            b = np.where(found & past, m, b)
            a = np.where(found & ~past, m, a)
        return np.where(found, (a + b) / 2, np.nan)

    def project(self, p, iters=6):
        """Moves points (N, 3) onto the surface along the distance gradient."""
        p = np.array(p, float)
        for _ in range(iters):
            x, y, z = p[:, 0], p[:, 1], p[:, 2]
            d = self.d(x, y, z)
            gx, gy, gz = self.grad(x, y, z)
            g2 = gx * gx + gy * gy + gz * gz + 1e-12
            p = p - np.stack([gx, gy, gz], axis=1) * (d / g2)[:, None]
        return p

    def front_point(self, x, y):
        x, y = np.broadcast_arrays(np.asarray(x, float), np.asarray(y, float))
        t = self.ray(x, y, 40.0, 0.0, 0.0, -1.0, t_max=80.0, step=0.2)
        return np.stack([x, y, 40.0 - t], axis=-1)

    def back_point(self, x, y):
        x, y = np.broadcast_arrays(np.asarray(x, float), np.asarray(y, float))
        t = self.ray(x, y, -40.0, 0.0, 0.0, 1.0, t_max=80.0, step=0.2)
        return np.stack([x, y, -40.0 + t], axis=-1)

    # Rings.
    def ring_centre(self, y):
        """A point inside the form at height y: the body's centre, then the neck's axis."""
        w = np.clip((y - (self.neck["y_base"] - 3.0)) / 3.0, 0.0, 1.0)
        w = w * w * (3 - 2 * w)  # eased, so the mesh's vertical lines don't kink
        return self.body_centre(y) * (1 - w) + self.neck_axis(y) * w

    def z_line(self, y):
        """Where the neck-cut plane crosses height y (points in front of it are cut away)."""
        return (self.cut_y - y) / self.cut_tan

    def neck_back(self, y):
        return self.neck_axis(y) - self.neck_ab(y)[1] / self.cos_lean

    def neck_front(self, y):
        return self.neck_axis(y) + self.neck_ab(y)[1] / self.cos_lean

    def ring(self, y, around):
        phi = 2 * np.pi * np.arange(around) / around
        cz = float(self.ring_centre(y))
        t = self.ray(0.0, y, cz, np.sin(phi), 0.0, np.cos(phi), t_max=40.0, step=0.5, inside_start=True)
        x, z = t * np.sin(phi), cz + t * np.cos(phi)
        zl = self.z_line(y)
        if z.max() > zl:
            # Above the cut: project the cut-away points onto the plane, from a point inside.
            mz = (self.neck_back(y) + min(zl, z.max())) / 2
            cut = z > zl
            x = np.where(cut, x * (zl - mz) / (z - mz), x)
            z = np.where(cut, zl, z)
        return x, z

    def top(self):
        """Height of the back of the neck cut (the top of the form)."""
        lo, hi = self.cut_y, self.cut_y + 20.0
        for _ in range(60):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if self.z_line(mid) > self.neck_back(mid) else (lo, mid)
        return lo

    def cut_start(self):
        """Height where the neck cut first meets the front of the neck."""
        lo, hi = self.cut_y - 20.0, self.cut_y
        for _ in range(60):
            mid = (lo + hi) / 2
            lo, hi = (mid, hi) if self.z_line(mid) > self.neck_front(mid) else (lo, mid)
        return lo


# The tape lines.
def resample(points, n=None, step=None, closed=False):
    """Evenly spaced points along a polyline (N, 3)."""
    p = np.asarray(points, float)
    if closed:
        p = np.vstack([p, p[:1]])
    seg = np.linalg.norm(np.diff(p, axis=0), axis=1)
    s = np.concatenate([[0.0], np.cumsum(seg)])
    count = n if n else max(2, int(s[-1] / step) + 1)
    t = np.linspace(0.0, s[-1], count, endpoint=not closed)
    return np.stack([np.interp(t, s, p[:, k]) for k in range(3)], axis=1)


def catmull_rom(points, per_span=40):
    p = np.asarray(points, float)
    p = np.vstack([p[:1], p, p[-1:]])
    out = []
    for i in range(1, len(p) - 2):
        p0, p1, p2, p3 = p[i - 1], p[i], p[i + 1], p[i + 2]
        for q in range(per_span):
            t = q / per_span
            out.append(0.5 * (2 * p1 + (p2 - p0) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t
                              + (3 * p1 - p0 - 3 * p2 + p3) * t ** 3))
    out.append(p[-2])
    return np.array(out)


def arc_point(F, y, arc, back=False):
    """The point on the ring at height y, `arc` cm along the surface from centre front (or back),
    on the form's left side."""
    u = np.linspace(0.0, math.pi / 2, 400)
    phi = math.pi - u if back else u
    cz = float(F.ring_centre(y))
    t = F.ray(0.0, y, cz, np.sin(phi), 0.0, np.cos(phi), t_max=40.0, step=0.5, inside_start=True)
    pts = np.stack([t * np.sin(phi), np.full_like(t, y), cz + t * np.cos(phi)], axis=1)
    s = np.concatenate([[0.0], np.cumsum(np.linalg.norm(np.diff(pts, axis=0), axis=1))])
    return np.array([np.interp(arc, s, pts[:, k]) for k in range(3)])


def tapes(F):
    """All tape lines and the landmarks they define, as (N, 3) arrays in cm (left side, +x)."""
    out, marks = {}, {}
    nk, X = F.neck, F.plate_x
    # The plate: where the joined body reaches past the plane x = X.
    ys = np.arange(100.0, F.y1, 0.05)
    zs = np.arange(-20.0, 20.0, 0.02)
    Y, Z = np.meshgrid(ys, zs, indexing="ij")
    inside = F.d_joined(np.full_like(Y, X), Y, Z) < 0
    fronts, backs = [], []
    for i in np.where(inside.any(axis=1))[0]:
        idx = np.where(inside[i])[0]
        backs.append((X, ys[i], zs[idx[0]]))
        fronts.append((X, ys[i], zs[idx[-1]]))
    loop = resample(np.array(fronts[::-1] + backs), n=96, closed=True)
    out["armhole"] = loop
    marks["plate_centre"] = loop.mean(axis=0)
    marks["armhole_bottom"] = loop[np.argmin(loop[:, 1])]
    tip = loop[np.argmax(loop[:, 1])]
    level = marks["plate_centre"][1]
    pair = loop[np.argsort(np.abs(loop[:, 1] - level))[:8]]
    marks["armhole_front"] = pair[np.argmax(pair[:, 2])]
    marks["armhole_back"] = pair[np.argmin(pair[:, 2])]

    # The neckline: the seam where the neck meets the body, in the crease of the fillet, where the
    # body and the neck are equally near. Found up each line of the surface round the neck axis.
    th = np.linspace(0.0, 2 * np.pi, 721)[:-1]
    ux, uz = np.sin(th), np.cos(th)
    lo = np.full(th.shape, nk["bottom"] + 0.5)
    hi = np.full(th.shape, F.cut_y - 3.0)

    def neck_line_point(y):
        cz = F.neck_axis(y)
        t = F.ray(0.0, y, cz, ux, 0.0, uz, t_max=40.0, step=0.1, inside_start=True)
        return t * ux, y, cz + t * uz

    for _ in range(36):
        mid = (lo + hi) / 2
        x, y, z = neck_line_point(mid)
        above = F.d_body(x, y, z) > F.d_neck(x, y, z)
        hi = np.where(above, mid, hi)
        lo = np.where(above, lo, mid)
    x, y, z = neck_line_point((lo + hi) / 2)
    ring = np.stack([x, y, z], axis=1)

    # The shoulder seam: along the top of the shoulder, from the neckline out to the top of the
    # plate. It ends at the shoulder tip, on the armhole ridge (the roll line) just above the plate,
    # where Armstrong pins the shoulder tip (p. 31). Seen from above it is a smooth curve through
    # the highest points of the shoulder, clear of the neck's fillet, carried on to the neckline,
    # which it meets at the side neck point.
    x0 = nk["a"] + nk["blend"] + 0.8
    xs = np.linspace(x0, X - 0.3 * F.plate_edge, 40)
    zg = np.arange(-12.0, 8.0, 0.05)
    XX, ZZ = np.meshgrid(xs, zg, indexing="ij")
    above = F.y1 + 5.0  # a height above the whole form, to look down from
    top_y = above - F.ray(XX, above, ZZ, 0.0, -1.0, 0.0, t_max=40.0, step=0.2)
    best = np.argmax(np.where(np.isnan(top_y), -1e9, top_y), axis=1)
    fit = np.polyfit(xs, zg[best], 2)
    n = len(ring)
    near = [i for i in range(n) if ring[i, 0] > 0.5 * nk["a"]]
    k = min(near, key=lambda i: abs(ring[i, 2] - np.polyval(fit, ring[i, 0])))
    hps = ring[k]
    # From the side neck point out to the tip, closer together near the neck where the line turns
    # down out of the crease; the small offset at the neck fades out over the first few cm.
    u = np.linspace(0.0, 1.0, 80)
    xs = hps[0] + (X - 0.3 * F.plate_edge - hps[0]) * u ** 1.5
    offset = (hps[2] - np.polyval(fit, hps[0])) * np.clip(1 - (xs - hps[0]) / 3.0, 0.0, 1.0) ** 2
    cz = np.polyval(fit, xs) + offset
    crest = np.stack([xs, above - F.ray(xs, above, cz, 0.0, -1.0, 0.0, t_max=40.0, step=0.1), cz], axis=1)
    crest[0] = hps
    tip = crest[-1]
    out["shoulder_seam"] = resample(crest, step=0.4)
    marks["side_neck"], marks["shoulder_point"] = hps, tip
    # Neckline halves: the front from the right neck point through centre front to the left one,
    # the back from the left neck point through centre back to the right one.
    out["neckline_front"] = resample(np.array([ring[(n - k + i) % n] for i in range(2 * k + 1)]), step=0.4)
    out["neckline_back"] = resample(np.array([ring[(k + i) % n] for i in range(n - 2 * k + 1)]), step=0.4)
    cfn, cbn = ring[0], ring[n // 2]
    marks["front_neck"], marks["back_neck"] = cfn, cbn

    # The bust point: the most prominent point of the bust.
    bust = [b for b in BUMPS[F.form] if b[0]][0]
    blade_bump = [b for b in BUMPS[F.form] if not b[0]][0]
    gx, gy = np.meshgrid(np.arange(bust[1] - 4.5, bust[1] + 4.5, 0.1), np.arange(bust[2] - 5.0, bust[2] + 5.0, 0.1),
                         indexing="ij")
    fp = F.front_point(gx, gy)
    apex = fp[np.unravel_index(np.nanargmax(fp[..., 2]), fp[..., 2].shape)]
    if bust[6] < 1.5:
        # A man's chest has no bust point: the line runs through the middle of the chest mound.
        apex = F.front_point(bust[1], bust[2])
    # The blades on a form are gentle, so their point is the centre of the blade mound.
    blade = F.back_point(blade_bump[1], blade_bump[2])
    marks["bust_apex"], marks["shoulder_blade"] = apex, blade
    # The side seam: straight down from the bottom of the plate.
    z_s = marks["armhole_bottom"][2]
    yy = np.arange(marks["armhole_bottom"][1], F.y0 + 0.6, -0.5)
    t = F.ray(40.0, yy, z_s, -1.0, 0.0, 0.0, t_max=80.0, step=0.2)
    out["side_seam"] = np.stack([40.0 - t, yy, np.full_like(yy, z_s)], axis=1)
    # Centre front and back.
    st = STATION_HEIGHTS[F.form]
    yy = np.arange(cfn[1], st["bottom"] - 0.01, -0.5)
    out["cf"] = F.front_point(np.zeros_like(yy), yy)
    yy = np.arange(cbn[1], st["bottom"] - 0.01, -0.5)
    out["cb"] = F.back_point(np.zeros_like(yy), yy)
    # Princess lines: mid-shoulder, the bust (or blade) point, the waist, the hip, the bottom.
    seam = out["shoulder_seam"]
    s = np.concatenate([[0.0], np.cumsum(np.linalg.norm(np.diff(seam, axis=0), axis=1))])
    mid = np.array([np.interp(s[-1] / 2, s, seam[:, k]) for k in range(3)])
    for name, point, back in (("princess_front", apex, False), ("princess_back", blade, True)):
        pr = PRINCESS[F.form]["back" if back else "front"]
        waist = arc_point(F, st["waist"], pr["waist_arc"], back)
        # Seen from the front (or back) the line is a smooth curve through mid-shoulder, the bust
        # (or blade) point, the waist, the hip and the bottom, laid straight onto the surface; at
        # mid-shoulder it meets the shoulder seam on the top of the shoulder.
        anchors = [(mid[0], mid[1]), (point[0], point[1]), (waist[0], waist[1]), (pr["hip_x"], st["hip"]),
                   (pr["bottom_x"], st["bottom"])]
        guide = catmull_rom(np.array([[x, y, 0.0] for x, y in anchors]), per_span=200)
        ys = np.concatenate([np.linspace(mid[1] - 0.02, mid[1] - 2.0, 40),
                             np.arange(mid[1] - 2.2, st["bottom"] - 0.01, -0.4)])
        xs = np.interp(ys, guide[::-1, 1], guide[::-1, 0])
        surface = F.back_point if back else F.front_point
        line = np.vstack([mid, surface(xs, ys)])
        out[name] = resample(line[np.isfinite(line).all(axis=1)], step=0.4)
    marks["front_waist"] = F.front_point(0.0, st["waist"])
    marks["back_waist"] = F.back_point(0.0, st["waist"])
    marks["cf_bottom"] = F.front_point(0.0, st["bottom"])
    marks["cb_bottom"] = F.back_point(0.0, st["bottom"])
    marks["cb_blade"] = F.back_point(0.0, blade[1])
    return out, marks


def build(form, out_path, ring_step=0.25, cut_step=0.08, around=192):
    F = Form(form)
    y0, y1, yc = F.y0, F.top() - 0.02, F.cut_start()
    heights = [y0 + ring_step * i for i in range(int((yc - y0) / ring_step) + 1) if y0 + ring_step * i < yc - 0.1]
    heights += [yc + cut_step * i for i in range(int((y1 - yc) / cut_step) + 1) if yc + cut_step * i < y1 - 0.01] + [y1]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    mesh = bpy.data.meshes.new("Form")
    bm = bmesh.new()
    rings = []
    for y in heights:
        x, z = F.ring(y, around)
        rings.append([bm.verts.new((xi / 100, -zi / 100, y / 100)) for xi, zi in zip(x, z)])
    for i in range(len(rings) - 1):
        for j in range(around):
            k = (j + 1) % around
            bm.faces.new((rings[i][j], rings[i + 1][j], rings[i + 1][k], rings[i][k]))
    bottom = bm.verts.new((0, -float(F.body_centre(y0)) / 100, y0 / 100))
    last = rings[-1]
    cx = sum(v.co.x for v in last) / around
    cy = sum(v.co.y for v in last) / around
    top = bm.verts.new((cx, cy, (F.cut_y + F.cut_tan * cy * 100) / 100))
    for j in range(around):
        k = (j + 1) % around
        bm.faces.new((bottom, rings[0][k], rings[0][j]))
        bm.faces.new((top, rings[-1][j], rings[-1][k]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new("Form", mesh)
    bpy.context.scene.collection.objects.link(obj)
    for p in mesh.polygons:
        p.use_smooth = True
    # The stand: the pole axis (x, z) and the neck-cut plane (height at the axis, tilt), in metres
    # and degrees, for the exporter and the renderer.
    obj["pole_xz"] = (0.0, 0.0)
    obj["neck_cut"] = (F.cut_y / 100, NECK_CUT[form][1])
    lines, marks = tapes(F)
    add_markers(form, F, lines, marks)
    bpy.ops.wm.save_as_mainfile(filepath=out_path)


def to_blender(p):
    return Vector((p[0] / 100, -p[2] / 100, p[1] / 100))


def add_markers(form, F, lines, marks):
    """Stations as empties on the centre line, landmarks as empties, tape lines as curves."""
    scene = bpy.context.scene.collection
    heights = dict(STATION_HEIGHTS[form])
    if form == "women-torso":
        heights["bust"] = round(float(marks["bust_apex"][1]), 2)
    for name, y in heights.items():
        e = bpy.data.objects.new(f"st.{name}", None)
        e.empty_display_type, e.empty_display_size = "CIRCLE", 0.2
        e.location = (0, -float(F.ring_centre(y)) / 100, y / 100)
        scene.objects.link(e)
    for name, p in marks.items():
        e = bpy.data.objects.new(f"lm.{name}", None)
        e.empty_display_type, e.empty_display_size = "SPHERE", 0.006
        e.location = to_blender(np.asarray(p, float).reshape(3))
        scene.objects.link(e)
    col = bpy.data.collections.new("Tapes")
    scene.children.link(col)
    for name, pts in lines.items():
        pts = [p for p in np.asarray(pts, float) if np.isfinite(p).all()]
        curve = bpy.data.curves.new(f"tape.{name}", "CURVE")
        curve.dimensions = "3D"
        spline = curve.splines.new("POLY")
        spline.points.add(len(pts) - 1)
        for sp, p in zip(spline.points, pts):
            v = to_blender(p)
            sp.co = (v.x, v.y, v.z, 1.0)
        spline.use_cyclic_u = name == "armhole"
        col.objects.link(bpy.data.objects.new(f"tape.{name}", curve))


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :]
    build(args[0], args[1])
