"""
Mesh primitives for the character builder (`build_character.py`): boxes, prisms, blobs, lofted tubes
and joint balls, each bound rigidly to one bone and painted with vertex colours.

Every part is bound whole to one bone, so the client can cut the mesh into limbs at the joints
(`client/src/render/gibs.ts`). Parts that meet at a joint overlap a little, and a joint ball fills
the gap a bent joint opens.

Axes are Blender's as the builder uses them: x is the character's left, -y forward, z up, metres.
"""

import math
import random

import bmesh
from mathutils import Matrix, Vector

def srgb(h):
    """Hex colour to linear rgba."""
    out = []
    for shift in (16, 8, 0):
        c = ((h >> shift) & 0xFF) / 255.0
        out.append(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4)
    return (*out, 1.0)


def grey(v):
    return (v, v, v, 1.0)


# The materials, by index into each mesh's material slots: `outfit` is tinted per character (team
# patches, a civilian's top), `gear` carries its final colours, and `skin` is tinted with a skin tone.
MAT_OUTFIT, MAT_GEAR, MAT_SKIN = 0, 1, 2


# Edges between faces turned further than this apart are drawn sharp, radians: the caps of a tube and
# the corners of a box. Below it the faces shade smoothly into each other.
SHARP = math.radians(60)


def sharpen(mesh):
    """Marks `mesh`'s edges sharp where its faces turn more than `SHARP`, so the export splits
    vertices only there."""
    mesh.set_sharp_from_angle(angle=SHARP)


class Builder:
    """One mesh under construction. `groups` maps bone names to vertex group indices and is shared by
    every mesh, so each mesh's groups line up with the armature's bones."""

    def __init__(self, groups):
        self.bm = bmesh.new()
        self.deform = self.bm.verts.layers.deform.verify()
        self.colors = self.bm.loops.layers.float_color.new("Color")
        self.groups = groups

    def weld(self, dist=0.0015):
        """Merges vertices closer than `dist` metres that are bound to the same bone, so parts that meet
        share them. Vertices on different bones stay apart: each part moves rigidly with its own bone."""
        by_bone = {}
        for v in self.bm.verts:
            groups = list(v[self.deform].keys())
            by_bone.setdefault(groups[0] if groups else -1, []).append(v)
        for verts in by_bone.values():
            bmesh.ops.remove_doubles(self.bm, verts=verts, dist=dist)

    def group(self, name):
        if name not in self.groups:
            self.groups[name] = len(self.groups)
        return self.groups[name]

    def finish(self, verts, bone, material, color, transform=None):
        """Binds freshly made `verts` rigidly to `bone` and paints their faces. `color` is an rgba, or a
        function of a face (its centre and normal, in the rest pose) returning one."""
        gi = self.group(bone)
        faces = set()
        for v in verts:
            if transform is not None:
                v.co = transform @ v.co
            v[self.deform][gi] = 1.0
            faces.update(v.link_faces)
        # Faces are smooth: the mesh's edges are made sharp past `SHARP` (`sharpen`), so a tube shades
        # round and shares its vertices while a box keeps its corners.
        by_vertex = {}
        per_vertex = getattr(color, "per_vertex", False)
        for f in sorted(faces, key=lambda f: f.index):
            f.normal_update()
            f.material_index = material
            f.smooth = True
            c = color(f) if callable(color) and not per_vertex else color
            for loop in f.loops:
                if per_vertex:
                    if loop.vert not in by_vertex:
                        by_vertex[loop.vert] = color(loop.vert)
                    loop[self.colors] = by_vertex[loop.vert]
                else:
                    loop[self.colors] = c
        return verts

    def box(self, center, size, bone, material, color, taper=None, transform=None):
        """Axis-aligned box. `taper` = (sx, sy) scales the top face's cross-section."""
        verts = bmesh.ops.create_cube(self.bm, size=1.0)["verts"]
        for v in verts:
            top = v.co.z > 0
            x, y, z = v.co.x * size[0], v.co.y * size[1], v.co.z * size[2]
            if taper and top:
                x *= taper[0]
                y *= taper[1]
            v.co = Vector((x, y, z)) + Vector(center)
        return self.finish(verts, bone, material, color, transform)

    def prism(self, p0, p1, r0, r1, bone, material, color, sides=6, transform=None):
        """Tapered prism from `p0` to `p1`."""
        p0, p1 = Vector(p0), Vector(p1)
        axis = p1 - p0
        rot = Vector((0, 0, 1)).rotation_difference(axis.normalized()).to_matrix().to_4x4()
        m = Matrix.Translation((p0 + p1) / 2) @ rot
        res = bmesh.ops.create_cone(
            self.bm, cap_ends=True, cap_tris=False, segments=sides, radius1=r0, radius2=r1, depth=axis.length, matrix=m
        )
        return self.finish(res["verts"], bone, material, color, transform)

    def blob(self, center, radii, bone, material, color, subdivisions=1, transform=None):
        """An icosphere stretched to `radii`."""
        res = bmesh.ops.create_icosphere(self.bm, subdivisions=subdivisions, radius=1.0)
        for v in res["verts"]:
            v.co = Vector((v.co.x * radii[0], v.co.y * radii[1], v.co.z * radii[2])) + Vector(center)
        return self.finish(res["verts"], bone, material, color, transform)

    def ball(self, center, r, bone, material, color):
        """A joint ball: a low-poly sphere of radius `r` that fills the gap a bent joint opens."""
        return self.blob(center, (r, r, r), bone, material, color)

    def loft(self, rings, bone, material, color, sides=8, axis="z", caps=(True, True)):
        """A closed tube through cross-sections. Each ring is `(a, b, c, ra, rb)`: along `axis` "z" the
        ring lies in the xy plane at height `c`, centred on (a, b), with half-widths `ra` along x and
        `rb` along y; along "y" it lies in the xz plane at `b`, centred on (a, c), half-widths along x
        and z; along "x" it lies in the yz plane at `a`, centred on (b, c), half-widths along y and z. A ring may carry a sixth value, a squareness 0..1 that pushes the section from an
        ellipse towards a rectangle, and a seventh, a slope: along "z" the ring tips so each point is
        `slope` metres higher for each metre it lies behind the centre (negative raises the front, as a
        hairline or a helmet's brow). The faces are flat, as the rest of the model is."""
        verts, loops = [], []
        for ring in rings:
            a, b, c, ra, rb = ring[:5]
            square = ring[5] if len(ring) > 5 else 0.0
            slope = ring[6] if len(ring) > 6 else 0.0
            loop = []
            for i in range(sides):
                angle = (i + 0.5) / sides * 2 * math.pi
                u, w = math.cos(angle), math.sin(angle)
                if square:
                    # Towards the rectangle: scale the direction so its larger component reaches 1.
                    k = 1.0 / max(abs(u), abs(w))
                    s = 1.0 + (k - 1.0) * square
                    u, w = u * s, w * s
                if axis == "z":
                    co = (a + ra * u, b + rb * w, c + slope * rb * w)
                elif axis == "x":
                    co = (a, b + ra * u, c + rb * w)
                else:
                    co = (a + ra * u, b, c + rb * w)
                v = self.bm.verts.new(co)
                loop.append(v)
                verts.append(v)
            loops.append(loop)
        faces = []
        for lo, hi in zip(loops, loops[1:]):
            for i in range(sides):
                j = (i + 1) % sides
                faces.append(self.bm.faces.new((lo[i], lo[j], hi[j], hi[i])))
        if caps[0]:
            faces.append(self.bm.faces.new(loops[0][::-1]))
        if caps[1]:
            faces.append(self.bm.faces.new(loops[-1]))
        # Whichever way the rings run, the faces are turned to face out of the tube.
        bmesh.ops.recalc_face_normals(self.bm, faces=faces)
        return self.finish(verts, bone, material, color)


def bands(*steps):
    """A colour by height: `bands((z0, c0), (z1, c1), ...)` paints a face `c_i` when its centre is at
    or above `z_i` (the steps in rising order, the first one the colour below them all)."""

    def paint(face):
        z = face.calc_center_median().z
        out = steps[0][1]
        for at, c in steps:
            if z >= at:
                out = c
        return out

    return paint


def soles(upper, sole, below=-0.6):
    """Paints faces facing down (`normal.z` under `below`) `sole`, the rest `upper`."""
    return lambda face: sole if face.normal.z < below else upper


def chequered(a, b, z0, height, sides):
    """A chequer of `a` and `b` round a ring: rows `height` metres tall from `z0`, a square a side."""

    def paint(face):
        c = face.calc_center_median()
        column = int((math.atan2(c.y, c.x) + math.pi) / (2 * math.pi) * sides + 0.5)
        row = int((c.z - z0) / height)
        return a if (row + column) % 2 == 0 else b

    return paint


def mottled(shades, seed):
    """A seeded random pick of `shades` per vertex, blended across the faces: a low-poly camouflage.
    Per vertex, not per face, so neighbouring faces still share their vertices."""
    rng = random.Random(seed)

    def paint(vert):
        return shades[rng.randrange(len(shades))]

    paint.per_vertex = True
    return paint
