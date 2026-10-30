"""
Builds the walker (an AT-ST in silhouette) and exports it to client/public/models/walker.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_walker.py -- \
        [--out client/public/models/walker.glb] [--preview DIR]

Everything is procedural and flat-shaded.

Conventions
  * Blender: Z up, the guns point along +Y, the walker's right hand side is +X. The glTF exporter turns
    that into Y up, guns along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider (a 2.0 x 2.4 x 3.6 m box, WALKER_HALF_EXTENTS).
  * Nodes the game looks up by name:
      `torso`      the upper body, origin at the hull centre; the game yaws it with the aim
      `gun`        child of `torso`, origin on WALKER_GUN_PIVOT; the game pitches it and slides it back in recoil
      `hip_L/R`    legs, origin at the hip (WALKER_FOOT_HALF sideways, the bottom of the hull); the game swings them
      `pelvis`     the fixed yoke between the hips, on the legs and not the upper body
      `knee_L/R`   child of the hip, one thigh below it
      `ankle_L/R`  child of the knee, one shin below it; the foot's sole is PAD below this origin
    Legs hang straight down in the rest pose: the game's IK sets every rotation from that.
  * Materials: `paint` is tinted per walker at runtime; `dark`, `metal`, `steel` keep their colours and
    `lamp` is unlit (viewports, muzzle glow).
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import common
from common import MATERIALS, arg, export, material

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/walker.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with WALKER_* in crates/shared/src/vehicles/ and render/walker.ts.
# --------------------------------------------------------------------------------------------------
HX, HY, HZ = 1.0, 1.8, 1.2  # half extents of the hull box: sideways, along the guns, up
FOOT_HALF = 1.7
# The legs are longer than the physics' reach (WALKER_LEG_LENGTH = 4.2) so that they stand bent, knees back.
THIGH, SHIN = 2.1, 2.2
PAD = 0.3  # the foot's thickness: ankle origin to sole
GUN_PIVOT = Vector((0.0, 0.0, 0.1))  # game [0, 0.1, 0]
GUN_LENGTH = 3.7

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


MATERIALS[:] = [
    material("paint", (0.55, 0.58, 0.58)),
    material("dark", (0.12, 0.13, 0.14)),
    material("metal", (0.26, 0.28, 0.3)),
    material("steel", (0.6, 0.62, 0.64)),
    material("lamp", (1.0, 0.5, 0.12)),
]
PAINT, DARK, METAL, STEEL, LAMP = range(5)


class Part(common.Part):

    def taper(self, centre, top, bottom, height, mat):
        """A box that narrows (or widens) from `top` to `bottom`, each an (x, y) size, `height` tall along Z."""
        cx, cy, cz = centre
        corners = lambda size, z: [(cx + sx * size[0] / 2, cy + sy * size[1] / 2, cz + z) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        t = [self.bm.verts.new(p) for p in corners(top, height / 2)]
        b = [self.bm.verts.new(p) for p in corners(bottom, -height / 2)]
        faces = [self.bm.faces.new(t), self.bm.faces.new(b[::-1])]
        for i in range(4):
            j = (i + 1) % 4
            faces.append(self.bm.faces.new((t[i], b[i], b[j], t[j])))
        for face in faces:
            face.material_index = mat


# --------------------------------------------------------------------------------------------------
# Upper body: the AT-ST's head, a narrow wedge that tapers to a chin, with slit viewports under a
# brow, cheek-mounted guns, ear hatches and a rear pack
# --------------------------------------------------------------------------------------------------
torso = Part("torso")
# Lower head: side profile (y, z), widest at the back, the chin thrust forward and down.
torso.prism(
    [(-1.8, -0.95), (1.0, -1.2), (1.75, -0.75), (1.85, -0.1), (1.35, 0.35), (-1.8, 0.35)],
    -0.8,
    0.8,
    PAINT,
)
# Upper head: narrower, a raked forehead over the viewports and a long roof sloping to the back.
torso.prism(
    [(-1.7, 0.3), (1.3, 0.3), (1.55, 0.75), (0.9, 1.15), (-0.6, 1.25), (-1.7, 0.9)],
    -0.55,
    0.55,
    PAINT,
)
for side in (-1, 1):
    torso.box((side * 0.4, 1.66, 0.03), (0.4, 0.1, 0.16), LAMP)  # slit viewports
    torso.box((side * 0.4, 1.7, 0.2), (0.5, 0.16, 0.1), DARK, rx=0.4)  # their brow
    # The ear hatches on the roof's flanks, and the cheek guns: a box and a short barrel each.
    torso.cylinder((side * 0.4, -0.7, 1.27), 0.28, 0.1, METAL, segments=10)
    torso.box((side * 0.92, 0.6, -0.1), (0.3, 1.6, 0.5), METAL)
    torso.cylinder((side * 0.92, 1.7, -0.1), 0.07, 1.0, DARK, axis="Y", segments=6)
    torso.box((side * 0.6, -0.6, 0.62), (0.08, 1.8, 0.5), DARK)  # raked side armour
torso.box((0.0, 0.0, -1.2), (1.3, 3.0, 0.2), DARK)  # belly
torso.box((0.0, -2.0, 0.0), (1.0, 0.5, 1.0), DARK)  # rear pack
for side in (-1, 1):
    torso.cylinder((side * 0.35, -2.35, 0.35), 0.1, 0.5, METAL, axis="Y", segments=8)
torso.cylinder((0.0, -0.4, 1.3), 0.3, 0.1, STEEL, segments=8)  # roof hatch
torso.cylinder((0.5, -1.2, 1.75), 0.02, 1.0, METAL, segments=5)  # antenna
torso_obj = torso.build()

# The chin guns: a housing with two long barrels, which lift together about the pivot.
gun = Part("gun", location=GUN_PIVOT, parent=torso_obj)
gun.box((0.0, 1.55, -1.0), (0.9, 0.95, 0.55), DARK)  # mount under the chin
gun.box((0.0, 1.0, -0.8), (0.6, 0.6, 0.3), PAINT)
for side in (-1, 1):
    x = side * 0.3
    gun.cylinder((x, 2.8, -1.0), 0.07, GUN_LENGTH - 2.0, METAL, axis="Y", segments=8)  # barrel
    gun.cylinder((x, 2.2, -1.0), 0.12, 0.7, DARK, axis="Y", segments=8)  # shroud
    gun.cylinder((x, GUN_LENGTH - 0.07, -1.0), 0.09, 0.14, STEEL, axis="Y", segments=8)  # muzzle brake
    gun.cylinder((x, GUN_LENGTH + 0.0, -1.0), 0.05, 0.04, LAMP, axis="Y", segments=6)
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Legs: long and thin like a bird's: a hip ball, thigh, a knee joint with its piston, a long shin, an
# ankle and a big three-toed foot, hanging straight down
# --------------------------------------------------------------------------------------------------
# The pelvis carries the hips and does not turn with the upper body: a yoke out to each hip.
pelvis = Part("pelvis")
pelvis.box((0.0, 0.0, -HZ - 0.12), (2 * FOOT_HALF - 0.4, 1.3, 0.5), PAINT)
pelvis.box((0.0, 0.0, -HZ + 0.1), (1.4, 1.6, 0.2), DARK)
pelvis.box((0.0, 0.55, -HZ - 0.12), (1.2, 0.2, 0.4), METAL)
pelvis_obj = pelvis.build()

leg_objects = [pelvis_obj]
for side, label in ((-1, "L"), (1, "R")):
    hip = Part(f"hip_{label}", location=(side * FOOT_HALF, 0.0, -HZ))
    hip.cylinder((0, 0, 0), 0.42, 0.7, DARK, axis="X", segments=10)
    hip.cylinder((side * 0.36, 0, 0), 0.3, 0.14, METAL, axis="X", segments=8)
    hip.taper((0.0, 0.0, -THIGH / 2 - 0.05), (0.5, 0.6), (0.4, 0.5), THIGH - 0.5, PAINT)  # thigh
    hip.cylinder((0.0, 0.34, -THIGH / 2), 0.06, THIGH - 0.6, STEEL, segments=6)  # front piston
    hip.box((0.0, -0.3, -0.7), (0.3, 0.12, 0.9), DARK)
    hip_obj = hip.build()

    knee = Part(f"knee_{label}", location=(0.0, 0.0, -THIGH), parent=hip_obj)
    knee.cylinder((0, 0, 0), 0.32, 0.6, METAL, axis="X", segments=10)
    knee.box((0.0, -0.32, 0.0), (0.45, 0.28, 0.6), PAINT, rx=0.15)  # kneecap: the knee points back
    knee.taper((0.0, 0.0, -SHIN / 2 - 0.05), (0.38, 0.48), (0.28, 0.36), SHIN - 0.5, DARK)  # shin
    knee.cylinder((0.0, 0.3, -SHIN / 2), 0.05, SHIN - 0.6, STEEL, segments=6)  # front piston
    knee_obj = knee.build()

    ankle = Part(f"ankle_{label}", location=(0.0, 0.0, -SHIN), parent=knee_obj)
    ankle.cylinder((0, 0, 0), 0.26, 0.55, METAL, axis="X", segments=8)
    ankle.box((0.0, 0.0, -PAD / 2), (0.55, 0.7, PAD), DARK)  # the pad's heel block
    for angle in (-0.55, 0.0, 0.55):
        ankle.box((math.sin(angle) * 0.5, 0.3 + math.cos(angle) * 0.5, -PAD * 0.4), (0.26, 1.1, PAD * 0.8), PAINT if angle == 0.0 else DARK)
    ankle.box((0.0, -0.5, -PAD * 0.4), (0.3, 0.5, PAD * 0.8), DARK)  # heel spur
    ankle_obj = ankle.build()
    leg_objects += [hip_obj, knee_obj, ankle_obj]

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [torso_obj, gun_obj, *leg_objects]
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
        "side": ("ORTHO", Vector((14, 0, -2.5)), Vector((0, 0, -2.5)), 11.0),
        "front": ("ORTHO", Vector((0, -14, -2.5)), Vector((0, 0, -2.5)), 11.0),
        "three": ("PERSP", Vector((11, -11, 2.0)), Vector((0, 0, -2.5)), 0),
        "rear": ("PERSP", Vector((-9, 11, 2.5)), Vector((0, 0, -2.5)), 0),
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
