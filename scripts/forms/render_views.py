"""Renders a dress form like a studio photo, for review.

    blender -b --factory-startup <form.blend> --python-exit-code 1 \
        --python scripts/forms/render_views.py -- <out.png> [<form.json>] [--closeup]
        [--fabric <f.blend> [--tile <metres>] [--beige]]

Headless (EEVEE, no window). Shows front, side, back and three-quarter views in one PNG, standing
on the floor, or with --closeup only the front and side of the upper torso, larger. The form is
covered in linen: a procedural weave, or an image fabric from a .blend holding one material
(--fabric; its images are laid on in repeats of TILE metres, --tile; --beige recolours it). With
an exported form file, the cover's seams are pressed into it where the panels of a real form are
sewn (centre front and back, princess, side, shoulder, neckline and armhole plates), from exactly
the samples the app will use, and a woven label at the bottom gives the size and the form's bust,
waist and hip. A metal neck cap with a rounded rim, a knob, and a pole on a round base complete
the look.
"""
import json
import math
import sys

import bmesh
import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

SPACING = 0.8
LINEN = (0.72, 0.58, 0.45, 1.0)
SEAM = (0.50, 0.40, 0.31, 1.0)
TAPE = (0.03, 0.03, 0.03, 1.0)
METAL = (0.75, 0.75, 0.76, 1.0)
BLACK = (0.02, 0.02, 0.02, 1.0)
SHOWN_TAPES = {"cf", "cb", "neckline_front", "neckline_back", "shoulder_seam", "armhole", "side_seam", "princess_front",
               "princess_back", "waist"}


def material(name, colour, metallic=0.0, roughness=0.75):
    m = bpy.data.materials.new(name)
    m.diffuse_color = colour
    if m.node_tree is None:
        m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = colour
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    return m


class Surface:
    """Points on the form from (angle from centre front, height), in Blender coordinates."""

    def __init__(self, obj):
        self.bvh = BVHTree.FromObject(obj, bpy.context.evaluated_depsgraph_get())

    def centre_y(self, z):
        guess = 0.0
        for _ in range(2):
            o = Vector((0, guess, z))
            f = self.bvh.ray_cast(o, Vector((0, -1, 0)), 2.0)
            b = self.bvh.ray_cast(o, Vector((0, 1, 0)), 2.0)
            if f[0] is None or b[0] is None:
                return guess
            guess = (f[0].y + b[0].y) / 2
        return guess

    def point(self, phi, z, lift=0.0008):
        o = Vector((0, self.centre_y(z), z))
        hit = self.bvh.ray_cast(o, Vector((math.sin(phi), -math.cos(phi), 0)), 2.0)
        return None if hit[0] is None else hit[0] + hit[1] * lift


def tape_curves(scene, form, form_file):
    """Curves along the seams and the waist tape, from the form file's samples."""
    surf = Surface(form)
    heights = [r["y"] for r in form_file["rings"]]
    n = len(heights)

    def height(v):
        f = min(max(v, 0.0), 1.0) * (n - 1)
        i = min(int(f), n - 2)
        return heights[i] + (heights[i + 1] - heights[i]) * (f - i)

    lines = []
    for name, tape in form_file["tapes"].items():
        if name not in SHOWN_TAPES:
            continue
        if "ring" in tape:
            v = form_file["stations"][tape["ring"]] / (n - 1)
            lines.append((name, [(2 * math.pi * j / 192, v) for j in range(192)], True))
            continue
        lines.append((name, tape["uv"], tape["closed"]))
        if tape["mirror"]:
            lines.append((name, [(2 * math.pi - phi, v) for phi, v in tape["uv"]], tape["closed"]))
    seam_mat, tape_mat = material("Seam", SEAM), material("Tape", TAPE)
    out = []
    for name, samples, closed in lines:
        world = [p for p in (surf.point(phi, height(v)) for phi, v in samples) if p is not None]
        if len(world) < 2:
            continue
        curve = bpy.data.curves.new(name, "CURVE")
        curve.dimensions = "3D"
        curve.bevel_depth = 0.0025 if name == "waist" else 0.0007
        curve.bevel_resolution = 2
        spline = curve.splines.new("POLY")
        spline.points.add(len(world) - 1)
        for sp, p in zip(spline.points, world):
            sp.co = (p.x, p.y, p.z, 1.0)
        spline.use_cyclic_u = closed
        obj = bpy.data.objects.new(name, curve)
        obj.data.materials.append(tape_mat if name == "waist" else seam_mat)
        scene.collection.objects.link(obj)
        out.append(obj)
    return out


# The seams of the linen cover, from the form file's tapes. Seams that meet (a T at the shoulder or
# the neckline) go in different channels, so each channel's groove stays sharp across the mesh.
SEAM_GROUPS = {
    "seam_a": ("cf", "cb", "princess_front", "princess_back", "side_seam"),
    "seam_b": ("shoulder_seam",),
    "seam_c": ("neckline_front", "neckline_back", "armhole"),
}
SEAM_FAR = 0.05  # metres: "no seam near here"
RUN_TO_EDGE = {"cf", "cb", "princess_front", "princess_back", "side_seam"}
DEBUG_SEAMS = False


def tape_points(surf, form_file, name):
    """A tape's polylines in Blender coordinates (two for a mirrored tape), with closed flags."""
    tape = form_file["tapes"].get(name)
    if not tape or "uv" not in tape:
        return []
    heights = [r["y"] for r in form_file["rings"]]
    n = len(heights)

    def height(v):
        f = min(max(v, 0.0), 1.0) * (n - 1)
        i = min(int(f), n - 2)
        return heights[i] + (heights[i + 1] - heights[i]) * (f - i)

    sets = [tape["uv"]] + ([[(2 * math.pi - phi, v) for phi, v in tape["uv"]]] if tape["mirror"] else [])
    out = []
    for samples in sets:
        pts = [p for p in (surf.point(phi, height(v), 0.0) for phi, v in samples) if p is not None]
        # The vertical seams run on down to the bottom edge of the cover, as on a real form.
        low = min(range(len(samples)), key=lambda i: samples[i][1])
        if name in RUN_TO_EDGE and low == len(samples) - 1:
            phi, z = samples[-1][0], height(samples[-1][1])
            pts += [p for p in (surf.point(phi, zz, 0.0) for zz in frange(z - 0.004, heights[0] + 0.004, 0.004))
                    if p is not None]
        if len(pts) >= 2:
            out.append((densify(pts, 0.001, tape["closed"]), tape["closed"]))
    return out


def frange(a, b, step):
    """a, a - step, ... down to b."""
    out = []
    while a > b:
        out.append(a)
        a -= step
    return out


def densify(pts, step, closed):
    """Points every `step` metres along a polyline."""
    if closed:
        pts = pts + [pts[0]]
    out = [pts[0]]
    for a, b in zip(pts, pts[1:]):
        k = max(1, int((b - a).length / step))
        out += [a.lerp(b, (i + 1) / k) for i in range(k)]
    return out[:-1] if closed else out


def seam_distances(form, form_file):
    """For each channel, two point attributes for the linen material: the signed distance (metres,
    across the seam on the surface) to the nearest seam, and whether that seam is near.

    The distance keeps its sign all the way out, so it only passes zero on a seam; where it flips
    between two seams' sides far from both, or past a seam's end, the "near" mask is zero, so no
    groove appears there when the shader blends between vertices."""
    from mathutils.kdtree import KDTree

    surf = Surface(form)
    me = form.data
    for attr, names in SEAM_GROUPS.items():
        lines = [line for name in names for line in tape_points(surf, form_file, name)]
        values, near = [SEAM_FAR] * len(me.vertices), [0.0] * len(me.vertices)
        if lines:
            owner = [(li, i) for li, (pts, _) in enumerate(lines) for i in range(len(pts))]
            tree = KDTree(len(owner))
            for k, (li, i) in enumerate(owner):
                tree.insert(lines[li][0][i], k)
            tree.balance()
            for vi, v in enumerate(me.vertices):
                _, k, _ = tree.find(v.co)
                li, i = owner[k]
                pts, closed = lines[li]
                d, past_end = across(v.co, v.normal, pts, i, closed)
                values[vi] = math.copysign(min(abs(d), SEAM_FAR), d)
                near[vi] = 0.0 if past_end or abs(d) > 0.008 else 1.0
        for name, data in ((attr, values), (attr + "_near", near)):
            a = me.attributes.get(name) or me.attributes.new(name, "FLOAT", "POINT")
            a.data.foreach_set("value", data)


def across(co, normal, pts, i, closed, end_slack=0.0015):
    """Signed distance from co across the polyline pts near its point i, and whether co lies more
    than `end_slack` metres past an end of an open polyline."""
    n = len(pts)
    best = None
    for j in (i - 1, i):
        if closed:
            a, b = pts[j % n], pts[(j + 1) % n]
        elif 0 <= j < n - 1:
            a, b = pts[j], pts[j + 1]
        else:
            continue
        t = b - a
        if t.length < 1e-9:
            continue
        u = (co - a).dot(t) / t.length_squared
        past = not closed and ((j == 0 and u < 0 and -u * t.length > end_slack) or
                               (j == n - 2 and u > 1 and (u - 1) * t.length > end_slack))
        c = a + t * min(max(u, 0.0), 1.0)
        d = (co - c).length
        if best is None or d < best[0]:
            side = t.cross(normal)
            signed = (co - c).dot(side.normalized()) if side.length > 1e-9 else d
            best = (d, signed if abs(signed) > 1e-9 else d, past)
    return (SEAM_FAR, True) if best is None else (best[1], best[2])


# Metres of cloth across one repeat of an image fabric, so its weave comes out as fine as a
# dress-form linen; set per fabric with --tile.
TILE = 0.15
# The natural beige of unbleached linen (sRGB), for fabrics that come in another colour (--beige).
BEIGE = (0.74, 0.66, 0.60)


def cut_panels(form, form_file, around=192):
    """Lays an image fabric on the form the way a cover is made: cut into panels along the seams
    (and round the bottom edge and the neck top), each panel flattened with its threads running
    straight up, all at the cloth's real size (TILE metres a repeat). The fabric's pattern breaks
    only at the seams, as between the panels of a real cover.

    Every seam must be one unbroken cut, or neighbouring panels stay joined and are flattened as
    one piece, which stretches the cloth. So centre front and back follow the mesh's own columns
    there (the neck gets a centre-back seam too, as a cover's neck does), the other seams are
    traced along mesh edges from point to point, and each seam's open ends are joined to the seam
    or edge they run into."""
    import heapq

    from mathutils.kdtree import KDTree

    surf = Surface(form)
    me = form.data
    nv = len(me.vertices) - 2  # ring vertices; the last two are the bottom and top centres
    rings = nv // around
    co = [v.co.copy() for v in me.vertices]
    adj = [[] for _ in co]
    for e in me.edges:
        a, b = e.vertices
        w = (co[a] - co[b]).length
        adj[a].append((b, w, e.index))
        adj[b].append((a, w, e.index))
    edge_verts = [tuple(e.vertices) for e in me.edges]
    tree = KDTree(nv)
    for i in range(nv):
        tree.insert(co[i], i)
    tree.balance()

    def path(a, b, reach=0.05):
        """The shortest chain of edges from vertex a to vertex b (close together)."""
        dist, prev, heap = {a: 0.0}, {}, [(0.0, a)]
        while heap:
            d, v = heapq.heappop(heap)
            if v == b:
                break
            if d > dist[v]:
                continue
            for u, w, ei in adj[v]:
                if u < nv and d + w < min(dist.get(u, reach), reach):
                    dist[u], prev[u] = d + w, (v, ei)
                    heapq.heappush(heap, (d + w, u))
        out, v = [], b
        while v != a:
            if v not in prev:
                return []
            v, ei = prev[v]
            out.append(ei)
        return out

    seams = []  # (vertex chain, closed?) for each seam
    # Centre front (up to the front neck) and centre back (all the way up the neck): mesh columns.
    cf_top = max((p for line, _ in tape_points(surf, form_file, "cf") for p in line), key=lambda p: p.z, default=None)
    i_top = tree.find(cf_top)[1] // around if cf_top else rings - 1
    seams.append(([i * around for i in range(i_top + 1)], False))
    seams.append(([i * around + around // 2 for i in range(rings)], False))
    for names in SEAM_GROUPS.values():
        for name in names:
            if name in ("cf", "cb"):
                continue
            for pts, closed in tape_points(surf, form_file, name):
                chain = []
                for p in pts[::3]:
                    v = tree.find(p)[1]
                    if not chain or chain[-1] != v:
                        chain.append(v)
                # Vertical seams run on down their mesh column, under the curve of the bottom,
                # to the bottom edge.
                if name in RUN_TO_EDGE and len(chain) > 1:
                    i, j = divmod(chain[-1], around)
                    if co[chain[-1]].z < co[chain[0]].z:
                        chain += [k * around + j for k in range(i - 1, -1, -1)]
                if len(chain) > 1:
                    seams.append((chain, closed))
    cut = set()
    owner = {}
    for si, (chain, closed) in enumerate(seams):
        links = list(zip(chain, chain[1:])) + ([(chain[-1], chain[0])] if closed else [])
        for a, b in links:
            found = path(a, b)
            if not found and DEBUG_SEAMS:
                print("SEAM GAP", si, tuple(round(c, 3) for c in co[a]), tuple(round(c, 3) for c in co[b]),
                      round((co[a] - co[b]).length, 4))
            for ei in found:
                cut.add(ei)
                for v in edge_verts[ei]:
                    owner.setdefault(v, set()).add(si)
    # The bottom edge of the cover and the top of the neck, where it meets the cap.
    edge_of = {tuple(sorted(ev)): ei for ei, ev in enumerate(edge_verts)}
    for i in (0, rings - 1):
        for j in range(around):
            ei = edge_of.get(tuple(sorted((i * around + j, i * around + (j + 1) % around))))
            if ei is not None:
                cut.add(ei)
                for v in edge_verts[ei]:
                    owner.setdefault(v, set()).add(-1)
    # Open ends: joined to the nearest other seam (or edge) within 2 cm.
    for si, (chain, closed) in enumerate(seams):
        if closed:
            continue
        others = [v for v, ss in owner.items() if ss - {si}]
        near = KDTree(len(others))
        for k, v in enumerate(others):
            near.insert(co[v], k)
        near.balance()
        for end in (chain[0], chain[-1]):
            if owner.get(end, set()) - {si}:
                continue
            _, k, dist = near.find(co[end])
            if k is not None and dist < 0.02:
                for ei in path(end, others[k]):
                    cut.add(ei)
    for e in me.edges:
        e.use_seam = e.index in cut
    view_layer = bpy.context.view_layer
    for o in view_layer.objects:
        o.select_set(False)
    view_layer.objects.active = form
    form.select_set(True)
    if not me.uv_layers:
        me.uv_layers.new(name="UVMap")
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.select_all(action="SELECT")
    bpy.ops.uv.unwrap(method="MINIMUM_STRETCH", margin=0.001)
    bpy.ops.uv.average_islands_scale()
    bpy.ops.uv.align_rotation(method="GEOMETRY", axis="Z")
    bpy.ops.object.mode_set(mode="OBJECT")
    # The cloth at its real size: as much cloth in the map as there is surface on the form.
    uv = me.uv_layers.active.data
    area_uv = 0.0
    for poly in me.polygons:
        pts = [uv[li].uv for li in poly.loop_indices]
        area_uv += abs(sum(pts[k].x * pts[k - 1].y - pts[k - 1].x * pts[k].y for k in range(len(pts)))) / 2
    scale = math.sqrt(sum(p.area for p in me.polygons) / area_uv) / TILE
    for d in uv:
        d.uv = d.uv * scale
    neck_grain(form, around)


def neck_grain(form, around):
    """The neck's piece is a tube cut down the centre back; flattened, its flared foot bends it into
    an arc and its threads come out slanted. A cover's neck is cut on the straight grain, so here
    the cloth is laid on directly: across, the angle round the neck's axis from the centre back
    (times the neck's radius); up, the distance along the surface from a level ring partway up the
    neck (so the cross threads run level, not along the slanted cut)."""
    me = form.data
    nv = len(me.vertices) - 2
    rings = nv // around
    co = [v.co for v in me.vertices]
    seam = {e.key for e in me.edges if e.use_seam}
    # The neck's faces: everything joined to the top ring without crossing a seam.
    edge_faces = {}
    for poly in me.polygons:
        for k in poly.edge_keys:
            edge_faces.setdefault(k, []).append(poly.index)
    top = rings - 1
    start = [p.index for p in me.polygons if any(top * around <= v < nv for v in p.vertices)
             and all(v < nv for v in p.vertices)]
    neck, todo = set(start), list(start)
    while todo:
        f = todo.pop()
        for k in me.polygons[f].edge_keys:
            if k in seam:
                continue
            for g in edge_faces[k]:
                if g not in neck:
                    neck.add(g)
                    todo.append(g)
    back = around // 2
    # Up: surface distance along each column from the level ring nearest the middle of the neck.
    neck_z = [co[v].z for f in neck for v in me.polygons[f].vertices]
    level = (min(neck_z) + max(neck_z)) / 2
    ref = min(range(rings), key=lambda i: abs(co[i * around + back].z - level))
    up = {}
    for j in range(around):
        up[ref * around + j] = 0.0
        for i in range(ref + 1, rings):
            up[i * around + j] = up[(i - 1) * around + j] + (co[i * around + j] - co[(i - 1) * around + j]).length
        for i in range(ref - 1, -1, -1):
            up[i * around + j] = up[(i + 1) * around + j] - (co[i * around + j] - co[(i + 1) * around + j]).length
    # The neck's axis: through the middle of the centre-front and centre-back columns, on the rings
    # where both are on the neck, up to the reference ring (above it the front is cut away), as a
    # straight line, since the neck leans evenly.
    neck_verts = {v for f in neck for v in me.polygons[f].vertices}
    pts = [(co[i * around].z, (co[i * around].y + co[i * around + back].y) / 2) for i in range(ref + 1)
           if i * around in neck_verts and i * around + back in neck_verts]
    n = len(pts)
    mz, my = sum(p[0] for p in pts) / n, sum(p[1] for p in pts) / n
    slope = sum((p[0] - mz) * (p[1] - my) for p in pts) / max(sum((p[0] - mz) ** 2 for p in pts), 1e-12)
    axis_y = lambda z: my + slope * (z - mz)  # noqa: E731
    ring_ref = [co[ref * around + j] for j in range(around)]
    radius = sum((ring_ref[(j + 1) % around] - ring_ref[j]).length for j in range(around)) / (2 * math.pi)
    uv = me.uv_layers.active.data
    for f in neck:
        poly = me.polygons[f]
        cols = [v % around for v in poly.vertices]
        wraps = back in cols and (back - 1) in cols
        for li in poly.loop_indices:
            v = me.loops[li].vertex_index
            p = co[v]
            # angle from the centre back (+y in Blender), going the same way round as the columns
            # (from the back towards -x)
            a = math.atan2(-p.x, p.y - axis_y(p.z)) % (2 * math.pi)
            if v % around == back:
                a = 2 * math.pi if wraps else 0.0
            elif wraps and a < math.pi / 2:
                a += 2 * math.pi
            uv[li].uv = (radius * a / TILE, up[v] / TILE)


def prepare(form, form_file, panels=False):
    """For an image fabric the cover's panels, then each face split in four (in place, the shape
    unchanged) so the narrow seam grooves are drawn from enough points to stay straight, then the
    seams themselves."""
    if panels:
        cut_panels(form, form_file)
    bm = bmesh.new()
    bm.from_mesh(form.data)
    bmesh.ops.subdivide_edges(bm, edges=bm.edges[:], cuts=1, use_grid_fill=True)
    bm.to_mesh(form.data)
    bm.free()
    for p in form.data.polygons:
        p.use_smooth = True
    seam_distances(form, form_file)


class Nodes:
    """Small helpers for building a node tree."""

    def __init__(self, tree):
        self.nodes, self.links = tree.nodes, tree.links

    def node(self, kind, **props):
        nd = self.nodes.new(kind)
        for k, v in props.items():
            setattr(nd, k, v)
        return nd

    def op(self, operation, a, b=None):
        nd = self.node("ShaderNodeMath", operation=operation)
        for sock, val in ((nd.inputs[0], a), (nd.inputs[1], b)):
            if val is None:
                continue
            if isinstance(val, (int, float)):
                sock.default_value = val
            else:
                self.links.new(val, sock)
        return nd.outputs[0]


def seam_nodes(nb, depth=0.22, ease=0.05):
    """The seams as a height in millimetres (a shallow groove `depth` deep where the stitches draw
    the cloth in, eased up `ease` either side) and how far into a groove each point is (0 to 1)."""
    op, groove, puffs = nb.op, None, None
    for attr in SEAM_GROUPS:
        d = nb.node("ShaderNodeAttribute", attribute_name=attr).outputs["Fac"]
        near = nb.node("ShaderNodeAttribute", attribute_name=attr + "_near").outputs["Fac"]
        g = op("EXPONENT", op("MULTIPLY", op("POWER", op("DIVIDE", d, 0.0011), 2.0), -1.0))
        r = op("EXPONENT", op("MULTIPLY", op("POWER", op("DIVIDE", op("SUBTRACT", op("ABSOLUTE", d), 0.0028),
                                                            0.0013), 2.0), -1.0))
        g, r = op("MULTIPLY", g, near), op("MULTIPLY", r, near)
        groove = g if groove is None else op("MAXIMUM", groove, g)
        puffs = r if puffs is None else op("MAXIMUM", puffs, r)
    return op("SUBTRACT", op("MULTIPLY", puffs, ease), op("MULTIPLY", groove, depth)), groove


def srgb_to_linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def mean_luminance(image):
    """The average (linear) brightness of an image, from a small copy of it."""
    import numpy as np

    small = image.copy()
    small.scale(256, 256)
    px = np.empty(256 * 256 * 4, dtype=np.float32)
    small.pixels.foreach_get(px)
    bpy.data.images.remove(small)
    rgb = px.reshape(-1, 4)[:, :3]
    if not image.is_float:  # stored sRGB-encoded
        rgb = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    return float((rgb @ np.array([0.2126, 0.7152, 0.0722])).mean())


def fabric(path, form_file, colour=None):
    """An image fabric from a .blend holding one material, with the form's seams pressed into it:
    its normal map goes through the seams' bump, and its colour is a shade darker in the seams.
    A cover is opaque and not metal, whatever the fabric's maps say. With `colour` (sRGB), the
    fabric keeps its own light and shade but takes that colour."""
    with bpy.data.libraries.load(path, link=False) as (src, dst):
        dst.materials = src.materials[:1]
    m = dst.materials[0]
    m.name = "Linen"
    tree = m.node_tree
    principled = next(n for n in tree.nodes if n.type == "BSDF_PRINCIPLED")
    for name, value in (("Alpha", 1.0), ("Metallic", 0.0)):
        sock = principled.inputs[name]
        for link in list(sock.links):
            tree.links.remove(link)
        sock.default_value = value
    for nd in [nd for nd in tree.nodes if nd.type == "TEX_IMAGE" and not any(o.is_linked for o in nd.outputs)]:
        tree.nodes.remove(nd)  # maps left unused (the see-through mask, metalness)
    m.surface_render_method = "DITHERED"
    if colour and principled.inputs["Base Color"].is_linked:
        source = principled.inputs["Base Color"].links[0].from_node
        tex = source if source.type == "TEX_IMAGE" else None
        nb = Nodes(tree)
        grey = nb.node("ShaderNodeRGBToBW")
        tree.links.new(principled.inputs["Base Color"].links[0].from_socket, grey.inputs[0])
        level = mean_luminance(tex.image) if tex and tex.image else 0.5
        shade = nb.op("DIVIDE", grey.outputs[0], level)
        tinted = nb.node("ShaderNodeMix", data_type="RGBA", blend_type="MULTIPLY")
        tinted.inputs["Factor"].default_value = 1.0
        tinted.inputs["A"].default_value = tuple(srgb_to_linear(c) for c in colour) + (1.0,)
        rgb = nb.node("ShaderNodeCombineColor")
        for k in range(3):
            tree.links.new(shade, rgb.inputs[k])
        tree.links.new(rgb.outputs["Color"], tinted.inputs["B"])
        tree.links.new(tinted.outputs["Result"], principled.inputs["Base Color"])
    # The fabric's height map only shades the surface, at the depth of real threads: displacing the
    # form itself would roughen its outline and open the panels apart along the seams.
    m.displacement_method = "BUMP"
    for nd in tree.nodes:
        if nd.type == "DISPLACEMENT":
            nd.inputs["Scale"].default_value = min(nd.inputs["Scale"].default_value, 0.0006)
    if not form_file:
        return m
    nb = Nodes(tree)
    bsdf = next(n for n in tree.nodes if n.type == "BSDF_PRINCIPLED")
    # A textured cloth hides a shallow seam, so here the seams are a little deeper and darker.
    height, groove = seam_nodes(nb, depth=0.45, ease=0.08)
    bump = nb.node("ShaderNodeBump")
    bump.inputs["Distance"].default_value = 0.001
    nb.links.new(height, bump.inputs["Height"])
    normal_in = bsdf.inputs["Normal"]
    if normal_in.is_linked:
        nb.links.new(normal_in.links[0].from_socket, bump.inputs["Normal"])
    nb.links.new(bump.outputs["Normal"], normal_in)
    colour_in = bsdf.inputs["Base Color"]
    darker = nb.node("ShaderNodeMix", data_type="RGBA", blend_type="MULTIPLY")
    darker.inputs["Factor"].default_value = 1.0
    if colour_in.is_linked:
        nb.links.new(colour_in.links[0].from_socket, darker.inputs["A"])
    else:
        darker.inputs["A"].default_value = colour_in.default_value
    shade = nb.op("SUBTRACT", 1.0, nb.op("MULTIPLY", groove, 0.22))
    grey = nb.node("ShaderNodeCombineColor")
    for k in range(3):
        nb.links.new(shade, grey.inputs[k])
    nb.links.new(grey.outputs["Color"], darker.inputs["B"])
    nb.links.new(darker.outputs["Result"], colour_in)
    return m


def linen(form, form_file=None, fabric_path=None, colour=None):
    """The linen cover with the seams pressed into it where the panels meet: an image fabric if
    one is given (in `colour`, if given), else a procedural plain weave."""
    if form_file:
        prepare(form, form_file, panels=bool(fabric_path))
    if fabric_path:
        return fabric(fabric_path, form_file, colour)
    m = bpy.data.materials.new("Linen")
    m.diffuse_color = LINEN
    m.use_nodes = True
    nt = m.node_tree
    nb = Nodes(nt)
    node, op, links = nb.node, nb.op, nb.links
    bsdf = nt.nodes.get("Principled BSDF")

    bsdf.inputs["Roughness"].default_value = 0.8
    for name, value in (("Sheen Weight", 0.35), ("Sheen Roughness", 0.5)):
        if name in bsdf.inputs:
            bsdf.inputs[name].default_value = value

    def mapped(vec, scale):
        mp = node("ShaderNodeMapping")
        mp.inputs["Scale"].default_value = scale
        links.new(vec, mp.inputs["Vector"])
        return mp.outputs["Vector"]

    def noise(vec, scale, detail=2.0):
        nz = node("ShaderNodeTexNoise")
        nz.inputs["Scale"].default_value = scale
        nz.inputs["Detail"].default_value = detail
        links.new(vec, nz.inputs["Vector"])
        return nz.outputs["Fac"]

    # Linen: a plain weave whose threads are each a slightly different shade and thickness, a
    # little uneven in spacing, with now and then a thicker slub, in a faintly mottled natural
    # colour with a soft cloth sheen.
    coord = node("ShaderNodeTexCoord").outputs["Object"]
    threads = []
    for axis in "XYZ":
        w = node("ShaderNodeTexWave", wave_type="BANDS", bands_direction=axis)
        w.inputs["Scale"].default_value = 450.0  # threads about 0.7 mm apart
        w.inputs["Distortion"].default_value = 1.2  # ...not quite evenly spaced
        w.inputs["Detail"].default_value = 1.0
        links.new(coord, w.inputs["Vector"])
        threads.append(w.outputs["Fac"])
    # Vertical threads are seen from whichever side faces the eye.
    normal = node("ShaderNodeSeparateXYZ")
    links.new(node("ShaderNodeNewGeometry").outputs["Normal"], normal.inputs[0])
    ax, ay = op("ABSOLUTE", normal.outputs[0]), op("ABSOLUTE", normal.outputs[1])
    side = op("DIVIDE", ax, op("ADD", op("ADD", ax, ay), 1e-4))
    vertical = node("ShaderNodeMix", data_type="FLOAT")
    links.new(side, vertical.inputs["Factor"])
    links.new(threads[0], vertical.inputs["A"])
    links.new(threads[1], vertical.inputs["B"])
    weft, warp = threads[2], vertical.outputs["Result"]
    # Noise that changes fast across the threads and slowly along them: long streaks give each
    # thread its own shade, shorter ones its thick and thin places.
    streak_h = noise(mapped(coord, (5.0, 5.0, 1300.0)), 1.0, 0.0)
    streak_v = noise(mapped(coord, (1300.0, 1300.0, 5.0)), 1.0, 0.0)
    slub_h = op("MAXIMUM", op("SUBTRACT", noise(mapped(coord, (45.0, 45.0, 1300.0)), 1.0, 1.0), 0.55), 0.0)
    slub_v = op("MAXIMUM", op("SUBTRACT", noise(mapped(coord, (1300.0, 1300.0, 45.0)), 1.0, 1.0), 0.55), 0.0)
    weave = op("ADD", op("MULTIPLY", op("ADD", weft, warp), 0.25),
               op("ADD", op("MULTIPLY", op("MULTIPLY", slub_h, weft), 2.0), op("MULTIPLY", op("MULTIPLY", slub_v, warp), 2.0)))
    slub = op("ADD", slub_h, slub_v)

    # Heights in millimetres: the weave's texture and the seams.
    height = op("MULTIPLY", weave, 0.05)
    groove = None
    if form_file:
        seams, groove = seam_nodes(nb)
        height = op("ADD", height, seams)
    bump = node("ShaderNodeBump")
    bump.inputs["Distance"].default_value = 0.001
    links.new(height, bump.inputs["Height"])
    links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])

    # Colour: the linen, each thread its own shade, mottled a little, its slubs slightly lighter,
    # a shade darker in the seams.
    threads_shade = op("ADD", op("MULTIPLY", op("ADD", op("MULTIPLY", streak_h, weft), op("MULTIPLY", streak_v, warp)),
                                 0.14), 0.9)
    shade = op("MULTIPLY", op("MULTIPLY", threads_shade, op("ADD", op("MULTIPLY", noise(coord, 18.0, 3.0), 0.06), 0.97)),
               op("ADD", op("MULTIPLY", slub, 0.25), 1.0))
    if groove is not None:
        shade = op("MULTIPLY", shade, op("SUBTRACT", 1.0, op("MULTIPLY", groove, 0.12)))
    tint = node("ShaderNodeMix", data_type="RGBA", blend_type="MULTIPLY")
    tint.inputs["Factor"].default_value = 1.0
    tint.inputs["A"].default_value = LINEN
    shade_rgb = node("ShaderNodeCombineColor")
    for k in range(3):
        links.new(shade, shade_rgb.inputs[k])
    links.new(shade_rgb.outputs["Color"], tint.inputs["B"])
    links.new(tint.outputs["Result"], bsdf.inputs["Base Color"])
    return m


def girth(form_file, station):
    """A taut tape round a station's ring (over the bust, not into the hollow), in cm."""
    ring = form_file["rings"][form_file["stations"][station]]
    n = len(ring["r"])
    pts = []
    for j, r in enumerate(ring["r"]):
        phi = math.pi * j / (n - 1)
        pts.append((r * math.sin(phi), r * math.cos(phi)))
    pts += [(-x, z) for x, z in reversed(pts[1:-1])]
    hull = convex_hull(pts)
    return sum(math.dist(hull[i], hull[(i + 1) % len(hull)]) for i in range(len(hull))) / 10


def label(form, form_file):
    """A woven label sewn on at the bottom of the front: the maker, the size and the measurements."""
    surf = Surface(form)
    kind = "WOMEN'S" if form_file["id"].startswith("women") else "MEN'S"
    size = form_file.get("base_size", "").replace("US ", "")
    chest = "bust" if "bust" in form_file["stations"] else "chest"
    lines = [("OpenDrape", 0.0125, 0.0085),
             (f"{kind} FORM  ·  SIZE {size}", 0.0062, -0.0014),
             (f"{chest.upper()} {girth(form_file, chest):.0f}  ·  WAIST {girth(form_file, 'waist'):.0f}  ·  "
              f"HIP {girth(form_file, 'hip'):.0f} cm", 0.0052, -0.0104)]
    heights = [r["y"] for r in form_file["rings"]]
    zc = heights[form_file["stations"]["bottom"]] + 0.046
    w, h = 0.095, 0.040

    def on_form(x, z, lift):
        hit = surf.bvh.ray_cast(Vector((x, -1.0, z)), Vector((0, 1, 0)), 2.0)
        return None if hit[0] is None else hit[0] + hit[1] * lift

    # The label: a grid laid on the surface.
    nu, nv = 38, 16
    bm = bmesh.new()
    grid = [[bm.verts.new(on_form(-w / 2 + w * i / nu, zc - h / 2 + h * j / nv, 0.0004)) for j in range(nv + 1)]
            for i in range(nu + 1)]
    for i in range(nu):
        for j in range(nv):
            bm.faces.new((grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1])).smooth = True
    mesh = bpy.data.meshes.new("Label")
    bm.to_mesh(mesh)
    bm.free()
    tag = bpy.data.objects.new("Label", mesh)
    bpy.context.scene.collection.objects.link(tag)
    tag.data.materials.append(material("Label", (0.025, 0.035, 0.085, 1.0), 0.0, 0.6))
    out = [tag]
    # The lettering, woven in: flat text laid on the label.
    thread = material("Lettering", (0.86, 0.82, 0.72, 1.0), 0.0, 0.55)
    for text, size_m, dz in lines:
        curve = bpy.data.curves.new("Text", "FONT")
        curve.body = text
        curve.size = size_m
        curve.align_x, curve.align_y = "CENTER", "CENTER"
        tmp = bpy.data.objects.new("Text", curve)
        bpy.context.scene.collection.objects.link(tmp)
        tmp.rotation_euler = (math.radians(90), 0, 0)
        tmp.location = (0, 0, zc + dz)
        bpy.context.view_layer.update()
        flat = bpy.data.meshes.new_from_object(tmp.evaluated_get(bpy.context.evaluated_depsgraph_get()))
        flat.transform(tmp.matrix_world)
        for v in flat.vertices:
            p = on_form(v.co.x, v.co.z, 0.0007)
            if p is not None:
                v.co = p
        bpy.data.objects.remove(tmp)
        obj = bpy.data.objects.new("Lettering", flat)
        bpy.context.scene.collection.objects.link(obj)
        obj.data.materials.append(thread)
        out.append(obj)
    return out


def plate_screws(form, form_file):
    """The screw at the centre of each armhole plate, as on a real form."""
    if "plate_centre" not in form_file["landmarks"]:
        return []
    surf = Surface(form)
    heights = [r["y"] for r in form_file["rings"]]
    phi, v = form_file["landmarks"]["plate_centre"]
    f = v * (len(heights) - 1)
    i = min(int(f), len(heights) - 2)
    z = heights[i] + (heights[i + 1] - heights[i]) * (f - i)
    dark = material("Screw", (0.25, 0.25, 0.26, 1.0), 1.0, 0.35)
    out = []
    for a in (phi, 2 * math.pi - phi):
        o = Vector((0, surf.centre_y(z), z))
        hit = surf.bvh.ray_cast(o, Vector((math.sin(a), -math.cos(a), 0)), 2.0)
        if hit[0] is None:
            continue
        # A flush disc lying on the surface.
        rotation = Vector((0, 0, 1)).rotation_difference(hit[1]).to_euler()
        bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.0042, depth=0.0012,
                                            location=hit[0] + hit[1] * 0.0004, rotation=rotation)
        screw = bpy.context.active_object
        screw.name = "Screw"
        screw.data.materials.append(dark)
        out.append(screw)
    return out


def cylinder(name, centre, radius, depth, mat):
    bpy.ops.mesh.primitive_cylinder_add(vertices=48, radius=radius, depth=depth, location=centre)
    obj = bpy.context.active_object
    obj.name = name
    obj.data.materials.append(mat)
    for p in obj.data.polygons:
        p.use_smooth = True
    return obj


def neck_cap(form, metal, ax, ay):
    """A metal cap on the slanted neck cut: a plate on the cut, its rim rounded over into a short
    collar that hugs the neck, as on a pressed metal cap."""
    cut_y, cut_deg = form["neck_cut"]
    tan = math.tan(math.radians(cut_deg))
    # The cut in Blender coordinates: height z = cut_y + tan * (y - ay); the front (-y) is lower.
    plane = lambda y: cut_y + tan * (y - ay)
    verts = [form.matrix_world @ v.co for v in form.data.vertices]
    face = [(v.x, v.y) for v in verts if abs(v.z - plane(v.y)) < 0.0004]
    # The outline of the cut seen from above, as a distance from its centre in 96 directions.
    hull = convex_hull(face)
    cx, cy = sum(p[0] for p in hull) / len(hull), sum(p[1] for p in hull) / len(hull)
    dirs, radii = [], []
    for k in range(96):
        a = 2 * math.pi * k / 96
        d = (math.cos(a), math.sin(a))
        dirs.append(d)
        radii.append(max(t for t in (ray_segment((cx, cy), d, hull[i], hull[(i + 1) % len(hull)])
                                     for i in range(len(hull))) if t is not None) + 0.0025)
    # The profile of the rim, from the middle of the plate out and down: (in from the edge, height
    # above the cut), metres. The top's edge rounds over with a 6 mm radius, the collar's foot 2 mm.
    # On the flat top the rings sit at fractions of the way out to where the rim starts.
    top, foot, rim, lip = 0.004, -0.022, 0.006, 0.002
    flat = [(f, top) for f in (0.35, 0.65, 0.88)]
    profile = [(rim - rim * math.sin(t), top - rim + rim * math.cos(t))
               for t in (math.radians(10 * i) for i in range(10))]
    profile += [(0.0, foot + lip + (top - rim - foot - lip) * f) for f in (0.66, 0.33, 0.0)]
    profile += [(lip - lip * math.cos(t), foot + lip - lip * math.sin(t))
                for t in (math.radians(15 * i) for i in range(1, 7))]
    profile += [(0.004, foot)]
    bm = bmesh.new()
    rings = []
    centre = bm.verts.new((cx, cy, plane(cy) + top))
    for k, (amount, h) in enumerate(flat + profile):
        ring = []
        for (dx, dy), r in zip(dirs, radii):
            rr = (r - rim) * amount if k < len(flat) else r - amount
            x, y = cx + dx * rr, cy + dy * rr
            ring.append(bm.verts.new((x, y, plane(y) + h)))
        rings.append(ring)
    n = len(dirs)
    for i in range(n):
        bm.faces.new((centre, rings[0][i], rings[0][(i + 1) % n])).smooth = True
    for r0, r1 in zip(rings, rings[1:]):
        for i in range(n):
            j = (i + 1) % n
            bm.faces.new((r0[i], r1[i], r1[j], r0[j])).smooth = True
    bm.faces.new(list(reversed(rings[-1])))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new("Cap")
    bm.to_mesh(mesh)
    bm.free()
    cap = bpy.data.objects.new("Cap", mesh)
    bpy.context.scene.collection.objects.link(cap)
    cap.data.materials.append(metal)
    return cap, plane(ay) + top


def convex_hull(points):
    pts = sorted(set(points))
    cross = lambda o, a, b: (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return lower[:-1] + upper[:-1]


def ray_segment(o, d, p, q):
    """Distance along the ray o + t d to the segment p-q, or None."""
    ex, ey = q[0] - p[0], q[1] - p[1]
    den = d[0] * ey - d[1] * ex
    if abs(den) < 1e-12:
        return None
    t = ((p[0] - o[0]) * ey - (p[1] - o[1]) * ex) / den
    s = ((p[0] - o[0]) * d[1] - (p[1] - o[1]) * d[0]) / den
    return t if t >= 0 and -1e-9 <= s <= 1 + 1e-9 else None


def stand(form):
    """The neck cap, a knob on a short rod and the pole, all on the form's pole axis."""
    metal, black = material("Metal", METAL, 1.0, 0.3), material("Black", BLACK, 0.0, 0.4)
    px, pz = form.get("pole_xz", (0.0, 0.0))
    ax, ay = px, -pz
    bottom = min((form.matrix_world @ v.co).z for v in form.data.vertices)
    parts = []
    if "neck_cut" in form:
        cap, cap_top = neck_cap(form, metal, ax, ay)
        parts.append(cap)
    else:
        cap_top = max((form.matrix_world @ v.co).z for v in form.data.vertices)
    parts.append(cylinder("Rod", (ax, ay, cap_top + 0.012), 0.006, 0.024, metal))
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.016, location=(ax, ay, cap_top + 0.036))
    knob = bpy.context.active_object
    knob.name = "Knob"
    knob.scale = (1.0, 1.0, 0.8)
    knob.data.materials.append(black)
    for p in knob.data.polygons:
        p.use_smooth = True
    parts.append(knob)
    parts.append(cylinder("Collar", (ax, ay, bottom + 0.002), 0.03, 0.012, metal))
    parts.append(cylinder("Pole", (ax, ay, bottom / 2), 0.013, bottom, black))
    parts.append(base_plate(ax, ay))
    return parts


def base_plate(ax, ay):
    """A heavy round base on the floor where the pole ends: a low dome with a rounded rim and a
    boss round the pole."""
    # Profile (radius, height) in metres, from the pole out to the rim and under.
    profile = [(0.014, 0.046), (0.024, 0.046), (0.030, 0.042), (0.032, 0.034), (0.036, 0.028), (0.05, 0.026)]
    profile += [(0.05 + 0.105 * f, 0.026 - 0.004 * f * f) for f in (0.25, 0.5, 0.75, 1.0)]
    rim = 0.009
    profile += [(0.155 + rim * math.sin(t), 0.022 - rim + rim * math.cos(t))
                for t in (math.radians(15 * i) for i in range(1, 7))]
    profile += [(0.164, 0.004), (0.161, 0.0), (0.0, 0.0)]
    bm = bmesh.new()
    n = 96
    rings = [[bm.verts.new((ax + r * math.cos(2 * math.pi * k / n), ay + r * math.sin(2 * math.pi * k / n), h))
              for k in range(n)] if r > 0 else None for r, h in profile]
    bottom = bm.verts.new((ax, ay, 0.0))
    body = [r for r in rings if r is not None]
    for r0, r1 in zip(body, body[1:]):
        for k in range(n):
            j = (k + 1) % n
            bm.faces.new((r0[k], r1[k], r1[j], r0[j])).smooth = True
    for k in range(n):
        bm.faces.new((body[-1][k], bottom, body[-1][(k + 1) % n]))
    bm.faces.new(body[0])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    mesh = bpy.data.meshes.new("Base")
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new("Base", mesh)
    bpy.context.scene.collection.objects.link(obj)
    obj.data.materials.append(material("Base", (0.03, 0.03, 0.035, 1.0), 0.6, 0.35))
    return obj


def backdrop():
    m = material("Backdrop", (1, 1, 1, 1))
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Emission Color"].default_value = (1, 1, 1, 1)
    bsdf.inputs["Emission Strength"].default_value = 2.5
    bpy.ops.mesh.primitive_plane_add(size=30, location=(1.2, 6.0, 1.0), rotation=(math.radians(90), 0, 0))
    bpy.context.active_object.data.materials.append(m)


def floor():
    m = material("Floor", (0.9, 0.9, 0.89, 1.0), 0.0, 0.9)
    bpy.ops.mesh.primitive_plane_add(size=30, location=(1.2, 0.0, 0.0))
    bpy.context.active_object.data.materials.append(m)


def main(out_path, form_path, closeup, fabric_path=None, colour=None):
    scene = bpy.context.scene
    form = scene.objects["Form"]
    form_file = json.load(open(form_path)) if form_path else None
    form.data.materials.append(linen(form, form_file, fabric_path, colour))
    extras = stand(form)
    if form_file:
        extras += plate_screws(form, form_file) + label(form, form_file)
    backdrop()
    floor()
    views = [0.0, -90.0] if closeup else [0.0, -90.0, 180.0, -40.0]
    for k, deg in enumerate(views):
        pivot = bpy.data.objects.new(f"View{k}", None)
        scene.collection.objects.link(pivot)
        pivot.location = (k * SPACING, 0, 0)
        pivot.rotation_euler = (0, 0, math.radians(deg))
        for p in [form] + extras:
            copy = p if k == 0 else p.copy()
            if k:
                scene.collection.objects.link(copy)
            copy.parent = pivot
    zs = [(form.matrix_world @ v.co).z for v in form.data.vertices]
    zlo, zhi = -0.02, max(zs) + 0.06
    if closeup:
        zlo = zhi - 0.50
    width, height = (1600, 1000) if closeup else (2000, 1300)
    cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
    scene.collection.objects.link(cam)
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = max(SPACING * len(views), (zhi - zlo) * 1.12 * width / height)
    cam.location = (SPACING * (len(views) - 1) / 2, -6.0, (zlo + zhi) / 2)
    cam.rotation_euler = (math.radians(90), 0, 0)
    scene.camera = cam
    for name, loc, energy, size in [("Key", (-3.6, -2.4, 2.2), 480, 1.6), ("Fill", (4.2, -2.8, 1.0), 45, 6.0),
                                    ("Rim", (1.0, 3.5, 3.0), 120, 3.0)]:
        light = bpy.data.objects.new(name, bpy.data.lights.new(name, "AREA"))
        light.data.energy, light.data.size = energy, size
        light.location = loc
        light.rotation_euler = (Vector((SPACING * 1.5, 0, 1.2)) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
        scene.collection.objects.link(light)
    scene.world = scene.world or bpy.data.worlds.new("World")
    if scene.world.node_tree is None:
        scene.world.use_nodes = True
    bg = scene.world.node_tree.nodes.get("Background")
    bg.inputs["Color"].default_value = (0.97, 0.97, 0.96, 1.0)
    bg.inputs["Strength"].default_value = 0.35
    scene.render.engine = "BLENDER_EEVEE"
    scene.eevee.taa_render_samples = 32
    for flag in ("use_gtao", "use_raytracing"):
        if hasattr(scene.eevee, flag):
            setattr(scene.eevee, flag, True)
    scene.render.resolution_x, scene.render.resolution_y = width, height
    scene.render.filepath = out_path
    bpy.ops.render.render(write_still=True)


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :]
    fabric_path = args[args.index("--fabric") + 1] if "--fabric" in args else None
    tile = args[args.index("--tile") + 1] if "--tile" in args else None
    if tile:
        TILE = float(tile)
    paths = [a for a in args if not a.startswith("--") and a not in (fabric_path, tile)]
    main(paths[0], paths[1] if len(paths) > 1 else None, "--closeup" in args, fabric_path,
         BEIGE if "--beige" in args else None)
