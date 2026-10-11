"""
Builds the Hedemora rooster NPC, with its animations, and exports it to client/public/models/rooster.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/npc/build_rooster.py -- \
        [--out FILE] [--preview DIR]

Everything is procedural and flat-shaded, a few hundred triangles, in the style of the character and cars.

Conventions
  * Blender: Z up, the rooster faces +Y and its left is +X. The glTF exporter turns that into Y up,
    facing -Z, which is the game's forward at yaw 0. The origin is on the ground between the feet.
  * Roughly 0.85 m from feet to comb, 0.65 m beak to tail: a rooster the size of a goofy rooster.
  * Clips, baked at 30 fps (the game plays them by name):
      `idle`   looping, 1.6 s: breathing, a peck now and then
      `walk`   looping, 0.6 s: about the pace the server walks it (ROOSTER_SPEED in consts.rs)
      `crow`   once, 50 frames = 1.67 s, which is ROOSTER_CROW_TICKS at 60 Hz: chest up, head back,
               beak open in pulses, wings out ("ko-ko-kooo!")
  * Materials keep their colours: `feather`, `hackle`, `comb`, `beak`, `leg`, `eye`.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def arg(name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/rooster.glb")))
PREVIEW = arg("--preview")

FPS = 30
IDLE_FRAMES = 48
WALK_FRAMES = 18
CROW_FRAMES = 50

MATERIAL_COLORS = {
    "feather": (0.07, 0.09, 0.08),  # near black, a green sheen on the tail
    "hackle": (0.72, 0.38, 0.1),  # the golden-red neck and saddle
    "comb": (0.8, 0.06, 0.05),
    "beak": (0.95, 0.72, 0.15),
    "leg": (0.9, 0.62, 0.12),
    "eye": (0.02, 0.02, 0.02),
    "sheen": (0.1, 0.32, 0.22),
}
NAMES = list(MATERIAL_COLORS)
FEATHER, HACKLE, COMB, BEAK, LEG, EYE, SHEEN = range(len(NAMES))

# name: (parent, head, tail). Bones point along +Y with no roll, so their local axes are the world axes:
# +X rotation lifts the front, +Y rolls to the right, +Z turns to the left.
BONES = {
    "root": (None, (0, 0, 0.0)),
    "body": ("root", (0, 0, 0.4)),
    "neck": ("body", (0, 0.14, 0.5)),
    "head": ("neck", (0, 0.2, 0.68)),
    "beak_low": ("head", (0, 0.27, 0.72)),
    "tail": ("body", (0, -0.24, 0.46)),
    "wing_L": ("body", (0.16, 0.02, 0.5)),
    "wing_R": ("body", (-0.16, 0.02, 0.5)),
    "leg_L": ("root", (0.07, 0.0, 0.32)),
    "leg_R": ("root", (-0.07, 0.0, 0.32)),
}
BONE_LENGTH = 0.06

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.fps = FPS

MATERIALS = []
for _name, _rgb in MATERIAL_COLORS.items():
    _m = bpy.data.materials.new(_name)
    _m.use_nodes = True
    _bsdf = _m.node_tree.nodes["Principled BSDF"]
    _bsdf.inputs["Base Color"].default_value = (*_rgb, 1.0)
    _bsdf.inputs["Roughness"].default_value = 1.0
    _m.diffuse_color = (*_rgb, 1.0)
    MATERIALS.append(_m)

# --------------------------------------------------------------------------------------------------
# Armature
# --------------------------------------------------------------------------------------------------
arm_data = bpy.data.armatures.new("rooster")
arm_obj = bpy.data.objects.new("rooster", arm_data)
scene.collection.objects.link(arm_obj)
bpy.context.view_layer.objects.active = arm_obj
bpy.ops.object.mode_set(mode="EDIT")
for _name, (_parent, _head) in BONES.items():
    _b = arm_data.edit_bones.new(_name)
    _b.head = Vector(_head)
    _b.tail = Vector(_head) + Vector((0, BONE_LENGTH, 0))
    _b.roll = 0.0
    if _parent:
        _b.parent = arm_data.edit_bones[_parent]
bpy.ops.object.mode_set(mode="OBJECT")
for _pb in arm_obj.pose.bones:
    _pb.rotation_mode = "XYZ"

# --------------------------------------------------------------------------------------------------
# Mesh: one object, every primitive weighted fully to one bone
# --------------------------------------------------------------------------------------------------
bm = bmesh.new()
deform = bm.verts.layers.deform.verify()
GROUP = {name: i for i, name in enumerate(BONES)}


def _finish(verts, bone, material):
    for v in verts:
        v[deform][GROUP[bone]] = 1.0
    for face in {f for v in verts for f in v.link_faces}:
        face.material_index = material


def box(centre, size, bone, material, rotate=(0.0, 0.0, 0.0)):
    m = Matrix.Translation(centre) @ Euler(rotate, "XYZ").to_matrix().to_4x4() @ Matrix.Diagonal((*size, 1.0))
    _finish(bmesh.ops.create_cube(bm, size=1.0, matrix=m)["verts"], bone, material)


def blob(centre, radii, bone, material, rotate=(0.0, 0.0, 0.0)):
    m = Matrix.Translation(centre) @ Euler(rotate, "XYZ").to_matrix().to_4x4() @ Matrix.Diagonal((*radii, 1.0))
    _finish(bmesh.ops.create_icosphere(bm, subdivisions=1, radius=1.0, matrix=m)["verts"], bone, material)


def cone(base, tip, radius, bone, material, segments=5):
    """A cone from `base` (radius `radius`) to a point at `tip`."""
    base, tip = Vector(base), Vector(tip)
    axis = tip - base
    rot = axis.to_track_quat("Z", "Y").to_matrix().to_4x4()
    m = Matrix.Translation((base + tip) / 2) @ rot
    verts = bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=segments, radius1=radius, radius2=0.0, depth=axis.length, matrix=m)["verts"]
    _finish(verts, bone, material)


def build_body():
    # Trunk: dark breast and belly, a golden saddle over the back.
    blob((0, 0.0, 0.42), (0.17, 0.27, 0.2), "body", FEATHER)
    blob((0, 0.12, 0.47), (0.15, 0.2, 0.18), "body", FEATHER)
    blob((0, -0.1, 0.52), (0.14, 0.16, 0.1), "body", HACKLE)
    # Neck and head, golden hackle.
    blob((0, 0.16, 0.6), (0.085, 0.09, 0.13), "neck", HACKLE, rotate=(0.3, 0, 0))
    blob((0, 0.22, 0.74), (0.065, 0.085, 0.075), "head", HACKLE)
    # The comb: three red fins, tallest in the middle.
    box((0, 0.2, 0.83), (0.025, 0.05, 0.05), "head", COMB)
    box((0, 0.24, 0.82), (0.025, 0.045, 0.06), "head", COMB)
    box((0, 0.275, 0.8), (0.025, 0.04, 0.035), "head", COMB)
    # Wattle under the beak.
    box((0, 0.285, 0.675), (0.03, 0.025, 0.06), "head", COMB)
    # Beak halves; the lower one hinges for the crow.
    cone((0, 0.285, 0.745), (0, 0.37, 0.735), 0.03, "head", BEAK, segments=4)
    cone((0, 0.285, 0.72), (0, 0.35, 0.715), 0.022, "beak_low", BEAK, segments=4)
    # Eyes.
    for side in (1, -1):
        box((side * 0.058, 0.25, 0.76), (0.02, 0.025, 0.025), "head", EYE)
    # Tail: a fan of long feathers, the middle ones greenish.
    for i, spread in enumerate((-0.6, -0.3, 0.0, 0.3, 0.6)):
        height = 0.36 - abs(spread) * 0.12
        material = SHEEN if i % 2 == 0 else FEATHER
        box((spread * 0.1, -0.25 - abs(spread) * 0.03, 0.55), (0.045, 0.05, height), "tail", material, rotate=(-0.4, 0.0, -spread * 0.35))
    # Wings, folded against the sides.
    for side, bone in ((1, "wing_L"), (-1, "wing_R")):
        box((side * 0.185, 0.0, 0.43), (0.035, 0.26, 0.17), bone, FEATHER, rotate=(0.12, 0, 0))
        box((side * 0.19, -0.1, 0.38), (0.03, 0.12, 0.08), bone, SHEEN, rotate=(0.2, 0, 0))
    # Legs: thin shanks and three-toed feet.
    for side, bone in ((1, "leg_L"), (-1, "leg_R")):
        x = side * 0.07
        box((x, 0.0, 0.17), (0.026, 0.026, 0.3), bone, LEG)
        box((x, 0.045, 0.012), (0.026, 0.11, 0.024), bone, LEG)
        box((x + side * 0.035, 0.035, 0.012), (0.024, 0.09, 0.024), bone, LEG, rotate=(0, 0, side * 0.35))
        box((x - side * 0.035, 0.035, 0.012), (0.024, 0.09, 0.024), bone, LEG, rotate=(0, 0, -side * 0.35))


build_body()
bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
mesh = bpy.data.meshes.new("rooster")
bm.to_mesh(mesh)
bm.free()
for m in MATERIALS:
    mesh.materials.append(m)
body = bpy.data.objects.new("rooster_mesh", mesh)
scene.collection.objects.link(body)
for name in BONES:
    body.vertex_groups.new(name=name)
for poly in mesh.polygons:
    poly.use_smooth = False
body.parent = arm_obj
mod = body.modifiers.new("Armature", "ARMATURE")
mod.object = arm_obj

# --------------------------------------------------------------------------------------------------
# Animation
# --------------------------------------------------------------------------------------------------
KEYED = list(BONES)


def reset_pose():
    for pb in arm_obj.pose.bones:
        pb.rotation_euler = (0.0, 0.0, 0.0)
        pb.location = (0.0, 0.0, 0.0)


def rot(name, x=0.0, y=0.0, z=0.0):
    pb = arm_obj.pose.bones[name]
    pb.rotation_euler = (pb.rotation_euler[0] + x, pb.rotation_euler[1] + y, pb.rotation_euler[2] + z)


def lift(name, z):
    arm_obj.pose.bones[name].location = (0.0, 0.0, z)


def bake(name, frames, pose_fn, loop=True):
    """One action: `pose_fn(phase)` poses the rig for phase 0..1. A looping clip's last key repeats its first."""
    act = bpy.data.actions.new(name)
    act.use_fake_user = True
    arm_obj.animation_data_create()
    arm_obj.animation_data.action = act
    scene.frame_start, scene.frame_end = 0, frames
    for f in range(frames + 1):
        reset_pose()
        pose_fn((f % frames) / frames if loop else f / frames)
        bpy.context.view_layer.update()
        for n in KEYED:
            pb = arm_obj.pose.bones[n]
            pb.keyframe_insert("rotation_euler", frame=f, group=n)
            pb.keyframe_insert("location", frame=f, group=n)


def smooth(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def idle(t):
    w = 2 * math.pi * t
    lift("body", math.sin(w) * 0.006)
    rot("body", x=math.sin(w) * 0.015)
    rot("tail", x=-math.sin(w) * 0.03)
    # A peck: the head dips and comes back between 55% and 80% of the loop, and a look around before it.
    peck = smooth((t - 0.55) / 0.1) * (1 - smooth((t - 0.7) / 0.1))
    rot("neck", x=-0.55 * peck)
    rot("head", x=-0.35 * peck)
    rot("body", x=-0.1 * peck)
    look = math.sin(2 * math.pi * min(1.0, t / 0.5)) * 0.35 * (1 if t < 0.5 else 0)
    rot("neck", z=look)
    rot("head", z=look * 0.5)


def walk(t):
    w = 2 * math.pi * t
    swing = 0.7
    rot("leg_L", x=math.sin(w) * swing)
    rot("leg_R", x=-math.sin(w) * swing)
    # A foot lifts as it comes forward: the leg is shortened by tucking it with the opposite phase.
    lift("body", abs(math.sin(w)) * 0.03)
    rot("body", y=math.sin(w) * 0.05, x=0.04)
    # The head bobs forward and back twice a stride, which is the walking-chicken look.
    rot("neck", x=math.sin(2 * w + 1.0) * 0.2)
    rot("head", x=-math.sin(2 * w + 1.0) * 0.15)
    rot("tail", x=math.sin(w * 2) * 0.05)


def crow(t):
    # In (0..0.2): chest up, head back; hold with the beak working; out (0.85..1).
    e = smooth(t / 0.2) * (1 - smooth((t - 0.85) / 0.15))
    pulses = max(0.0, math.sin(2 * math.pi * 3.0 * (t - 0.2) / 0.65)) if 0.2 <= t <= 0.85 else 0.0
    rot("body", x=0.35 * e)
    lift("body", 0.04 * e)
    rot("tail", x=-0.1 * e)
    rot("neck", x=0.2 * e)
    rot("head", x=0.4 * e - 0.1 * pulses)
    rot("beak_low", x=-0.5 * pulses - 0.15 * e)
    # Wings flick out on each call.
    rot("wing_L", y=-0.55 * pulses - 0.12 * e, z=0.15 * pulses)
    rot("wing_R", y=0.55 * pulses + 0.12 * e, z=-0.15 * pulses)
    # The legs straighten a little, up on its toes.
    rot("leg_L", x=-0.1 * e)
    rot("leg_R", x=-0.1 * e)


bake("idle", IDLE_FRAMES, idle)
bake("walk", WALK_FRAMES, walk)
bake("crow", CROW_FRAMES, crow, loop=False)
reset_pose()


# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
def export():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    arm_obj.select_set(True)
    body.select_set(True)
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
        export_cameras=False,
        export_lights=False,
        export_extras=False,
    )
    valid = bpy.ops.export_scene.gltf.get_rna_type().properties.keys()
    bpy.ops.export_scene.gltf(**{k: v for k, v in kwargs.items() if k in valid})


export()
print("tris:", sum(len(p.vertices) - 2 for p in mesh.polygons))
print("bones:", len(arm_data.bones))
print("actions:", sorted(a.name for a in bpy.data.actions))
print("wrote", OUT)


# --------------------------------------------------------------------------------------------------
# Preview renders (optional)
# --------------------------------------------------------------------------------------------------
def preview(directory):
    os.makedirs(directory, exist_ok=True)
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "MATERIAL"
    scene.render.resolution_x, scene.render.resolution_y = 480, 480
    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 1.4
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "side": (Vector((6, 0, 0.45)), Vector((0, 0, 0.45))),
        "front": (Vector((0, 6, 0.45)), Vector((0, 0, 0.45))),
        "three": (Vector((3.5, 4.5, 2.4)), Vector((0, 0, 0.42))),
    }

    def shoot(name, view):
        pos, target = views[view]
        cam.location = pos
        cam.rotation_euler = (target - pos).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, f"{name}.png")
        bpy.ops.render.render(write_still=True)

    def at(action, frame):
        arm_obj.animation_data.action = bpy.data.actions[action]
        scene.frame_set(frame)

    at("idle", 0)
    for v in views:
        shoot(f"idle_{v}", v)
    at("idle", 30)
    shoot("idle_peck", "side")
    for i in range(3):
        at("walk", i * WALK_FRAMES // 3)
        shoot(f"walk_{i}", "side")
    for frame in (0, 10, 18, 26, 49):
        at("crow", frame)
        shoot(f"crow_{frame}", "side")
    at("crow", 18)
    shoot("crow_three", "three")


if PREVIEW:
    preview(PREVIEW)
