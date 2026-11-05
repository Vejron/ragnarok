"""
Imports Quaternius' CC0 deer and stag (poly.pizza) and exports them to client/public/models/{deer,stag}.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup -P tools/npc/import_deer.py -- \
        --source FILE --animal deer|stag [--out FILE]

The sources are the "Deer" and "Stag" of Quaternius' Ultimate Animated Animals, public domain:
    https://poly.pizza/m/T6Cs7tmMHJ   (deer)
    https://poly.pizza/m/tQdzbZ1Cmw   (stag)

What it changes
  * Scale and heading: the source faces -Y in Blender and is a few units tall. It is turned to face +Y
    (the glTF exporter makes that -Z, the game's forward at yaw 0) and scaled so the deer stands about
    1.6 m to the top of its head and the stag 1.85 m (2.5 m to its antler tips). Origin on the ground.
  * Clips: only the ones the game plays, renamed:
      `idle`    <- Idle      looping, standing about
      `graze`   <- Eating    looping, head down cropping grass (FLAG_GRAZING)
      `walk`    <- Walk      looping
      `gallop`  <- Gallop    looping, played faster or slower with ground speed
      `death`   <- Death     once, falling over onto its side, held while it lies dead (FLAG_DEAD)
  * Leaf-bone empties, a stray icosphere and vertex colours go; materials keep their colours.
"""

import math
import os
import sys

import bpy

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def arg(name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


HERE = os.path.dirname(os.path.abspath(__file__))
ANIMAL = arg("--animal", "deer")
SOURCE = arg("--source")
OUT = arg("--out", os.path.normpath(os.path.join(HERE, f"../../client/public/models/{ANIMAL}.glb")))
if not SOURCE:
    sys.exit("--source FILE is required")

# Source height of the body mesh (feet to top of head) -> metres in game.
HEIGHT = {"deer": 1.6, "stag": 1.85}[ANIMAL]
CLIPS = {"Idle": "idle", "Eating": "graze", "Walk": "walk", "Gallop": "gallop", "Death": "death"}

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=SOURCE)
bpy.context.view_layer.update()

for obj in list(bpy.data.objects):
    if obj.name == "Icosphere" or (obj.type == "EMPTY" and obj.name.endswith("_end")):
        bpy.data.objects.remove(obj, do_unlink=True)

# The source has each clip twice, once as "AnimalArmature|Name".
for action in list(bpy.data.actions):
    if action.name in CLIPS:
        action.name = CLIPS[action.name]
        action.use_fake_user = True
    else:
        bpy.data.actions.remove(action)

arm_obj = bpy.data.objects["AnimalArmature"]
body = next(o for o in bpy.data.objects if o.type == "MESH" and o.parent == arm_obj and o.parent_type == "OBJECT")
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
for obj in meshes:
    for name in [a.name for a in obj.data.color_attributes]:
        obj.data.color_attributes.remove(obj.data.color_attributes[name])

top = max((body.matrix_world @ v.co).z for v in body.data.vertices)
root = arm_obj.parent
root.scale = (HEIGHT / top,) * 3
root.rotation_mode = "XYZ"
root.rotation_euler = (0.0, 0.0, math.pi)
bpy.context.view_layer.update()

# Rest the armature in its idle pose so the exported bind pose is a standing animal.
arm_obj.animation_data_create()
arm_obj.animation_data.action = bpy.data.actions["idle"]


def export():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    for obj in [root, arm_obj, *meshes]:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = arm_obj
    kwargs = dict(
        filepath=OUT,
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=False,
        export_skins=True,
        export_animations=True,
        export_animation_mode="ACTIONS",
        export_frame_range=False,
        export_force_sampling=True,
        export_optimize_animation_size=True,
        export_vertex_color="NONE",
        export_cameras=False,
        export_lights=False,
        export_extras=False,
    )
    valid = bpy.ops.export_scene.gltf.get_rna_type().properties.keys()
    bpy.ops.export_scene.gltf(**{k: v for k, v in kwargs.items() if k in valid})


export()
