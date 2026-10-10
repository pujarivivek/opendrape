"""Prints station girths (convex hull of each station ring) of a .form.json, in cm."""
import json
import math
import sys

form = json.load(open(sys.argv[1]))
K = form["angles"]


def ring_points(ring):
    pts = []
    m = 2 * (K - 1)
    for j in range(m):
        k = j if j < K else m - j
        a = 2 * math.pi * j / m
        r = ring["r"][k] / 1000
        pts.append((r * math.sin(a), ring["zc"] + r * math.cos(a)))
    return pts


def hull_perimeter(pts):
    pts = sorted(set(pts))
    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    hull = lower[:-1] + upper[:-1]
    return sum(math.dist(hull[i], hull[(i + 1) % len(hull)]) for i in range(len(hull)))


for name, i in form["stations"].items():
    ring = form["rings"][i]
    print(f"{name:12s} y={ring['y']:.3f}  girth {hull_perimeter(ring_points(ring)) * 100:6.1f} cm")
