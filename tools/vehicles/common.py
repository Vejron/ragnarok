"""
What the vehicle builders share. Each is run by Blender with its own directory on `sys.path`.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []

# The materials of the model being built, in the order parts name them by index. Each builder fills it.
MATERIALS = []


def arg(name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


def material(name, rgb):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    bsdf.inputs["Roughness"].default_value = 1.0
    m.diffuse_color = (*rgb, 1.0)
    return m


def about_x(angle):
    return Matrix.Rotation(angle, 4, "X")


def about_y(angle):
    return Matrix.Rotation(angle, 4, "Y")


def about_z(angle):
    return Matrix.Rotation(angle, 4, "Z")


def stadium(front, rear, z, radius, steps=8):
    """The (y, z) outline of a track: two arcs joined by straights, going over the top first."""
    loop = []
    for k in range(steps + 1):
        a = math.pi / 2 - math.pi * k / steps
        loop.append((front + radius * math.cos(a), z + radius * math.sin(a)))
    for k in range(steps + 1):
        a = -math.pi / 2 - math.pi * k / steps
        loop.append((rear + radius * math.cos(a), z + radius * math.sin(a)))
    # Drop the duplicated join points at the arcs' ends.
    return [p for i, p in enumerate(loop) if i == 0 or p != loop[i - 1]]


def export(out):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    kwargs = dict(
        filepath=out,
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
        export_animations=False,
        export_skins=False,
        export_cameras=False,
        export_lights=False,
        export_extras=False,
        export_materials="EXPORT",
    )
    valid = bpy.ops.export_scene.gltf.get_rna_type().properties.keys()
    bpy.ops.export_scene.gltf(**{k: v for k, v in kwargs.items() if k in valid})


class Part:
    """A mesh under construction: primitives are added in the part's own frame, each with a material."""

    def __init__(self, name, location=(0.0, 0.0, 0.0), parent=None):
        self.bm = bmesh.new()
        self.name = name
        self.location = Vector(location)
        self.parent = parent

    def _tag(self, verts, mat):
        for face in {f for v in verts for f in v.link_faces}:
            face.material_index = mat

    def box(self, centre, size, mat, rx=0.0):
        m = Matrix.Translation(centre) @ Matrix.Rotation(rx, 4, "X") @ Matrix.Diagonal((*size, 1.0))
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

    def cylinder(self, centre, radius, depth, mat, axis="Z", segments=10, radius2=None, squash=(1.0, 1.0)):
        """A cylinder or frustum. `squash` stretches the two cross-section axes (a slot or an ellipse)."""
        turn = {"Z": Matrix.Identity(4), "X": Matrix.Rotation(math.pi / 2, 4, "Y"), "Y": Matrix.Rotation(-math.pi / 2, 4, "X")}[axis]
        flat = Matrix.Diagonal((squash[0], squash[1], 1.0, 1.0))
        m = Matrix.Translation(centre) @ turn @ flat
        verts = bmesh.ops.create_cone(
            self.bm,
            cap_ends=True,
            cap_tris=False,
            segments=segments,
            radius1=radius,
            radius2=radius if radius2 is None else radius2,
            depth=depth,
            matrix=m,
        )["verts"]
        self._tag(verts, mat)

    def prism(self, profile, x0, x1, mat):
        """Extrudes a closed (y, z) outline along X, from x0 to x1."""
        a = [self.bm.verts.new((x0, y, z)) for y, z in profile]
        b = [self.bm.verts.new((x1, y, z)) for y, z in profile]
        faces = [self.bm.faces.new(a), self.bm.faces.new(b[::-1])]
        for i in range(len(profile)):
            j = (i + 1) % len(profile)
            faces.append(self.bm.faces.new((a[i], a[j], b[j], b[i])))
        for face in faces:
            face.material_index = mat

    def build(self):
        bmesh.ops.remove_doubles(self.bm, verts=self.bm.verts, dist=1e-4)
        bmesh.ops.recalc_face_normals(self.bm, faces=self.bm.faces)
        mesh = bpy.data.meshes.new(self.name)
        self.bm.to_mesh(mesh)
        self.bm.free()
        for m in MATERIALS:
            mesh.materials.append(m)
        obj = bpy.data.objects.new(self.name, mesh)
        obj.location = self.location
        bpy.context.scene.collection.objects.link(obj)
        if self.parent:
            obj.parent = self.parent
        return obj

    def dome(self, centre, scale, mat, floor, u=14, v=8):
        """The part of an ellipsoid above the horizontal plane `floor` (relative to its centre)."""
        m = Matrix.Translation(centre) @ Matrix.Diagonal((*scale, 1.0))
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=u, v_segments=v, radius=1.0, matrix=m)["verts"]
        below = [vert for vert in verts if vert.co.z < centre[2] + floor * scale[2] - 1e-4]
        bmesh.ops.delete(self.bm, geom=below, context="VERTS")
        self._tag([vert for vert in verts if vert.is_valid], mat)

    def loft(self, stations, mat, segments=12, power=2.0):
        """A body along Y through elliptical cross-sections `(y, centre_x, centre_z, radius_x, radius_z)`.
        `power` squares them off: 2 is an ellipse, higher is boxier."""
        rings = []
        for y, cx, cz, rx, rz in stations:
            ring = []
            for k in range(segments):
                a = 2 * math.pi * k / segments
                c, s = math.cos(a), math.sin(a)
                x = rx * math.copysign(abs(c) ** (2 / power), c)
                z = rz * math.copysign(abs(s) ** (2 / power), s)
                ring.append(self.bm.verts.new((cx + x, y, cz + z)))
            rings.append(ring)
        faces = [self.bm.faces.new(rings[0]), self.bm.faces.new(rings[-1][::-1])]
        for a, b in zip(rings, rings[1:]):
            for i in range(segments):
                j = (i + 1) % segments
                faces.append(self.bm.faces.new((a[i], a[j], b[j], b[i])))
        for face in faces:
            face.material_index = mat

    def ring(self, outer, inner, x0, x1, mat):
        """Extrudes the band between two (y, z) loops along X."""
        n = len(outer)
        verts = {
            (name, x, i): self.bm.verts.new((x, *loop[i]))
            for name, loop in (("o", outer), ("i", inner))
            for x in (x0, x1)
            for i in range(n)
        }
        for i in range(n):
            j = (i + 1) % n
            for x in (x0, x1):
                self.bm.faces.new((verts["o", x, i], verts["o", x, j], verts["i", x, j], verts["i", x, i])).material_index = mat
            self.bm.faces.new((verts["o", x0, i], verts["o", x0, j], verts["o", x1, j], verts["o", x1, i])).material_index = mat
            self.bm.faces.new((verts["i", x0, i], verts["i", x0, j], verts["i", x1, j], verts["i", x1, i])).material_index = mat
