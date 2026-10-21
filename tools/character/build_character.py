"""
Builds the player character and exports it to client/public/models/player.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/character/build_character.py -- \
        [--out client/public/models/player.glb] [--preview DIR]

Everything is procedural so the art is reproducible: a low-poly, flat-shaded humanoid, a ~20 bone
rig, one baked action per movement clip per stance, and the weapons as separate meshes on the right
hand that the game shows or hides.

Conventions
  * Blender: Z up, the character faces -Y, its right hand side is -X. The glTF exporter turns that into
    Y up, facing +Z (the renderer spins it by PI so it faces the game's -Z).
  * Origin is at the feet. Height is about 1.72 m, matching the 1.7 m capsule.
  * Two materials, so two draw calls per player: `outfit` (vertex colours are shades of white, tinted
    per player at runtime) and `gear` (skin, boots; vertex colours are the final colours). Each weapon
    is its own mesh, `weapon_<name>`, skinned to the right hand and made in the pose it is held in.
  * Stances say how the arms hold things: `rifle` (both hands on the gun), `pistol` (both hands, out in front) and
    `hold` (hands low, in front). Clips are baked per stance: the rifle's are named plainly (`idle`,
    `walk_fwd`, ...), the others carry a `pistol_` / `hold_` prefix. `death` is shared.
  * Clips: idle, walk_fwd/back/left/right, run_fwd/back/left/right, crouch_idle, crouch_fwd/back/left/
    right, crawl_idle, crawl_fwd/back/left/right, slide, jump_air, sit, death. The crawl is an army
    crawl lying face down along -Y, resting on the ground at the origin's height. `sit` is a seat in a
    vehicle: hips dropped by `SIT_DROP`, thighs level and shins down, so the head ends up `SIT_HEAD`
    above the origin (the client places a seated player by it). Clips loop with a duplicated last key so the mixer wraps without a hitch.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Euler, Matrix, Quaternion, Vector

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def arg(name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/player.glb")))
PREVIEW = arg("--preview")

FPS = 60

# --------------------------------------------------------------------------------------------------
# Dimensions (metres, Blender axes: x = character's left, -y = forward, z = up)
# --------------------------------------------------------------------------------------------------
HIP_Z = 0.90
KNEE_Z = 0.48
ANKLE_Z = 0.10
LEG_X = 0.11
SPINE_Z = 1.00
CHEST_Z = 1.18
SHOULDER_Z = 1.40
NECK_Z = 1.46
HEAD_Z = 1.52
SHOULDER_X = 0.25
UPPER_ARM = 0.34
FOREARM = 0.32
HAND_LEN = 0.10

# Where the right hand grips the rifle and where the left hand supports it, armature space.
GRIP = Vector((-0.15, -0.24, 1.04))
FOREGRIP = Vector((-0.15, -0.44, 1.07))

# Where each hand goes in each stance (wrist target) and which way the elbow points. The right hand
# is the one that holds the item; the muzzle is measured from `GRIP_OF[stance]`.
STANCES = {
    "rifle": dict(R=GRIP, L=FOREGRIP, pole_r=Vector((-0.6, 0.35, -1.0)), pole_l=Vector((0.8, 0.25, -1.0))),
    # The revolver in both hands, out in front of the chest: the left hand cups the right from below.
    "pistol": dict(
        R=Vector((-0.07, -0.42, 1.2)), L=Vector((-0.02, -0.38, 1.15)),
        pole_r=Vector((-0.5, 0.2, -1.0)), pole_l=Vector((0.6, 0.2, -1.0)),
    ),
    # Hands low and in front, as when holding a grenade or winding a bandage.
    "hold": dict(
        R=Vector((-0.19, -0.30, 1.05)), L=Vector((0.21, -0.22, 1.03)),
        pole_r=Vector((-0.7, 0.2, -1.0)), pole_l=Vector((0.7, 0.2, -1.0)),
    ),
}
STANCE_PREFIX = {"rifle": "", "pistol": "pistol_", "hold": "hold_"}


def srgb(h):
    """Hex colour to linear rgba."""
    out = []
    for shift in (16, 8, 0):
        c = ((h >> shift) & 0xFF) / 255.0
        out.append(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4)
    return (*out, 1.0)


def grey(v):
    return (v, v, v, 1.0)


# Outfit shades are multiplied by the player's colour at runtime.
OUTFIT_TOP = grey(1.0)
OUTFIT_PANTS = grey(0.55)
OUTFIT_TRIM = grey(0.78)
SKIN = srgb(0xE8B58F)
DARK = srgb(0x2A2C31)
BOOT = srgb(0x1B1C20)
GUN_BODY = srgb(0x3B3F47)
GUN_TRIM = srgb(0xF2A33A)
WOOD = srgb(0x7A4A2A)
OLIVE = srgb(0x4F5D2E)
CLOTH = srgb(0xF2F2EE)
CROSS = srgb(0xD93A3A)
VISOR = srgb(0x7FD8F0)

# --------------------------------------------------------------------------------------------------
# Scene and armature
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.fps = FPS

arm_data = bpy.data.armatures.new("Armature")
arm_obj = bpy.data.objects.new("Armature", arm_data)
scene.collection.objects.link(arm_obj)
bpy.context.view_layer.objects.active = arm_obj
bpy.ops.object.mode_set(mode="EDIT")

# name: (parent, head, tail)
BONES = {}


def bone(name, parent, head, tail):
    BONES[name] = (parent, Vector(head), Vector(tail))


bone("root", None, (0, 0, 0), (0, 0, 0.15))
bone("hips", "root", (0, 0, HIP_Z), (0, 0, SPINE_Z))
bone("spine", "hips", (0, 0, SPINE_Z), (0, 0, CHEST_Z))
bone("chest", "spine", (0, 0, CHEST_Z), (0, 0, NECK_Z))
bone("neck", "chest", (0, 0, NECK_Z), (0, 0, HEAD_Z))
bone("head", "neck", (0, 0, HEAD_Z), (0, 0, HEAD_Z + 0.25))
for side, sx in (("L", 1), ("R", -1)):
    bone(f"thigh_{side}", "hips", (sx * LEG_X, 0, HIP_Z), (sx * LEG_X, 0, KNEE_Z))
    bone(f"shin_{side}", f"thigh_{side}", (sx * LEG_X, 0, KNEE_Z), (sx * LEG_X, 0, ANKLE_Z))
    bone(f"foot_{side}", f"shin_{side}", (sx * LEG_X, 0, ANKLE_Z), (sx * LEG_X, -0.2, 0.04))
    sh = (sx * SHOULDER_X, 0, SHOULDER_Z)
    el = (sx * SHOULDER_X, 0, SHOULDER_Z - UPPER_ARM)
    wr = (sx * SHOULDER_X, 0, SHOULDER_Z - UPPER_ARM - FOREARM)
    bone(f"upper_arm_{side}", "chest", sh, el)
    bone(f"forearm_{side}", f"upper_arm_{side}", el, wr)
    bone(f"hand_{side}", f"forearm_{side}", wr, (wr[0], wr[1], wr[2] - HAND_LEN))


def make_bones():
    for name, (parent, head, tail) in BONES.items():
        eb = arm_data.edit_bones.new(name)
        eb.head, eb.tail = head, tail
        eb.roll = 0.0
        if parent:
            eb.parent = arm_data.edit_bones[parent]
            eb.use_connect = False


make_bones()
bpy.ops.object.mode_set(mode="OBJECT")

# Rest orientation of every bone, used to turn "rotate about world X" into the bone's own Euler.
REST3 = {b.name: b.matrix_local.to_3x3() for b in arm_data.bones}
for pb in arm_obj.pose.bones:
    pb.rotation_mode = "XYZ"


def local_euler(name, world_rotation):
    """Bone-local Euler for a rotation given in armature axes, applied after the parent's delta."""
    r = REST3[name]
    return (r.inverted() @ world_rotation @ r).to_euler("XYZ")


def pose_rot(name, wx=0.0, wy=0.0, wz=0.0):
    m = Euler((wx, wy, wz), "XYZ").to_matrix()
    arm_obj.pose.bones[name].rotation_euler = local_euler(name, m)


def pose_loc(name, world_offset):
    arm_obj.pose.bones[name].location = REST3[name].inverted() @ Vector(world_offset)


def reset_pose():
    for pb in arm_obj.pose.bones:
        pb.rotation_euler = (0, 0, 0)
        pb.location = (0, 0, 0)


# --------------------------------------------------------------------------------------------------
# Carry pose: both hands on the rifle, solved with a two bone IK so the hands really meet the gun.
# --------------------------------------------------------------------------------------------------
def solve_arm(side, target, pole):
    """Aims upper arm and forearm so the wrist reaches `target`. Returns world deltas (upper, fore)."""
    shoulder = BONES[f"upper_arm_{side}"][1]
    to_target = target - shoulder
    reach = min(to_target.length, (UPPER_ARM + FOREARM) * 0.985)
    direction = to_target.normalized()
    # Distance along the line to the elbow's foot, and how far the elbow stands off the line.
    a = (UPPER_ARM**2 - FOREARM**2 + reach**2) / (2 * reach)
    h = math.sqrt(max(UPPER_ARM**2 - a**2, 0.0))
    side_vec = pole - direction * pole.dot(direction)
    side_vec.normalize()
    elbow = shoulder + direction * a + side_vec * h
    wrist = shoulder + direction * reach
    down = Vector((0, 0, -1))
    upper = down.rotation_difference((elbow - shoulder).normalized()).to_matrix()
    fore = down.rotation_difference((wrist - elbow).normalized()).to_matrix()
    return upper, fore


def apply_chain(side, upper_world, fore_world, hand_world):
    arm_obj.pose.bones[f"upper_arm_{side}"].rotation_euler = local_euler(f"upper_arm_{side}", upper_world)
    arm_obj.pose.bones[f"forearm_{side}"].rotation_euler = local_euler(
        f"forearm_{side}", upper_world.inverted() @ fore_world
    )
    arm_obj.pose.bones[f"hand_{side}"].rotation_euler = local_euler(
        f"hand_{side}", fore_world.inverted() @ hand_world
    )


def carry_pose(stance="rifle"):
    """Both arms for `stance`: the hands go where the stance puts them, elbows out and down."""
    t = STANCES[stance]
    upper, fore = solve_arm("R", t["R"], t["pole_r"])
    apply_chain("R", upper, fore, fore)
    upper, fore = solve_arm("L", t["L"], t["pole_l"])
    apply_chain("L", upper, fore, fore)


# Each stance's arm rotations, captured so every clip can start from them.
CARRY = {}
# The stance clips are being baked for.
CURRENT = "rifle"


def capture_carry():
    for stance in STANCES:
        reset_pose()
        carry_pose(stance)
        CARRY[stance] = {}
        for n in ("R", "L"):
            for part in ("upper_arm", "forearm", "hand"):
                pb = arm_obj.pose.bones[f"{part}_{n}"]
                CARRY[stance][pb.name] = pb.rotation_euler.copy()


capture_carry()
bpy.context.view_layer.update()


def skin_matrix(name):
    pb = arm_obj.pose.bones[name]
    return pb.matrix @ arm_data.bones[name].matrix_local.inverted()


# --------------------------------------------------------------------------------------------------
# Mesh
# --------------------------------------------------------------------------------------------------
bm = bmesh.new()
deform = bm.verts.layers.deform.verify()
colors = bm.loops.layers.float_color.new("Color")
GROUPS = {}


def start_mesh():
    """Starts a new mesh; `box`, `prism` and `blob` build into it."""
    global bm, deform, colors
    bm = bmesh.new()
    deform = bm.verts.layers.deform.verify()
    colors = bm.loops.layers.float_color.new("Color")


def group_index(name):
    if name not in GROUPS:
        GROUPS[name] = len(GROUPS)
    return GROUPS[name]


MAT_OUTFIT, MAT_GEAR = 0, 1


def finish(verts, bone_name, material, color, transform=None):
    """Binds freshly created verts rigidly to a bone and paints their faces."""
    gi = group_index(bone_name)
    faces = set()
    for v in verts:
        if transform is not None:
            v.co = transform @ v.co
        v[deform][gi] = 1.0
        faces.update(v.link_faces)
    for f in faces:
        f.material_index = material
        f.smooth = False
        for loop in f.loops:
            loop[colors] = color


def box(center, size, bone_name, material, color, taper=None, transform=None):
    """Axis-aligned box. `taper` = (sx, sy) scales the top face's cross-section."""
    res = bmesh.ops.create_cube(bm, size=1.0)
    verts = res["verts"]
    for v in verts:
        top = v.co.z > 0
        x, y, z = v.co.x * size[0], v.co.y * size[1], v.co.z * size[2]
        if taper and top:
            x *= taper[0]
            y *= taper[1]
        v.co = Vector((x, y, z)) + Vector(center)
    finish(verts, bone_name, material, color, transform)
    return verts


def prism(p0, p1, r0, r1, bone_name, material, color, sides=6, transform=None):
    """Tapered prism from p0 to p1."""
    p0, p1 = Vector(p0), Vector(p1)
    axis = p1 - p0
    rot = Vector((0, 0, 1)).rotation_difference(axis.normalized()).to_matrix().to_4x4()
    m = Matrix.Translation((p0 + p1) / 2) @ rot
    res = bmesh.ops.create_cone(
        bm, cap_ends=True, cap_tris=False, segments=sides, radius1=r0, radius2=r1, depth=axis.length, matrix=m
    )
    finish(res["verts"], bone_name, material, color, transform)
    return res["verts"]


def blob(center, radii, bone_name, material, color, subdivisions=1, transform=None):
    res = bmesh.ops.create_icosphere(bm, subdivisions=subdivisions, radius=1.0)
    for v in res["verts"]:
        v.co = Vector((v.co.x * radii[0], v.co.y * radii[1], v.co.z * radii[2])) + Vector(center)
    finish(res["verts"], bone_name, material, color, transform)
    return res["verts"]


def build_body():
    # Pelvis and torso: a belted hip block under a wider chest, wide at the shoulders.
    box((0, 0, 0.93), (0.30, 0.20, 0.16), "hips", MAT_OUTFIT, OUTFIT_PANTS)
    box((0, 0, 1.085), (0.30, 0.19, 0.18), "spine", MAT_OUTFIT, OUTFIT_TOP, taper=(1.05, 1.0))
    box((0, 0, 1.27), (0.37, 0.22, 0.28), "chest", MAT_OUTFIT, OUTFIT_TOP, taper=(1.15, 1.0))
    # Belt, and a backpack.
    box((0, 0, 1.01), (0.32, 0.215, 0.05), "spine", MAT_GEAR, DARK)
    box((0, 0.185, 1.25), (0.26, 0.15, 0.30), "chest", MAT_GEAR, DARK, taper=(0.9, 0.9))
    # Neck, head, helmet and goggles.
    prism((0, 0, 1.43), (0, 0, 1.53), 0.055, 0.05, "neck", MAT_GEAR, SKIN)
    blob((0, 0, 1.62), (0.155, 0.165, 0.17), "head", MAT_GEAR, SKIN)
    blob((0, 0.02, 1.665), (0.178, 0.18, 0.155), "head", MAT_OUTFIT, OUTFIT_TRIM)
    box((0, -0.15, 1.635), (0.27, 0.06, 0.075), "head", MAT_GEAR, VISOR)
    box((0, -0.13, 1.575), (0.24, 0.10, 0.06), "head", MAT_GEAR, SKIN)
    for side, sx in (("L", 1), ("R", -1)):
        # Legs: thigh, shin, boot.
        prism((sx * LEG_X, 0, HIP_Z - 0.02), (sx * LEG_X, 0, KNEE_Z), 0.098, 0.075, f"thigh_{side}", MAT_OUTFIT, OUTFIT_PANTS)
        prism((sx * LEG_X, 0, KNEE_Z), (sx * LEG_X, 0, ANKLE_Z + 0.04), 0.078, 0.062, f"shin_{side}", MAT_OUTFIT, OUTFIT_PANTS)
        box((sx * LEG_X, -0.045, 0.06), (0.125, 0.28, 0.13), f"foot_{side}", MAT_GEAR, BOOT, taper=(0.9, 0.8))
        # Arms: shoulder pad, sleeve, forearm, glove.
        sx_ = sx * SHOULDER_X
        blob((sx_, 0, SHOULDER_Z - 0.03), (0.095, 0.095, 0.095), f"upper_arm_{side}", MAT_OUTFIT, OUTFIT_TRIM)
        prism((sx_, 0, SHOULDER_Z - 0.02), (sx_, 0, SHOULDER_Z - UPPER_ARM), 0.07, 0.058, f"upper_arm_{side}", MAT_OUTFIT, OUTFIT_TOP)
        prism((sx_, 0, SHOULDER_Z - UPPER_ARM), (sx_, 0, SHOULDER_Z - UPPER_ARM - FOREARM + 0.03), 0.058, 0.048, f"forearm_{side}", MAT_OUTFIT, OUTFIT_TOP)
        wz = SHOULDER_Z - UPPER_ARM - FOREARM
        box((sx_, 0, wz - 0.035), (0.085, 0.10, 0.12), f"hand_{side}", MAT_GEAR, DARK)


def pose_to_rest(stance):
    """Poses the rig in `stance` and returns the matrix that un-poses points onto the right hand bone."""
    reset_pose()
    carry_pose(stance)
    bpy.context.view_layer.update()
    return skin_matrix("hand_R").inverted()


def build_weapon(parts, stance):
    """`parts`: (kind, centre offset from the grip, size, colour) in the stance's pose space."""
    rest_from_pose = pose_to_rest(stance)
    g = STANCES[stance]["R"]
    for kind, off, size, color in parts:
        if kind == "box":
            box(g + Vector(off), size, "hand_R", MAT_GEAR, color, transform=rest_from_pose)
        else:
            blob(g + Vector(off), size, "hand_R", MAT_GEAR, color, transform=rest_from_pose)


# (name, stance, parts, muzzle offset from the grip or None)
WEAPONS = [
    (
        "ak5",
        "rifle",
        [
            ("box", (0, -0.20, 0.035), (0.055, 0.46, 0.085), GUN_BODY),  # receiver / handguard
            ("box", (0, 0.12, 0.02), (0.05, 0.20, 0.09), GUN_BODY),  # stock
            ("box", (0, 0.0, -0.065), (0.04, 0.05, 0.11), GUN_TRIM),  # pistol grip
            ("box", (0, -0.20, -0.055), (0.04, 0.07, 0.12), GUN_TRIM),  # magazine
            ("box", (0, -0.40, 0.04), (0.025, 0.28, 0.025), DARK),  # barrel
            ("box", (0, -0.18, 0.105), (0.04, 0.16, 0.04), DARK),  # scope
        ],
        (0, -0.56, 0.04),
    ),
    (
        "sniper",
        "rifle",
        [
            ("box", (0, -0.22, 0.035), (0.045, 0.52, 0.08), GUN_BODY),  # receiver / handguard
            ("box", (0, 0.13, 0.02), (0.05, 0.22, 0.095), WOOD),  # stock
            ("box", (0, 0.0, -0.065), (0.04, 0.05, 0.11), GUN_TRIM),  # pistol grip
            ("box", (0, -0.14, -0.04), (0.035, 0.06, 0.07), GUN_TRIM),  # magazine
            ("box", (0, -0.45, 0.04), (0.02, 0.22, 0.02), DARK),  # barrel
            ("box", (0, -0.18, 0.115), (0.05, 0.30, 0.05), DARK),  # scope
            ("box", (0, -0.36, 0.115), (0.07, 0.04, 0.07), DARK),  # scope lens hood
        ],
        None,  # shares the rifle's muzzle bone
    ),
    (
        "revolver",
        "pistol",
        [
            ("box", (0, -0.13, 0.035), (0.035, 0.24, 0.04), GUN_BODY),  # barrel
            ("box", (0, -0.03, 0.03), (0.05, 0.07, 0.065), GUN_TRIM),  # cylinder
            ("box", (0, 0.0, 0.0), (0.04, 0.10, 0.05), GUN_BODY),  # frame
            ("box", (0, 0.04, -0.065), (0.04, 0.05, 0.12), WOOD),  # grip
            ("box", (0, -0.25, 0.06), (0.015, 0.02, 0.02), DARK),  # sight
        ],
        (0, -0.26, 0.035),
    ),
    (
        "grenade",
        "hold",
        [
            ("blob", (0, -0.03, 0.0), (0.055, 0.055, 0.065), OLIVE),
            ("box", (0, -0.03, 0.065), (0.03, 0.03, 0.03), GUN_BODY),
            ("box", (0.03, -0.03, 0.06), (0.012, 0.02, 0.07), GUN_TRIM),
        ],
        None,
    ),
    (
        "bandage",
        "hold",
        [
            ("box", (0, -0.03, 0.0), (0.12, 0.07, 0.06), CLOTH),
            ("box", (0, -0.03, 0.0), (0.075, 0.075, 0.02), CROSS),
            ("box", (0, -0.03, 0.0), (0.02, 0.075, 0.065), CROSS),
        ],
        None,
    ),
]

build_body()

outfit_mat = bpy.data.materials.new("outfit")
gear_mat = bpy.data.materials.new("gear")
for m in (outfit_mat, gear_mat):
    m.use_nodes = True
    nodes = m.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 1.0
    bsdf.inputs["Metallic"].default_value = 0.0
    attr = nodes.new("ShaderNodeVertexColor")
    attr.layer_name = "Color"
    m.node_tree.links.new(attr.outputs["Color"], bsdf.inputs["Base Color"])


def make_object(name, builder):
    """Turns what `builder` put in the current bmesh into an armature-skinned object."""
    mesh_ = bpy.data.meshes.new(name)
    builder.to_mesh(mesh_)
    builder.free()
    mesh_.attributes.active_color = mesh_.color_attributes["Color"]
    mesh_.materials.append(outfit_mat)
    mesh_.materials.append(gear_mat)
    obj = bpy.data.objects.new(name, mesh_)
    scene.collection.objects.link(obj)
    for gname in GROUPS:
        obj.vertex_groups.new(name=gname)
    mod_ = obj.modifiers.new("Armature", "ARMATURE")
    mod_.object = arm_obj
    obj.parent = arm_obj
    return obj, mesh_


body, mesh = make_object("Player", bm)
for gname, index in GROUPS.items():
    assert body.vertex_groups[gname].index == index

# Weapons: one mesh each, in the pose it is held in, and the muzzle where its barrel ends.
weapon_objects = []
MUZZLES = {}  # bone name -> (stance, tip in rest space)
for wname, wstance, parts, muzzle in WEAPONS:
    start_mesh()
    build_weapon(parts, wstance)
    obj, _ = make_object(f"weapon_{wname}", bm)
    weapon_objects.append(obj)
    if muzzle is not None:
        # The tip is found here, with the rig posed in this stance; edit mode does not re-evaluate the pose.
        tip = pose_to_rest(wstance) @ (STANCES[wstance]["R"] + Vector(muzzle))
        MUZZLES["muzzle" if wstance == "rifle" else f"muzzle_{wstance}"] = (wstance, tip)

# Muzzle bones ride on the right hand, at the barrel tips.
bpy.context.view_layer.objects.active = arm_obj
bpy.ops.object.mode_set(mode="EDIT")
for bname, (wstance, tip) in MUZZLES.items():
    eb = arm_data.edit_bones.new(bname)
    eb.head = tip
    eb.tail = tip + Vector((0, 0, -0.05))
    eb.parent = arm_data.edit_bones["hand_R"]
bpy.ops.object.mode_set(mode="OBJECT")
for bname in MUZZLES:
    arm_obj.pose.bones[bname].rotation_mode = "XYZ"
    REST3[bname] = arm_data.bones[bname].matrix_local.to_3x3()

# --------------------------------------------------------------------------------------------------
# Animation
# --------------------------------------------------------------------------------------------------
KEYED = [b for b in BONES]  # muzzle is never animated by hand: it follows the hand.
TAU = 2 * math.pi
SOLO_BONES = KEYED


def set_arms_carry():
    for name, e in CARRY[CURRENT].items():
        arm_obj.pose.bones[name].rotation_euler = e.copy()


def new_action(name):
    act = bpy.data.actions.new(name)
    act.use_fake_user = True
    arm_obj.animation_data_create()
    arm_obj.animation_data.action = act
    return act


def key_frame(frame):
    for n in KEYED:
        pb = arm_obj.pose.bones[n]
        pb.keyframe_insert("rotation_euler", frame=frame, group=n)
        if n in ("root", "hips"):
            pb.keyframe_insert("location", frame=frame, group=n)


def bake(name, frames, pose_fn, loop=True):
    """One action: `pose_fn(phase)` poses the rig for phase 0..1. Looping clips repeat the first key last."""
    new_action(name)
    scene.frame_start, scene.frame_end = 0, frames
    for f in range(frames + 1):
        reset_pose()
        set_arms_carry()
        pose_fn((f % frames) / frames if loop else f / frames)
        bpy.context.view_layer.update()
        key_frame(f)


def gait(direction, amp, lat_amp, knee, lean, bob, cycle_sway):
    """Walk/run cycle travelling along `direction` = (right, forward) in the body frame."""
    dr, df = direction

    def fn(t):
        phi = TAU * t
        for side, offset in (("L", 0.0), ("R", math.pi)):
            p = phi + offset
            s = math.sin(p)
            lift = max(0.0, math.cos(p))
            # Foot swings along the travel direction, so a strafe steps sideways with the same cycle.
            thigh_x = -amp * s * df + 0.05
            # A constant spread keeps a sidestep from crossing the legs over each other.
            spread = (0.06 + 0.3 * abs(dr)) * (-1.0 if side == "L" else 1.0)
            thigh_y = lat_amp * s * dr + spread
            shin_x = knee * lift ** 1.2 + 0.08
            pose_rot(f"thigh_{side}", thigh_x, thigh_y, 0)
            pose_rot(f"shin_{side}", shin_x, 0, 0)
            foot = -(thigh_x + shin_x) * 0.85
            pose_rot(f"foot_{side}", foot, 0, 0)
        up = bob * (math.cos(2 * phi) - 1.0)
        pose_loc("hips", (cycle_sway * math.sin(phi) * 0.5, 0, up - 0.02))
        pose_rot("hips", -lean * 0.4 - 0.04, 0, 0.05 * math.sin(phi) * (abs(df) + 0.2))
        pose_rot("spine", -lean * 0.3 + 0.02, 0, -0.04 * math.sin(phi))
        pose_rot("chest", -lean * 0.3 + 0.01 * math.cos(2 * phi), 0, -0.03 * math.sin(phi))
        pose_rot("head", lean * 0.7, 0, 0.02 * math.sin(phi))

    return fn


def idle(t):
    phi = TAU * t
    breath = math.sin(phi)
    pose_loc("hips", (0, 0, -0.015 + 0.006 * breath))
    pose_rot("hips", -0.03, 0.0, 0.015 * math.sin(phi))
    pose_rot("spine", 0.01 + 0.01 * breath, 0, -0.01 * math.sin(phi))
    pose_rot("chest", 0.01 + 0.02 * breath, 0, 0)
    pose_rot("head", -0.02 - 0.015 * breath, 0.0, 0)
    for side in ("L", "R"):
        pose_rot(f"thigh_{side}", 0.05, 0.06 if side == "L" else -0.06, 0)
        pose_rot(f"shin_{side}", 0.08, 0, 0)
        pose_rot(f"foot_{side}", -0.11, 0, 0)


def jump_air(t):
    phi = TAU * t
    wob = 0.04 * math.sin(phi)
    pose_loc("hips", (0, 0, 0.0))
    pose_rot("hips", 0.05, 0, 0)
    pose_rot("spine", -0.05, 0, 0)
    pose_rot("chest", -0.05 + wob, 0, 0)
    pose_rot("head", 0.08, 0, 0)
    pose_rot("thigh_L", -0.7 + wob, 0.1, 0)
    pose_rot("shin_L", 1.0, 0, 0)
    pose_rot("foot_L", -0.2, 0, 0)
    pose_rot("thigh_R", -0.1 - wob, -0.1, 0)
    pose_rot("shin_R", 0.5, 0, 0)
    pose_rot("foot_R", 0.2, 0, 0)


def death(t):
    # Staggers, then goes over backwards about the feet (the root), ending flat on the back.
    e = t * t * (3 - 2 * t)
    pose_rot("root", -e * (math.pi / 2), 0, 0)
    pose_loc("root", (0, 0, 0.13 * e))
    kneel = math.sin(min(1.0, t * 1.6) * math.pi * 0.5)
    for side, s in (("L", 1), ("R", -1)):
        pose_rot(f"thigh_{side}", -0.25 * kneel, s * 0.12 * e, 0)
        pose_rot(f"shin_{side}", 0.45 * kneel, 0, 0)
        pose_rot(f"foot_{side}", 0.3 * e, 0, 0)
    pose_rot("spine", 0.12 * e, 0, 0)
    pose_rot("chest", 0.1 * e, 0, 0)
    pose_rot("head", 0.25 * e, 0, 0.1 * e)
    # Arms fling out and the gun hand goes limp: blend the carry pose away.
    for side, s in (("L", 1), ("R", -1)):
        pose_rot(f"upper_arm_{side}", -0.5 * e, s * -0.2 * e - 0.0, 0)
        pose_rot(f"forearm_{side}", -0.3 * e, 0, 0)
        pose_rot(f"hand_{side}", 0, 0, 0)
        # Replace the carry pose: pose_rot overwrote it only on the bones set above.


CROUCH_DROP = 0.46


def crouch_pose(phi, sway=0.0):
    """Knees and hips bent, body low and tucked behind the rifle. `phi` drives a small breath."""
    breath = math.sin(phi)
    pose_loc("hips", (sway, 0.0, -CROUCH_DROP + 0.006 * breath))
    pose_rot("hips", -0.3, 0, 0)
    pose_rot("spine", 0.18, 0, 0)
    pose_rot("chest", 0.12 + 0.015 * breath, 0, 0)
    pose_rot("head", 0.2, 0, 0)


def crouch_gait(direction, amp, lat_amp, lift_amount):
    """Low shuffle along `direction` = (right, forward): short steps, knees staying bent."""
    dr, df = direction

    def fn(t):
        phi = TAU * t
        crouch_pose(phi, 0.02 * math.sin(phi))
        for side, offset in (("L", 0.0), ("R", math.pi)):
            p = phi + offset
            s = math.sin(p)
            lift = max(0.0, math.cos(p))
            thigh_x = -1.25 - amp * s * df
            spread = (0.08 + 0.25 * abs(dr)) * (-1.0 if side == "L" else 1.0)
            thigh_y = lat_amp * s * dr + spread
            shin_x = 2.3 + lift_amount * lift ** 1.2
            pose_rot(f"thigh_{side}", thigh_x, thigh_y, 0)
            pose_rot(f"shin_{side}", shin_x, 0, 0)
            pose_rot(f"foot_{side}", -(thigh_x + shin_x) * 0.9, 0, 0)

    return fn


def crouch_idle(t):
    crouch_pose(TAU * t)
    for side in ("L", "R"):
        pose_rot(f"thigh_{side}", -1.25, 0.08 if side == "R" else -0.08, 0)
        pose_rot(f"shin_{side}", 2.3, 0, 0)
        pose_rot(f"foot_{side}", -0.95, 0, 0)


# Lying face down: the root turns a quarter about the feet so the body lies along the ground, head
# forward (-Y). In the lying body's own axes X is still its left, -Y its belly and Z towards its head.
# The body is then lowered (by the root) until it rests on the floor, measured on the mesh itself.
REST4 = {b.name: b.matrix_local.copy() for b in arm_data.bones}
# Body vertices, by the bone each is bound to, for finding the floor. Arms are left out: they reach
# for the ground themselves.
_GROUP = {g.index: g.name for g in body.vertex_groups}
TRUNK = [
    (_GROUP[v.groups[0].group], v.co.copy())
    for i, v in enumerate(mesh.vertices)
    if v.groups and i % 2 == 0 and not _GROUP[v.groups[0].group].startswith(("upper_arm", "forearm", "hand"))
]
# How far the chest lifts off the ground to look ahead.
PRONE_ARCH = 0.22


def pose_mat(name, world_rotation):
    """Like `pose_rot`, with the rotation given as a matrix in armature axes."""
    arm_obj.pose.bones[name].rotation_euler = local_euler(name, world_rotation)


def settle():
    """Lowers the body by the root until its lowest point rests on the ground."""
    pose_loc("root", (0, 0, 0))
    bpy.context.view_layer.update()
    mats = {}
    low = math.inf
    for b, co in TRUNK:
        m = mats.get(b)
        if m is None:
            m = mats[b] = arm_obj.pose.bones[b].matrix @ REST4[b].inverted()
        low = min(low, (m @ co).z)
    pose_loc("root", (0, 0, -low))
    bpy.context.view_layer.update()


def lie(breath=0.0, arch=PRONE_ARCH, yaw=0.0):
    pose_mat("root", Matrix.Rotation(yaw, 3, "Z") @ Matrix.Rotation(math.pi / 2, 3, "X"))
    pose_rot("hips", -0.05, 0, 0)
    pose_rot("spine", -arch * 0.5 - 0.01 * breath, 0, 0)
    pose_rot("chest", -arch * 0.7, 0, 0)
    pose_rot("neck", -0.3, 0, 0)
    pose_rot("head", -0.35, 0, 0)


def frog_leg(side, draw):
    """One leg lying: `draw` 0 is stretched out behind, 1 is the knee pulled up beside the hip along
    the ground. The leg is turned out, so the inside of it lies on the floor and the knee bends flat."""
    sx = 1.0 if side == "L" else -1.0
    pose_mat(
        f"thigh_{side}",
        Matrix.Rotation(-sx * (0.12 + 0.95 * draw), 3, "Y") @ Matrix.Rotation(sx * (1.5 + 0.1 * draw), 3, "Z") @ Matrix.Rotation(-0.14, 3, "X"),
    )
    pose_rot(f"shin_{side}", 0.15 + 1.75 * draw, 0, 0)
    # The foot lies on its inside edge, toes out.
    pose_rot(f"foot_{side}", 0.1, 0, 0)


def ease(t):
    t = min(max(t, 0.0), 1.0)
    return t * t * (3 - 2 * t)


def drawn(p):
    """How far a leg is drawn up at its phase `p`: it pushes back slowly, rests, then is pulled up."""
    p %= 1.0
    if p < 0.6:
        return 1.0 - ease(p / 0.6)
    if p < 0.72:
        return 0.0
    return ease((p - 0.72) / 0.28)


# Each stance's hands as the carry pose holds them: the guns are modelled there, so lying down the
# hands keep these turns and the gun still points ahead.
CARRY_HAND = {}
for _stance in STANCES:
    reset_pose()
    carry_pose(_stance)
    bpy.context.view_layer.update()
    CARRY_HAND[_stance] = {n: (arm_obj.pose.bones[f"hand_{n}"].matrix.to_3x3(), arm_obj.pose.bones[f"hand_{n}"].head.copy()) for n in ("R", "L")}
reset_pose()
# A flat hand on the ground, fingers ahead, palm down.
FLAT_HAND = {
    "L": Matrix.Rotation(-math.pi / 2, 3, "Y") @ Matrix.Rotation(-math.pi / 2, 3, "X"),
    "R": Matrix.Rotation(math.pi / 2, 3, "Y") @ Matrix.Rotation(-math.pi / 2, 3, "X"),
}


def reach(side, wrist, hand, pole):
    """Two-bone IK on the body as it is posed now: the wrist to `wrist`, the hand turned `hand` and the
    elbow towards `pole`, all in armature space."""
    upper, fore, hand_bone = f"upper_arm_{side}", f"forearm_{side}", f"hand_{side}"
    bpy.context.view_layer.update()
    chest = arm_obj.pose.bones["chest"].matrix
    free = chest @ REST4["chest"].inverted() @ REST4[upper]
    root = free.to_translation()
    to = wrist - root
    length = min(max(to.length, 0.05), (UPPER_ARM + FOREARM) * 0.999)
    d = to.normalized()
    a = (UPPER_ARM**2 - FOREARM**2 + length**2) / (2 * length)
    h = math.sqrt(max(UPPER_ARM**2 - a * a, 0.0))
    out = Vector(pole) - d * Vector(pole).dot(d)
    out.normalize()
    elbow = root + d * a + out * h
    end = root + d * length

    def aim(free_rotation, start, target):
        q = free_rotation.col[1].normalized().rotation_difference((target - start).normalized())
        return q.to_matrix() @ free_rotation

    up = aim(free.to_3x3(), root, elbow)
    fo = aim(up @ REST3[upper].inverted() @ REST3[fore], elbow, end)
    chest3 = chest.to_3x3()
    bones = arm_obj.pose.bones
    bones[upper].rotation_euler = ((chest3 @ REST3["chest"].inverted() @ REST3[upper]).inverted() @ up).to_euler("XYZ")
    bones[fore].rotation_euler = ((up @ REST3[upper].inverted() @ REST3[fore]).inverted() @ fo).to_euler("XYZ")
    bones[hand_bone].rotation_euler = ((fo @ REST3[fore].inverted() @ REST3[hand_bone]).inverted() @ hand).to_euler("XYZ")


# Lying down, where the right wrist holds each gun, from the right shoulder: the rifle under the chin,
# the revolver further out and nearer the middle, where both hands meet on it.
GUN_AHEAD = {"rifle": Vector((0.17, -0.3, -0.12)), "pistol": Vector((0.2, -0.4, -0.1))}


def lying_arms(t, moving, back=False):
    """A gun is carried forward in both hands under the chin, nudged ahead with each push. A grenade or a
    bandage stays in the right hand while the hands take turns pulling along the ground."""
    bpy.context.view_layer.update()
    shoulder = {n: arm_obj.pose.bones[f"upper_arm_{n}"].head.copy() for n in ("R", "L")}
    hands = CARRY_HAND[CURRENT]
    if CURRENT in GUN_AHEAD:
        nudge = 0.04 * math.sin(2 * TAU * t) if moving else 0.0
        grip = shoulder["R"] + GUN_AHEAD[CURRENT] - Vector((0, nudge, 0))
        grip.z = max(grip.z, 0.1 if moving else 0.16)
        reach("R", grip, hands["R"][0], (-1.0, 0.2, -0.5))
        reach("L", grip + (hands["L"][1] - hands["R"][1]), hands["L"][0], (1.0, 0.0, -0.5))
        return
    for n, sx in (("R", -1.0), ("L", 1.0)):
        # Each hand pulls along with the opposite leg's push.
        d = drawn(t + (0.0 if n == "R" else 0.5)) if moving else 0.35
        if back:
            d = 1.0 - d
        lift = 0.06 * math.sin(math.pi * d) if moving and drawn(t + (0.0 if n == "R" else 0.5) + 0.01) > d else 0.0
        wrist = Vector((shoulder[n].x + sx * 0.1, shoulder[n].y - 0.05 - 0.42 * d, 0.06 + lift))
        turned = hands["R"][0] if n == "R" else FLAT_HAND[n]
        reach(n, wrist, turned, (sx * 1.0, 0.2, 0.4))


def crawl_gait(yaw=0.0, amount=1.0, back=False):
    """Army crawl. The legs take turns drawing a knee up along the ground and pushing off it; the hips
    roll up on the drawn side and the spine snakes towards it, while the arms pull or carry the gun."""

    def fn(t):
        u = 1.0 - t if back else t
        lie(math.sin(TAU * u), yaw=yaw)
        dl = drawn(u) * amount
        dr = drawn(u + 0.5) * amount
        frog_leg("L", dl)
        frog_leg("R", dr)
        roll = 0.12 * (dl - dr)
        bones = arm_obj.pose.bones
        for name, m in (
            ("hips", Matrix.Rotation(-roll, 3, "Z")),
            ("spine", Matrix.Rotation(0.06 * (dl - dr), 3, "Y")),
            ("chest", Matrix.Rotation(roll * 0.6, 3, "Z") @ Matrix.Rotation(-0.05 * (dl - dr), 3, "Y")),
            ("head", Matrix.Rotation(-roll * 0.5, 3, "Z")),
        ):
            bones[name].rotation_euler = (local_euler(name, m).to_matrix() @ bones[name].rotation_euler.to_matrix()).to_euler("XYZ")
        settle()
        lying_arms(u, True, back)

    return fn


def crawl_idle(t):
    """Lying still, propped on the elbows with the gun up and aimed."""
    lie(math.sin(TAU * t), arch=PRONE_ARCH * 1.3)
    frog_leg("L", 0.18)
    frog_leg("R", 0.05)
    settle()
    lying_arms(t, False)


def crawl_stride():
    """How far one cycle carries the body: each knee slides from beside the hip to straight out once."""
    knees = []
    for draw in (0.0, 1.0):
        reset_pose()
        lie()
        frog_leg("L", draw)
        bpy.context.view_layer.update()
        knees.append(arm_obj.pose.bones["shin_L"].head.copy())
    reset_pose()
    return 2 * abs(knees[1].y - knees[0].y)


def slide(t):
    """Knee-slide: sat back, lead leg out in front, trailing knee down, rifle held up."""
    breath = math.sin(TAU * t)
    pose_loc("hips", (0, 0, -0.5 + 0.004 * breath))
    pose_rot("hips", 0.3, 0, 0)
    pose_rot("spine", -0.1, 0, 0)
    pose_rot("chest", -0.1 + 0.01 * breath, 0, 0)
    pose_rot("head", -0.1, 0, 0)
    pose_rot("thigh_R", -1.3, -0.1, 0)
    pose_rot("shin_R", 0.15, 0, 0)
    pose_rot("foot_R", -0.35, 0, 0)
    pose_rot("thigh_L", -0.3, 0.12, 0)
    pose_rot("shin_L", 2.4, 0, 0)
    pose_rot("foot_L", 0.5, 0, 0)


# Sitting: the hips come down this far, thighs level and shins reaching forward.
SIT_DROP = 0.42


def sit(t):
    """In a vehicle seat: upright, thighs forward, the item still in hand, breathing."""
    breath = math.sin(TAU * t)
    pose_loc("hips", (0, 0, -SIT_DROP))
    pose_rot("hips", -0.05, 0, 0)
    pose_rot("spine", 0.03, 0, 0)
    pose_rot("chest", 0.02 + 0.015 * breath, 0, 0)
    pose_rot("head", -0.02 - 0.01 * breath, 0, 0)
    # The shins reach forward under a dashboard rather than straight down, so the feet stay in the car.
    for side in ("L", "R"):
        pose_rot(f"thigh_{side}", -1.52, 0.1 if side == "R" else -0.1, 0)
        pose_rot(f"shin_{side}", 0.72, 0, 0)
        pose_rot(f"foot_{side}", 0.72, 0, 0)


# One crawl cycle, two pushes, at 60 fps.
CRAWL_FRAMES = 78


def build_actions():
    global CURRENT
    walk = dict(amp=0.68, lat_amp=0.38, knee=0.95, lean=0.0, bob=0.022, cycle_sway=0.03)
    run = dict(amp=0.95, lat_amp=0.5, knee=1.45, lean=0.2, bob=0.04, cycle_sway=0.04)
    dirs = {"fwd": (0, 1), "back": (0, -1), "right": (1, 0), "left": (-1, 0)}
    for stance, prefix in STANCE_PREFIX.items():
        CURRENT = stance
        bake(f"{prefix}idle", 120, idle)
        for name, d in dirs.items():
            p = dict(walk)
            if name == "back":
                p["lean"] = -0.05
            bake(f"{prefix}walk_{name}", 36, gait(d, **p))
        for name, d in dirs.items():
            p = dict(run)
            if name == "back":
                p["lean"] = 0.0
                p["amp"] = 0.8
            if name in ("left", "right"):
                p["lean"] = 0.1
            bake(f"{prefix}run_{name}", 24, gait(d, **p))
        bake(f"{prefix}crouch_idle", 120, crouch_idle)
        for name, d in dirs.items():
            bake(f"{prefix}crouch_{name}", 36, crouch_gait(d, 0.3, 0.26, 0.5))
        bake(f"{prefix}crawl_idle", 180, crawl_idle)
        bake(f"{prefix}crawl_fwd", CRAWL_FRAMES, crawl_gait())
        bake(f"{prefix}crawl_back", CRAWL_FRAMES, crawl_gait(amount=0.8, back=True))
        # Sideways the body turns a little towards where it goes and crawls on that slant.
        bake(f"{prefix}crawl_left", CRAWL_FRAMES, crawl_gait(yaw=0.6, amount=0.8))
        bake(f"{prefix}crawl_right", CRAWL_FRAMES, crawl_gait(yaw=-0.6, amount=0.8))
        bake(f"{prefix}slide", 120, slide)
        bake(f"{prefix}jump_air", 60, jump_air)
        bake(f"{prefix}sit", 120, sit)
    # Dying lets go of everything, so one clip serves every stance.
    CURRENT = "rifle"
    bake("death", 60, death, loop=False)
    reset_pose()
    arm_obj.animation_data.action = bpy.data.actions["idle"]


build_actions()
anim_action = arm_obj.animation_data.action
arm_obj.animation_data.action = None
print(f"crawl: {crawl_stride() / (CRAWL_FRAMES / FPS):.2f} m/s")
arm_obj.animation_data.action = anim_action

# The death clip should not loop-wrap with a pose change: hold the final pose on its closing key.
# (It is `t = f / frames`, so the last key at t = 1 equals the lying-down pose and is not wrapped.)


# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
def export():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    arm_obj.select_set(True)
    body.select_set(True)
    for obj in weapon_objects:
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
        export_cameras=False,
        export_lights=False,
        export_vertex_color="MATERIAL",
        export_all_vertex_colors=False,
        export_active_vertex_color_when_no_material=False,
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
    scene.display.shading.color_type = "VERTEX"
    scene.render.resolution_x, scene.render.resolution_y = 360, 480
    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 2.1
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam

    views = {
        "side": (Vector((6, 0, 0.9)), Vector((0, 0, 0.9))),
        "front": (Vector((0, -6, 0.9)), Vector((0, 0, 0.9))),
        "three": (Vector((-4, -5, 3.0)), Vector((0, 0, 0.85))),
        # Lying down, the body runs along -Y from the feet: frame it from above and from the side.
        "lie_top": (Vector((0, -0.85, 6.0)), Vector((0, -0.85, 0.0))),
        "lie_side": (Vector((6, -0.85, 0.4)), Vector((0, -0.85, 0.2))),
        "lie_three": (Vector((-3.5, -4.0, 2.4)), Vector((0, -0.85, 0.2))),
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
    for clip, frames in (("walk_fwd", 36), ("walk_right", 36), ("run_fwd", 24), ("run_left", 24)):
        for i in range(4):
            at(clip, i * frames // 4)
            shoot(f"{clip}_{i}", "side" if clip.endswith("fwd") else "front")
    for clip, view in (("slide", "side"), ("slide", "front"), ("crouch_idle", "side"), ("crouch_idle", "three"), ("crouch_fwd", "side"), ("crouch_right", "front"), ("sit", "side"), ("sit", "three"), ("pistol_sit", "side")):
        for i in range(2):
            at(clip, i * (18 if "fwd" in clip or "right" in clip else 60))
            shoot(f"{clip}_{view}_{i}", view)
    # Lying down in each stance: still from three sides, and a crawl cycle in four steps.
    for stance, item in (("", "ak5"), ("pistol_", "revolver"), ("hold_", "grenade")):
        for obj in weapon_objects:
            obj.hide_render = obj.name != f"weapon_{item}"
        at(f"{stance}crawl_idle", 0)
        for view in ("lie_side", "lie_top", "lie_three"):
            shoot(f"{stance}crawl_idle_{view}", view)
        for i in range(4):
            at(f"{stance}crawl_fwd", i * CRAWL_FRAMES // 4)
            shoot(f"{stance}crawl_fwd_{i}_top", "lie_top")
            shoot(f"{stance}crawl_fwd_{i}_three", "lie_three")
    for obj in weapon_objects:
        obj.hide_render = False
    # Each weapon in the stance it is held in, one at a time.
    for obj in weapon_objects:
        obj.hide_render = True
    for wname, wstance, _, _ in WEAPONS:
        for obj in weapon_objects:
            obj.hide_render = obj.name != f"weapon_{wname}"
        for clip in ("idle", "run_fwd"):
            at(STANCE_PREFIX[wstance] + clip, 6)
            for view in ("side", "three"):
                shoot(f"held_{wname}_{clip}_{view}", view)
    for obj in weapon_objects:
        obj.hide_render = False
    at("jump_air", 10)
    shoot("jump", "side")
    at("death", 60)
    shoot("death", "side")


if PREVIEW:
    preview(PREVIEW)
