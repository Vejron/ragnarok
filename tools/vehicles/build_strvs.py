"""
Builds the Strv S, a Swedish turretless tank (the "S-tank"), and exports it to client/public/models/strvs.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_strvs.py -- \
        [--out client/public/models/strvs.glb] [--preview DIR]

Everything is procedural and flat-shaded, well under three thousand triangles in all.

Conventions
  * Blender: Z up, the gun points along +Y, the tank's right hand side is +X. The glTF exporter turns
    that into Y up, gun along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider (a 2.6 x 0.9 x 5.4 m box). The ground is 0.91 m below it.
  * There is no turret: the gun is fixed in a casemate on the hull, and the whole tank turns to aim.
  * Nodes the game looks up by name:
      `hull`            everything fixed to the hull, tracks and casemate included
      `wheel_L_<n>`     road wheels, idler and sprocket of the left track, origin on the axle: the game rolls them about X
      `wheel_R_<n>`     the same on the right
      `gun`             origin on the trunnion (the game's STRV_GUN_PIVOT); the game pitches it and slides it back in recoil
  * Materials: `paint` is tinted per tank at runtime; `dark` (tracks, tyres), `metal` (barrel, fittings)
    and `steel` (rims) keep their colours.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def arg(name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/strvs.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with STRV_* in crates/shared/src/consts.rs.
# --------------------------------------------------------------------------------------------------
GUN_PIVOT = Vector((0.0, 1.0, 0.75))  # game [0, 0.75, -1.0]: +Z in the game is backwards
BARREL_LENGTH = 3.4
TRACK_X = 1.25  # centre line of each track
TRACK_WIDTH = 0.46
TRACK_Z = -0.47  # height of the wheel axles
WHEEL_RADIUS = 0.37
ROAD_WHEELS = (-1.9, -0.95, 0.0, 0.95, 1.9)
SPROCKET_Y, IDLER_Y = -2.5, 2.5

# --------------------------------------------------------------------------------------------------
# Scene and materials
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


def material(name, rgb):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    bsdf.inputs["Roughness"].default_value = 1.0
    m.diffuse_color = (*rgb, 1.0)
    return m


MATERIALS = [
    material("paint", (0.62, 0.66, 0.5)),
    material("dark", (0.1, 0.1, 0.1)),
    material("metal", (0.24, 0.26, 0.23)),
    material("steel", (0.55, 0.56, 0.55)),
]
PAINT, DARK, METAL, STEEL = range(4)


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

    def dome(self, centre, scale, mat, floor, u=14, v=8):
        """The part of an ellipsoid above the horizontal plane `floor` (relative to its centre)."""
        m = Matrix.Translation(centre) @ Matrix.Diagonal((*scale, 1.0))
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=u, v_segments=v, radius=1.0, matrix=m)["verts"]
        below = [vert for vert in verts if vert.co.z < centre[2] + floor * scale[2] - 1e-4]
        bmesh.ops.delete(self.bm, geom=below, context="VERTS")
        self._tag([vert for vert in verts if vert.is_valid], mat)

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
        scene.collection.objects.link(obj)
        if self.parent:
            obj.parent = self.parent
        return obj


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


# --------------------------------------------------------------------------------------------------
# Hull, casemate and tracks
# --------------------------------------------------------------------------------------------------
hull = Part("hull")
HALF_W = TRACK_X - TRACK_WIDTH / 2  # the hull's side plates meet the inner faces of the tracks
# Side profile, (y, z): a flat engine deck, a long low glacis and a short steep tail.
hull.prism(
    [(-2.7, -0.45), (-2.7, 0.3), (-2.5, 0.45), (1.2, 0.45), (2.7, -0.05), (2.7, -0.3), (2.45, -0.45)],
    -HALF_W,
    HALF_W,
    PAINT,
)
# The casemate the gun sits in: a box on the roof with a steeply sloped front (the collider is a box
# [-1.45, 1.15] long, 0.45..1.05 high).
hull.prism(
    [(-1.45, 0.45), (-1.45, 0.95), (-1.2, 1.05), (0.35, 1.05), (1.15, 0.7), (1.15, 0.45)],
    -0.85,
    0.85,
    PAINT,
)
for side in (-1, 1):
    x = side * TRACK_X
    outer = stadium(SPROCKET_Y * -1, SPROCKET_Y, TRACK_Z, WHEEL_RADIUS + 0.1)
    inner = stadium(SPROCKET_Y * -1, SPROCKET_Y, TRACK_Z, WHEEL_RADIUS + 0.02)
    hull.ring(outer, inner, x - TRACK_WIDTH / 2, x + TRACK_WIDTH / 2, DARK)
    # Mudguard over the track, a stowage bin at the back and a headlamp at the front.
    hull.box((x, 0.0, 0.05), (TRACK_WIDTH + 0.14, 5.6, 0.06), PAINT)
    hull.box((x, 2.75, 0.0), (TRACK_WIDTH + 0.14, 0.5, 0.05), PAINT, rx=-0.45)
    hull.box((x, -1.6, 0.2), (0.4, 1.0, 0.3), PAINT)
    hull.box((x, 2.3, 0.2), (0.2, 0.2, 0.16), METAL)
# Driver's hatch on the casemate roof at the front right, the engine deck grille and the exhaust at the tail.
hull.box((0.35, 0.3, 1.07), (0.5, 0.5, 0.08), PAINT)
hull.box((0.35, 0.3, 1.12), (0.32, 0.32, 0.05), METAL)
hull.box((0.0, -2.0, 0.47), (1.1, 1.2, 0.05), DARK)
hull.cylinder((0.6, -2.75, 0.1), 0.07, 0.3, DARK, axis="Y", segments=6)
# Two dozer-blade stubs under the nose, as the real one has.
hull.box((0.0, 2.72, -0.38), (2.0, 0.12, 0.16), METAL)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: a tyre, a lighter rim and a bar across it, so that rolling can be seen.
# --------------------------------------------------------------------------------------------------
wheel_objects = []
for side, label in ((-1, "L"), (1, "R")):
    stations = [(f"{n}", y, WHEEL_RADIUS, 0.3) for n, y in enumerate(ROAD_WHEELS)]
    stations.append(("5", SPROCKET_Y, 0.34, 0.34))
    stations.append(("6", IDLER_Y, 0.36, 0.3))
    for index, y, radius, width in stations:
        wheel = Part(f"wheel_{label}_{index}", location=(side * TRACK_X, y, TRACK_Z))
        wheel.cylinder((0, 0, 0), radius, width, DARK, axis="X", segments=10)
        wheel.cylinder((0, 0, 0), radius * 0.62, width + 0.06, STEEL, axis="X", segments=8)
        wheel.box((0, 0, 0), (width + 0.09, radius * 1.1, 0.08), METAL)
        wheel_objects.append(wheel.build())

# --------------------------------------------------------------------------------------------------
# The gun: a mantlet in the casemate's front and a long barrel with a bore evacuator, origin on the trunnion.
# --------------------------------------------------------------------------------------------------
gun = Part("gun", location=GUN_PIVOT)
gun.box((0.0, 0.2, 0.0), (0.7, 0.5, 0.45), PAINT)  # mantlet
gun.cylinder((0.0, (0.45 + BARREL_LENGTH) / 2, 0.0), 0.12, BARREL_LENGTH - 0.45, METAL, axis="Y", segments=10, radius2=0.085)
gun.cylinder((0.0, 2.2, 0.0), 0.16, 0.55, METAL, axis="Y", segments=8)  # bore evacuator
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
def export():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    kwargs = dict(
        filepath=OUT,
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


export()
everything = [hull_obj, gun_obj, *wheel_objects]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)


# --------------------------------------------------------------------------------------------------
# Preview renders (optional)
# --------------------------------------------------------------------------------------------------
def preview(directory):
    os.makedirs(directory, exist_ok=True)
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "MATERIAL"
    scene.render.resolution_x, scene.render.resolution_y = 900, 520
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "side": ("ORTHO", Vector((12, 0, -0.2)), Vector((0, 0, -0.2)), 7.5),
        "top": ("ORTHO", Vector((0, 0, 12)), Vector((0, 0, 0)), 7.5),
        "front": ("ORTHO", Vector((0, -12, 0)), Vector((0, 0, 0)), 4.5),
        "three": ("PERSP", Vector((7.5, -7.5, 4.5)), Vector((0, 0, -0.1)), 0),
        "rear": ("PERSP", Vector((-6, 8.5, 4.0)), Vector((0, 0, 0)), 0),
    }
    for name, (kind, pos, target, scale) in views.items():
        cam_data.type = kind
        if kind == "ORTHO":
            cam_data.ortho_scale = scale
        cam.location = pos
        cam.rotation_euler = (target - pos).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, f"{name}.png")
        bpy.ops.render.render(write_still=True)


if PREVIEW:
    preview(PREVIEW)
