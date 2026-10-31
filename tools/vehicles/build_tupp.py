"""
Builds the tupp (a giant robot Hedemora rooster) and exports it to client/public/models/tupp.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_tupp.py -- \
        [--out client/public/models/tupp.glb] [--preview DIR]

The Hedemora is the old cold-hardy landrace of Dalarna, so this is one in armour plate: dark plumage with
a gold hackle and saddle, a small single comb and small wattles against the frost, a dark beak and dark
legs, and feathered "trousers" all the way down to the toes. The rest follows build_hen.py.

Conventions
  * Blender: Z up, the beak points along +Y, the tupp's right hand side is +X. The game's frame is body
    axes +X right, +Y up, -Z forward. Origin is the centre of the hull collider (TUPP_HALF_EXTENTS).
  * Nodes the game looks up by name:
      `body`, `wing_L/R`, `tail`, `pelvis`, `hip_L/R`, `knee_L/R`, `ankle_L/R`, as in build_hen.py
      `head`       the neck and its hackle, origin at the base of the neck; it bobs as a hen's does
      `skull`      under `head`, origin at TUPP_HEAD_PIVOT; the game turns it to where the eyes aim
  * Materials: `paint` (the plumage) and `hackle` are tinted per tupp at runtime; `comb`, `beak`, `leg`,
    `dark`, `metal`, `glass`, `sheen`, `blue`, `yellow` keep their colours and `lamp` is unlit (the eyes).
"""

import math
import os
import sys

import bpy
from mathutils import Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import common
from common import MATERIALS, Bird as Part, arg, export, material

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/tupp.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with TUPP_* / WALKER_* in crates/shared/src/vehicles/ and render/tupp.ts.
# --------------------------------------------------------------------------------------------------
HZ = 1.2  # half height of the hull box
FOOT_HALF = 1.7
THIGH, SHIN = 2.1, 2.2  # as the walker's: longer than the physics' reach, so they stand bent, knees back
PAD = 0.3  # the foot's thickness: ankle origin to sole
NECK = Vector((0.0, 1.35, 0.55))  # base of the neck
PIVOT = Vector((0.0, 2.3, 2.45))  # TUPP_HEAD_PIVOT, with Blender's axes
SHOULDER = (1.35, 0.45, 0.5)  # right shoulder; the left is mirrored
TAIL = Vector((0.0, -1.75, 0.55))  # base of the tail
EYE_X = 0.27  # TUPP_EYE_X

bpy.ops.wm.read_factory_settings(use_empty=True)

MATERIALS[:] = [
    material("paint", (0.05, 0.045, 0.05)),
    material("dark", (0.1, 0.1, 0.11)),
    material("metal", (0.42, 0.44, 0.47)),
    material("comb", (0.8, 0.05, 0.05)),
    material("beak", (0.16, 0.16, 0.19)),
    material("leg", (0.13, 0.13, 0.16)),
    material("glass", (0.45, 0.7, 0.85)),
    material("lamp", (1.0, 0.1, 0.05)),
    material("hackle", (0.85, 0.5, 0.1)),
    material("sheen", (0.02, 0.2, 0.17)),
    material("blue", (0.0, 0.2, 0.5)),
    material("yellow", (0.98, 0.78, 0.08)),
]
PAINT, DARK, METAL, COMB, BEAK, LEG, GLASS, LAMP, HACKLE, SHEEN, BLUE, YELLOW = range(12)


def plates(part, centre, radius, drop, count, size, lean, mat, skip=()):
    """A tier of long feather plates hung round a point on the Z axis, tips out and down."""
    for k in range(count):
        angle = (k + 0.5) / count * math.pi * 2
        if k in skip:
            continue
        out = Vector((math.sin(angle), math.cos(angle), 0.0))
        at = Vector(centre) + out * (radius + math.sin(lean) * size[2] / 2) + Vector((0.0, 0.0, -drop))
        part.box(at, size, mat, rx=lean, rz=-angle)


# --------------------------------------------------------------------------------------------------
# Body: the hen's plump hull carried more upright, a deep breast, armour bands, a glass cockpit on the
# back, and a cape of gold saddle plates over the rump
# --------------------------------------------------------------------------------------------------
body = Part("body")
body.blob((0.0, -0.1, 0.05), (1.25, 1.95, 1.3), PAINT, u=14, v=9, rx=0.15)
body.blob((0.0, 1.1, -0.05), (1.05, 0.95, 1.15), PAINT, u=12, v=8)  # breast
body.blob((0.0, -1.2, 0.3), (1.0, 0.9, 0.9), PAINT, u=12, v=8)  # rump
for y, rx, sx, sz in ((0.15, 0.0, 1.3, 1.38), (-0.75, 0.15, 1.22, 1.3)):
    body.cylinder((0.0, y, 0.0), 1.0, 0.16, METAL, axis="Y", segments=14, rx=rx)
    body.blob((0.0, y, 0.0), (sx, 0.08, sz), METAL, u=14, v=4)
for k in range(8):
    angle = (k + 0.5) / 8 * math.pi * 2
    body.box((math.sin(angle) * 1.33, 0.15, math.cos(angle) * 1.4), (0.12, 0.12, 0.12), DARK)
# The cockpit, where the hen's is.
body.cylinder((0.0, -0.35, 1.2), 0.62, 0.18, DARK, segments=12)
body.dome((0.0, -0.35, 1.2), (0.55, 0.6, 0.5), GLASS, floor=0.0, u=12, v=6)
# The saddle: gold plates fanning out over the rump, either side of the cockpit.
for k in range(9):
    spread = (k - 4) / 4
    body.box((spread * 1.05, -1.75 + abs(spread) * 0.45, 0.65 - abs(spread) * 0.6), (0.3, 0.1, 1.15), HACKLE, rx=0.65, ry=spread * 0.8)
body.box((0.0, 0.0, -1.3), (1.5, 2.2, 0.25), DARK)  # belly plate
body_obj = body.build()

# --------------------------------------------------------------------------------------------------
# Head: a long neck under a thick gold hackle that spills over the shoulders. The skull turns on its own
# at the top of it: two forward red lenses under a scowl, a dark beak, the small comb and wattles
# --------------------------------------------------------------------------------------------------
head = Part("head", location=NECK, parent=body_obj)
top = PIVOT - NECK
head.cylinder((0.0, top.y * 0.5, top.z * 0.5), 0.62, 2.35, PAINT, segments=10, radius2=0.36, rx=-0.46)  # neck
head.cylinder((0.0, 0.12, 0.12), 0.72, 0.16, METAL, segments=10, rx=-0.46)  # collar ring
# Tier on tier from just under the skull: narrow at the top, broad and flared down on the shoulders.
for k, share, radius, count, width, height, lean in (
    (0, 1.0, 0.36, 9, 0.34, 0.95, 0.12),
    (1, 0.8, 0.42, 10, 0.38, 1.05, 0.2),
    (2, 0.6, 0.5, 11, 0.42, 1.15, 0.28),
    (3, 0.4, 0.6, 12, 0.46, 1.2, 0.4),
    (4, 0.2, 0.74, 13, 0.5, 1.2, 0.6),
):
    plates(head, (0.0, top.y * share, top.z * share + 0.05), radius, 0.3, count, (width, 0.09, height), lean, HACKLE)
head_obj = head.build()

skull = Part("skull", location=top, parent=head_obj)
skull.blob((0.0, 0.0, 0.0), (0.5, 0.62, 0.48), PAINT, u=12, v=8)
for side in (-1, 1):
    skull.cylinder((side * EYE_X, 0.46, 0.05), 0.2, 0.14, DARK, axis="Y", segments=10)  # eye socket
    skull.cylinder((side * EYE_X, 0.53, 0.05), 0.15, 0.04, LAMP, axis="Y", segments=10)  # lens
    skull.box((side * 0.25, 0.55, 0.26), (0.36, 0.14, 0.08), METAL, ry=side * 0.38)  # the scowl
skull.cone((0.0, 0.45, -0.04), (0.0, 1.2, -0.28), 0.24, BEAK, segments=6)  # upper beak, hooked down
skull.cone((0.0, 0.42, -0.2), (0.0, 0.9, -0.34), 0.15, BEAK, segments=6)  # lower beak
# The comb: a small single blade with its points along the crown. Big ones lose their tips to the frost.
skull.box((0.0, -0.02, 0.5), (0.1, 0.72, 0.14), COMB)
for y, height in ((0.32, 0.2), (0.14, 0.28), (-0.04, 0.3), (-0.22, 0.24), (-0.38, 0.16)):
    skull.cone((0.0, y, 0.52), (0.0, y - 0.02, 0.52 + height), 0.1, COMB, segments=4)
for side in (-1, 1):
    skull.blob((side * 0.09, 0.55, -0.46), (0.08, 0.1, 0.16), COMB, u=8, v=6)  # the wattles, small too
skull_obj = skull.build()

# --------------------------------------------------------------------------------------------------
# Wings: dark plates with sheened slats, and a Swedish roundel on each, as a fighter of Dalarna should
# --------------------------------------------------------------------------------------------------
wing_objects = []
for side, label in ((-1, "L"), (1, "R")):
    sx, sy, sz = SHOULDER
    wing = Part(f"wing_{label}", location=(side * sx, sy, sz), parent=body_obj)
    wing.cylinder((0, 0, 0), 0.3, 0.3, METAL, axis="X", segments=8)  # the shoulder joint
    wing.blob((side * 0.3, -0.6, -0.7), (0.24, 1.4, 1.05), PAINT, u=10, v=6, rx=0.3)
    wing.box((side * 0.3, -0.1, -0.2), (0.12, 1.2, 0.3), METAL, rx=0.3)  # leading-edge armour
    for k in range(5):
        length = 0.9 + k * 0.2
        wing.box((side * 0.4, 0.1 - k * 0.42, -1.3 - k * 0.08), (0.12, 0.4, length), SHEEN if k % 2 else HACKLE, rx=0.25 + k * 0.08)
    wing.cylinder((side * 0.5, -0.45, -0.6), 0.42, 0.06, YELLOW, axis="X", segments=12)
    wing.cylinder((side * 0.54, -0.45, -0.6), 0.33, 0.06, BLUE, axis="X", segments=12)
    wing_objects.append(wing.build())

# --------------------------------------------------------------------------------------------------
# Tail: the rooster's whole point. Long sickle plates arching up and over behind it, sheened green-black
# --------------------------------------------------------------------------------------------------
tail = Part("tail", location=TAIL, parent=body_obj)
tail.cylinder((0, 0, 0), 0.4, 1.2, METAL, axis="X", segments=8)
for k in range(9):
    spread = (k - 4) / 4
    reach = 1.0 - abs(spread) * 0.3  # the middle ones are the longest
    at = Vector((spread * 0.45, -0.1, 0.2))
    angle = 0.25  # from straight up, leaning back, more with every segment
    for segment in range(6):
        length = 0.8 * reach
        step = Vector((spread * 0.2, -math.sin(angle), math.cos(angle))) * length
        mat = SHEEN if k % 2 == 0 else PAINT
        tail.box(at + step / 2, (0.42 - segment * 0.04, 0.08, length + 0.08), mat, rx=angle, ry=spread * 0.25)
        at += step
        angle += 0.42
tail_obj = tail.build()

# --------------------------------------------------------------------------------------------------
# Legs: the walker's joints and lengths. Fluffed-out trousers, and feather tufts down the dark shin to
# the toes, which is what a Hedemora keeps its feet warm with
# --------------------------------------------------------------------------------------------------
pelvis = Part("pelvis")
pelvis.box((0.0, 0.0, -HZ - 0.05), (2 * FOOT_HALF - 0.6, 1.0, 0.45), DARK)
pelvis_obj = pelvis.build()

leg_objects = [pelvis_obj]
for side, label in ((-1, "L"), (1, "R")):
    hip = Part(f"hip_{label}", location=(side * FOOT_HALF, 0.0, -HZ))
    hip.cylinder((0, 0, 0), 0.42, 0.7, METAL, axis="X", segments=10)
    hip.blob((0.0, 0.0, -THIGH * 0.42), (0.68, 0.78, THIGH * 0.56), PAINT, u=10, v=7)  # the drumstick
    plates(hip, (0.0, 0.0, -THIGH * 0.72), 0.5, 0.2, 8, (0.4, 0.1, 0.95), 0.3, PAINT)  # the trousers
    hip_obj = hip.build()

    knee = Part(f"knee_{label}", location=(0.0, 0.0, -THIGH), parent=hip_obj)
    knee.cylinder((0, 0, 0), 0.3, 0.55, METAL, axis="X", segments=10)
    knee.cylinder((0.0, 0.0, -SHIN / 2), 0.2, SHIN - 0.3, LEG, segments=8, radius2=0.16)  # shin
    for k in range(3):
        plates(knee, (0.0, 0.0, -0.35 - k * 0.6), 0.18, 0.1, 6, (0.24, 0.08, 0.7 - k * 0.06), 0.35, PAINT)
    knee.cone((0.0, -0.15, -SHIN * 0.8), (0.0, -0.75, -SHIN * 0.87), 0.1, METAL, segments=5)  # a cock's long spur
    knee_obj = knee.build()

    ankle = Part(f"ankle_{label}", location=(0.0, 0.0, -SHIN), parent=knee_obj)
    ankle.cylinder((0, 0, 0), 0.24, 0.5, METAL, axis="X", segments=8)
    ankle.box((0.0, 0.0, -PAD / 2), (0.5, 0.55, PAD), LEG)
    for angle in (-0.5, 0.0, 0.5):
        toe = Vector((math.sin(angle), math.cos(angle), 0.0))
        ankle.box((toe.x * 0.65, toe.y * 0.65, -PAD * 0.55), (0.22, 1.3, PAD * 0.7), LEG, rz=-angle)
        ankle.box((toe.x * 0.6, toe.y * 0.6, -PAD * 0.1), (0.3, 0.8, 0.1), PAINT, rz=-angle)  # feathered to the toes
        ankle.cone((toe.x * 1.25, toe.y * 1.25, -PAD * 0.55), (toe.x * 1.55, toe.y * 1.55, -PAD * 0.8), 0.12, DARK, segments=4)  # claw
    ankle.box((0.0, -0.45, -PAD * 0.55), (0.2, 0.7, PAD * 0.7), LEG)  # back toe
    ankle_obj = ankle.build()
    leg_objects += [hip_obj, knee_obj, ankle_obj]


# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [body_obj, head_obj, skull_obj, tail_obj, *wing_objects, *leg_objects]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)

if PREVIEW:
    common.preview(PREVIEW, middle=-0.6, scale=14)
