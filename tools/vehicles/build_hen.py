"""
Builds the hen (a giant robot hen on the walker's legs) and exports it to client/public/models/hen.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_hen.py -- \
        [--out client/public/models/hen.glb] [--preview DIR]

Everything is procedural and flat-shaded, like build_walker.py, whose legs and conventions it shares.

Conventions
  * Blender: Z up, the beak points along +Y, the hen's right hand side is +X. The glTF exporter turns
    that into Y up, beak along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider (a 2.0 x 2.4 x 3.6 m box, HEN_HALF_EXTENTS, the walker's).
  * Nodes the game looks up by name:
      `body`       the plump hull, breast and cockpit; it does not move on its own
      `head`       neck and head, origin at the base of the neck; the game bobs it forwards and back
      `wing_L/R`   origin at the shoulder; they hang folded down the flanks in the rest pose, and the game
                   swings them out about the fore-and-aft axis to spread and flap them
      `tail`       origin at the base of the tail; the game lifts it when an egg is laid
      `pelvis`     the fixed yoke between the hips
      `hip_L/R`, `knee_L/R`, `ankle_L/R`: the walker's legs (same lengths, same origins), with a feathered
                   drumstick of a thigh, a scaly shin and a three-toed foot; the foot's sole is PAD below
                   the ankle. Legs hang straight down in the rest pose: the game's IK sets every rotation.
  * Materials: `paint` is tinted per hen at runtime; `comb`, `beak`, `leg`, `dark`, `metal`, `glass` keep
    their colours and `lamp` is unlit (the eyes).
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
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/hen.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with HEN_* / WALKER_* in crates/shared/src/consts.rs and render/hen.ts.
# --------------------------------------------------------------------------------------------------
HX, HY, HZ = 1.0, 1.8, 1.2  # half extents of the hull box: sideways, along the beak, up
FOOT_HALF = 1.7
THIGH, SHIN = 2.1, 2.2  # as the walker's: longer than the physics' reach, so they stand bent, knees back
PAD = 0.3  # the foot's thickness: ankle origin to sole
NECK = Vector((0.0, 1.35, 0.55))  # base of the neck
SHOULDER = (1.35, 0.45, 0.5)  # right shoulder; the left is mirrored
TAIL = Vector((0.0, -1.75, 0.55))  # base of the tail

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
    material("paint", (0.93, 0.91, 0.86)),
    material("dark", (0.12, 0.13, 0.14)),
    material("metal", (0.42, 0.44, 0.47)),
    material("comb", (0.85, 0.08, 0.06)),
    material("beak", (0.98, 0.72, 0.12)),
    material("leg", (0.95, 0.6, 0.12)),
    material("glass", (0.45, 0.7, 0.85)),
    material("lamp", (1.0, 0.85, 0.2)),
]
PAINT, DARK, METAL, COMB, BEAK, LEG, GLASS, LAMP = range(8)


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

    def box(self, centre, size, mat, rx=0.0, ry=0.0, rz=0.0):
        m = (
            Matrix.Translation(centre)
            @ Matrix.Rotation(rz, 4, "Z")
            @ Matrix.Rotation(ry, 4, "Y")
            @ Matrix.Rotation(rx, 4, "X")
            @ Matrix.Diagonal((*size, 1.0))
        )
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

    def cylinder(self, centre, radius, depth, mat, axis="Z", segments=10, radius2=None, rx=0.0):
        turn = {"Z": Matrix.Identity(4), "X": Matrix.Rotation(math.pi / 2, 4, "Y"), "Y": Matrix.Rotation(-math.pi / 2, 4, "X")}[axis]
        m = Matrix.Translation(centre) @ Matrix.Rotation(rx, 4, "X") @ turn
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

    def blob(self, centre, scale, mat, u=12, v=8, rx=0.0):
        """A faceted ellipsoid."""
        m = Matrix.Translation(centre) @ Matrix.Rotation(rx, 4, "X") @ Matrix.Diagonal((*scale, 1.0))
        self._tag(bmesh.ops.create_uvsphere(self.bm, u_segments=u, v_segments=v, radius=1.0, matrix=m)["verts"], mat)

    def dome(self, centre, scale, mat, floor, u=12, v=8):
        """The part of an ellipsoid above the horizontal plane `floor` (relative to its centre)."""
        m = Matrix.Translation(centre) @ Matrix.Diagonal((*scale, 1.0))
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=u, v_segments=v, radius=1.0, matrix=m)["verts"]
        below = [vert for vert in verts if vert.co.z < centre[2] + floor * scale[2] - 1e-4]
        bmesh.ops.delete(self.bm, geom=below, context="VERTS")
        self._tag([vert for vert in verts if vert.is_valid], mat)

    def cone(self, base, tip, radius, mat, segments=6):
        """A cone from a circle of `radius` at `base` to a point at `tip`."""
        base, tip = Vector(base), Vector(tip)
        axis = tip - base
        m = Matrix.Translation((base + tip) / 2) @ axis.to_track_quat("Z", "Y").to_matrix().to_4x4()
        verts = bmesh.ops.create_cone(self.bm, cap_ends=True, cap_tris=False, segments=segments, radius1=radius, radius2=0.0, depth=axis.length, matrix=m)["verts"]
        self._tag(verts, mat)

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


# --------------------------------------------------------------------------------------------------
# Body: a plump, faceted egg of a hull, a puffed-out breast, riveted armour bands, a glass cockpit on
# the back for the driver, and a belly hatch the eggs come out of
# --------------------------------------------------------------------------------------------------
body = Part("body")
body.blob((0.0, -0.1, 0.0), (1.3, 2.0, 1.35), PAINT, u=14, v=9)
body.blob((0.0, 1.05, -0.15), (1.05, 0.95, 1.05), PAINT, u=12, v=8)  # breast
body.blob((0.0, -1.2, 0.35), (1.05, 0.9, 0.95), PAINT, u=12, v=8)  # rump, up into the tail
# Armour bands round the middle and the breast, studded with rivets.
for y, rx, sx, sz in ((0.15, 0.0, 1.34, 1.38), (-0.75, 0.15, 1.24, 1.3)):
    body.cylinder((0.0, y, 0.0), 1.0, 0.16, METAL, axis="Y", segments=14, rx=rx)
    body.blob((0.0, y, 0.0), (sx, 0.08, sz), METAL, u=14, v=4)
for k in range(8):
    angle = (k + 0.5) / 8 * math.pi * 2
    body.box((math.sin(angle) * 1.37, 0.15, math.cos(angle) * 1.4), (0.12, 0.12, 0.12), DARK)
# The cockpit: a glass bubble in a dark ring, a little behind the shoulders.
body.cylinder((0.0, -0.35, 1.2), 0.62, 0.18, DARK, segments=12)
body.dome((0.0, -0.35, 1.2), (0.55, 0.6, 0.5), GLASS, floor=0.0, u=12, v=6)
# Belly plate and the egg hatch under the tail.
body.box((0.0, 0.0, -1.3), (1.5, 2.2, 0.25), DARK)
body.cylinder((0.0, -1.75, -0.95), 0.35, 0.2, METAL, axis="Y", segments=10)
body.cylinder((0.0, -1.85, -0.95), 0.22, 0.1, DARK, axis="Y", segments=10)
body_obj = body.build()

# --------------------------------------------------------------------------------------------------
# Head: a thick neck up and forward from the breast, a round head with lamp eyes, a big two-part beak,
# the comb on top and the wattle under the beak
# --------------------------------------------------------------------------------------------------
head = Part("head", location=NECK, parent=body_obj)
head.cylinder((0.0, 0.25, 0.55), 0.62, 1.3, PAINT, segments=10, radius2=0.45, rx=-0.35)  # neck
head.cylinder((0.0, 0.2, 0.2), 0.68, 0.18, METAL, segments=10, rx=-0.35)  # collar ring
head.blob((0.0, 0.55, 1.35), (0.62, 0.7, 0.62), PAINT, u=12, v=8)
for side in (-1, 1):
    head.cylinder((side * 0.55, 0.75, 1.45), 0.2, 0.12, DARK, axis="X", segments=10)  # eye socket
    head.cylinder((side * 0.6, 0.75, 1.45), 0.14, 0.06, LAMP, axis="X", segments=8)  # eye
head.cone((0.0, 1.05, 1.42), (0.0, 1.85, 1.3), 0.3, BEAK, segments=6)  # upper beak
head.cone((0.0, 1.0, 1.2), (0.0, 1.55, 1.12), 0.2, BEAK, segments=6)  # lower beak
head.box((0.0, 1.15, 1.31), (0.5, 0.25, 0.04), DARK)  # the beak's seam
# The comb: four rounded red fins along the crown, tallest in the middle.
for k, (y, height) in enumerate(((0.95, 0.38), (0.6, 0.55), (0.25, 0.5), (-0.05, 0.36))):
    head.blob((0.0, y, 1.9 + height / 2), (0.1, 0.22, height / 2 + 0.1), COMB, u=8, v=6)
# The wattle, two lobes under the beak.
for side in (-1, 1):
    head.blob((side * 0.12, 1.05, 0.85), (0.12, 0.16, 0.26), COMB, u=8, v=6)
head_obj = head.build()

# --------------------------------------------------------------------------------------------------
# Wings: armoured plates with feather slats along their trailing edge, hanging folded down each flank
# --------------------------------------------------------------------------------------------------
wing_objects = []
for side, label in ((-1, "L"), (1, "R")):
    sx, sy, sz = SHOULDER
    wing = Part(f"wing_{label}", location=(side * sx, sy, sz), parent=body_obj)
    wing.cylinder((0, 0, 0), 0.3, 0.3, METAL, axis="X", segments=8)  # the shoulder joint
    # The plate: wide at the shoulder and narrowing down to the tip, leaning back.
    wing.blob((side * 0.3, -0.6, -0.7), (0.24, 1.4, 1.05), PAINT, u=10, v=6, rx=0.3)
    wing.box((side * 0.3, -0.1, -0.2), (0.12, 1.2, 0.3), METAL, rx=0.3)  # leading-edge armour
    for k in range(5):
        # The slats: longer towards the back, as primaries are.
        length = 0.9 + k * 0.2
        wing.box((side * 0.4, 0.1 - k * 0.42, -1.3 - k * 0.08), (0.12, 0.4, length), METAL if k % 2 else DARK, rx=0.25 + k * 0.08)
    wing_objects.append(wing.build())

# --------------------------------------------------------------------------------------------------
# Tail: a fan of stiff feathers sweeping up and back
# --------------------------------------------------------------------------------------------------
tail = Part("tail", location=TAIL, parent=body_obj)
tail.cylinder((0, 0, 0), 0.4, 1.2, METAL, axis="X", segments=8)
for k in range(7):
    spread = (k - 3) / 3
    tail.box((spread * 0.7, -0.5, 0.85 - abs(spread) * 0.3), (0.2, 0.5, 2.0 - abs(spread) * 0.5), PAINT if k % 2 == 0 else METAL, rx=0.6, ry=spread * 0.6)
tail_obj = tail.build()

# --------------------------------------------------------------------------------------------------
# Legs: the walker's joints and lengths. A fat feathered drumstick of a thigh, a thin scaly shin, a
# spur, and a three-toed foot with a back toe
# --------------------------------------------------------------------------------------------------
pelvis = Part("pelvis")
pelvis.box((0.0, 0.0, -HZ - 0.05), (2 * FOOT_HALF - 0.6, 1.0, 0.45), DARK)
pelvis_obj = pelvis.build()

leg_objects = [pelvis_obj]
for side, label in ((-1, "L"), (1, "R")):
    hip = Part(f"hip_{label}", location=(side * FOOT_HALF, 0.0, -HZ))
    hip.cylinder((0, 0, 0), 0.42, 0.7, METAL, axis="X", segments=10)
    hip.blob((0.0, 0.0, -THIGH * 0.42), (0.6, 0.7, THIGH * 0.55), PAINT, u=10, v=7)  # the drumstick
    hip_obj = hip.build()

    knee = Part(f"knee_{label}", location=(0.0, 0.0, -THIGH), parent=hip_obj)
    knee.cylinder((0, 0, 0), 0.3, 0.55, METAL, axis="X", segments=10)
    knee.cylinder((0.0, 0.0, -SHIN / 2), 0.2, SHIN - 0.3, LEG, segments=8, radius2=0.16)  # shin
    for k in range(4):
        knee.cylinder((0.0, 0.0, -0.45 - k * 0.42), 0.23, 0.08, METAL, segments=8)  # scales
    knee.cone((0.0, -0.15, -SHIN * 0.7), (0.0, -0.55, -SHIN * 0.75), 0.09, DARK, segments=5)  # spur
    knee_obj = knee.build()

    ankle = Part(f"ankle_{label}", location=(0.0, 0.0, -SHIN), parent=knee_obj)
    ankle.cylinder((0, 0, 0), 0.24, 0.5, METAL, axis="X", segments=8)
    ankle.box((0.0, 0.0, -PAD / 2), (0.5, 0.55, PAD), LEG)
    for angle in (-0.5, 0.0, 0.5):
        toe = Vector((math.sin(angle), math.cos(angle), 0.0))
        ankle.box((toe.x * 0.65, toe.y * 0.65, -PAD * 0.55), (0.22, 1.3, PAD * 0.7), LEG, rz=-angle)
        ankle.cone((toe.x * 1.25, toe.y * 1.25, -PAD * 0.55), (toe.x * 1.55, toe.y * 1.55, -PAD * 0.8), 0.12, DARK, segments=4)  # claw
    ankle.box((0.0, -0.45, -PAD * 0.55), (0.2, 0.7, PAD * 0.7), LEG)  # back toe
    ankle_obj = ankle.build()
    leg_objects += [hip_obj, knee_obj, ankle_obj]


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
everything = [body_obj, head_obj, tail_obj, *wing_objects, *leg_objects]
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
    scene.render.resolution_x, scene.render.resolution_y = 900, 700
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "side": ("ORTHO", Vector((16, 0, -1.5)), Vector((0, 0, -1.5)), 11.5),
        "front": ("ORTHO", Vector((0, 16, -1.5)), Vector((0, 0, -1.5)), 11.5),
        "three": ("PERSP", Vector((12, 11, 2.5)), Vector((0, 0, -1.5)), 0),
        "rear": ("PERSP", Vector((-9, -12, 3.0)), Vector((0, 0, -1.5)), 0),
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
