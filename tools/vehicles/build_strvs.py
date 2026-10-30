"""
Builds the Strv S, a Swedish turretless tank (the "S-tank"), and exports it to client/public/models/strvs.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_strvs.py -- \
        [--out client/public/models/strvs.glb] [--preview DIR]

Everything is procedural and flat-shaded, well under three thousand triangles in all.

Conventions
  * Blender: Z up, the gun points along +Y, the tank's right hand side is +X. The glTF exporter turns
    that into Y up, gun along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider (a 2.6 x 0.9 x 5.4 m box). The ground is 1.08 m below it.
  * There is no turret and no casemate: the gun is fixed in the hull, and the whole tank turns to aim.
  * Nodes the game looks up by name:
      `hull`            everything fixed to the hull, tracks included
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

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from common import MATERIALS, Part, arg, export, material, stadium

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/strvs.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with STRV_* in crates/shared/src/vehicles/.
# --------------------------------------------------------------------------------------------------
GUN_PIVOT = Vector((0.0, 1.3, 0.72))  # game [0, 0.72, -1.3]: +Z in the game is backwards
BARREL_LENGTH = 4.4
TRACK_X = 1.25  # centre line of each track
TRACK_WIDTH = 0.46
TRACK_Z = -0.56  # height of the wheel axles
WHEEL_RADIUS = 0.44
ROAD_WHEELS = (-1.9, -0.95, 0.0, 0.95, 1.9)
SPROCKET_Y, IDLER_Y = -2.5, 2.5

# --------------------------------------------------------------------------------------------------
# Scene and materials
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


MATERIALS[:] = [
    material("paint", (0.62, 0.66, 0.5)),
    material("dark", (0.1, 0.1, 0.1)),
    material("metal", (0.24, 0.26, 0.23)),
    material("steel", (0.55, 0.56, 0.55)),
]
PAINT, DARK, METAL, STEEL = range(4)


# --------------------------------------------------------------------------------------------------
# Hull and tracks
# --------------------------------------------------------------------------------------------------
hull = Part("hull")
HALF_W = TRACK_X - TRACK_WIDTH / 2  # the hull's side plates meet the inner faces of the tracks
# Side profile, (y, z): one low wedge. A flat engine deck, a slightly raised flat roof and a long glacis
# leaning back about 55 degrees from upright. There is no turret and no box on top: the gun comes out of
# the top of the glacis. The belly sits well clear of the ground, between big road wheels.
hull.prism(
    [(-2.7, -0.4), (-2.7, 0.3), (-2.45, 0.5), (-0.9, 0.5), (-0.7, 0.78), (1.5, 0.78), (2.7, -0.02), (2.7, -0.3), (2.45, -0.4)],
    -HALF_W,
    HALF_W,
    PAINT,
)
for side in (-1, 1):
    x = side * TRACK_X
    outer = stadium(SPROCKET_Y * -1, SPROCKET_Y, TRACK_Z, WHEEL_RADIUS + 0.08)
    inner = stadium(SPROCKET_Y * -1, SPROCKET_Y, TRACK_Z, WHEEL_RADIUS + 0.01)
    hull.ring(outer, inner, x - TRACK_WIDTH / 2, x + TRACK_WIDTH / 2, DARK)
    # Mudguard over the track, a stowage bin at the back and a headlamp at the front.
    hull.box((x, 0.0, 0.05), (TRACK_WIDTH + 0.14, 5.6, 0.06), PAINT)
    hull.box((x, 2.75, 0.0), (TRACK_WIDTH + 0.14, 0.5, 0.05), PAINT, rx=-0.45)
    hull.box((x, -1.6, 0.2), (0.4, 1.0, 0.3), PAINT)
    hull.box((x, 2.3, 0.2), (0.2, 0.2, 0.16), METAL)
# Flat hatches on the roof (the driver's to the left of the gun, the commander's to the right), the engine
# deck grille and the exhaust at the tail.
hull.box((-0.45, 0.1, 0.8), (0.5, 0.5, 0.06), PAINT)
hull.box((-0.45, 0.1, 0.84), (0.34, 0.34, 0.04), METAL)
hull.box((0.5, 0.0, 0.8), (0.42, 0.42, 0.06), PAINT)
hull.box((0.5, 0.0, 0.85), (0.2, 0.3, 0.06), METAL)
hull.box((0.0, -1.8, 0.51), (1.1, 1.2, 0.05), DARK)
hull.cylinder((0.6, -2.75, 0.1), 0.07, 0.3, DARK, axis="Y", segments=6)
# The dozer blade stub under the nose, as the real one has.
hull.box((0.0, 2.72, -0.28), (2.0, 0.12, 0.16), METAL)
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
# The gun: a small mantlet in the top of the glacis and a very long barrel with a bore evacuator, origin on the trunnion.
# --------------------------------------------------------------------------------------------------
gun = Part("gun", location=GUN_PIVOT)
gun.box((0.0, 0.1, 0.0), (0.5, 0.5, 0.36), PAINT)  # mantlet
gun.cylinder((0.0, (0.35 + BARREL_LENGTH) / 2, 0.0), 0.13, BARREL_LENGTH - 0.35, METAL, axis="Y", segments=10, radius2=0.09)
gun.cylinder((0.0, 2.9, 0.0), 0.17, 0.6, METAL, axis="Y", segments=8)  # bore evacuator
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
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
