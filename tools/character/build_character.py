"""
Builds the player character and exports it to client/public/models/player.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/character/build_character.py -- \
        [--out client/public/models/player.glb] [--preview DIR [--only BODY] [--strip CLIPS]] \
        [--preview-source DIR]

It runs in three stages (CHARACTER-LOOKS.md in docs/ has the whole pipeline):
  1. The source rig: the game's own 27 bone rig, its clips posed in code below, one baked action per
     movement clip per stance. Its body (`bodies.py`) is not exported; it settles the lying clips.
  2. The people (`characters.py`): Quaternius' Animated Men and Women, recoloured into the looks and
     bound to one skeleton named as the game names its bones.
  3. The clips onto the people (`retarget.py`): every source clip, and in place of some of them the
     hand-keyed clips of Quaternius' Universal Animation Library (`library.py`, `FROM_LIBRARY`). The
     weapons are moved onto the people's right hand. Only the people, their skeleton and the weapons
     are exported.

Conventions (of the source rig and its clips; the people's skeleton has no toes, and on the people the
clips in `FROM_LIBRARY` are the library's, the source's of that name giving only the held upper body)
  * Blender: Z up, the character faces -Y, its right hand side is -X. The glTF exporter turns that into
    Y up, facing +Z (the renderer spins it by PI so it faces the game's -Z).
  * Origin is at the feet. Height is about 1.72 m, matching the 1.7 m capsule.
  * Looks: thirteen bodies, `body_<name>` for each row of `characters.BODIES`, all on the one skeleton;
    the client shows one.
  * Three materials, so up to three draw calls per player: `outfit` (vertex colours are shades of white,
    tinted per character at runtime: a soldier's patches, a civilian's top), `gear` (vertex colours are
    the final colours) and `skin` (shades of white, tinted with a skin tone). Each weapon is its own
    mesh, `weapon_<name>`, skinned to the right hand and made in the pose it is held in.
  * Hands and feet: `fingers_*` curls the four fingers together and `thumb_*` the thumb; `toes_*` bends
    at the ball of the foot. No clip keys them itself: `bake` closes the hands on what the stance holds
    (`GRIP_CURL`) unless the clip is `empty_handed`, and `roll_toes` bends the toes up wherever a pose
    would push their tips into the ground, so a lifting heel rolls over the ball of the foot.
  * Stances say how the arms hold things: `rifle` (both hands on the gun), `pistol` (both hands, out in front),
    `hold` (hands low, in front), `shoulder` (a launcher tube on the right shoulder) and `free` (empty
    hands, swinging). Clips are baked per stance: the rifle's are named plainly (`idle`, `walk_fwd`,
    ...), the others carry a `pistol_` / `hold_` / `shoulder_` / `free_` prefix. `freefall`, `chute` and the get-ups are shared. There is no death clip: a body
    killed is a ragdoll (KNOCKDOWN.md).
  * Clips: idle, walk_fwd/back/left/right, run_fwd/back/left/right, crouch_idle, crouch_fwd/back/left/
    right, crawl_idle, crawl_fwd/back/left/right, slide, jump_air, sit, freefall (a skydiver's box,
    belly down about `FALL_PIVOT`), chute (hanging under the canopy), and getup_back and getup_front
    (up off the ground after a knockdown, through a squat). The crawl is an army
    crawl lying face down along -Y, resting on the ground at the origin's height. `sit` is a seat in a
    vehicle: hips dropped by `SIT_DROP`, thighs level and shins down, so the head ends up `SIT_HEAD`
    above the origin (the client places a seated player by it). `car_out` and `car_out_r` (per stance,
    played once over `CAR_OUT_FRAMES`) get out of a car's front seat to the soldier's left and right: from
    `sit`, swivel towards the door, the outer leg out to the ground, duck under a roof line `CAR_ROOF` over
    the ground, and stand into `idle`. The root stays put (the client carries the body out of the seat);
    the hips rise by `SIT_DROP` within the clip. Played backwards it gets in. Clips loop with a duplicated last key so the mixer wraps without a hitch.
  * Swimming clips, shared by every stance as the gun is put away: swim_tread (upright, the head out),
    swim_fwd (a breaststroke tipped forward about `SWIM_PIVOT`), dive_idle and dive_fwd (flat under the
    water about `DIVE_PIVOT`, near the capsule centre, which the client tips towards where it swims).
  * Zombie clips, shared by every stance as a zombie's hands are empty: zombie_idle and zombie_walk
    (the library's on the people; the source's are hunched and swaying with the arms half up, and a
    shamble with the right leg dragged and the arms out in front), zombie_claw (the
    right arm up and back, then raked down across the front) and zombie_rise (once, out of the
    grave: a hand through the soil, the head, a heave out on both hands, a knee up, and standing).
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Euler, Matrix, Quaternion, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.dont_write_bytecode = True
sys.path.insert(0, HERE)
import bodies  # noqa: E402
import characters  # noqa: E402
import library  # noqa: E402
import retarget  # noqa: E402
from parts import MAT_GEAR, MAT_OUTFIT, MAT_SKIN, Builder, grey, sharpen, srgb  # noqa: E402

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def arg(name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/player.glb")))
PREVIEW = arg("--preview")
PREVIEW_SOURCE = arg("--preview-source")
# Only the preview shots whose names start with this.
ONLY = arg("--only", "")
# With `--preview`: clips (comma-separated) to render through their cycle instead, eight phases each.
STRIP = [c for c in arg("--strip", "").split(",") if c]

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
# From the wrist to the knuckles, and the fingers' length, metres.
KNUCKLES = 0.07
FINGERS = 0.08
# How far ahead of the ankle the ball of the foot is, where the toes bend, and the toe's tip (-y is forward).
BALL_Y = -0.108
TOE_Y = -0.215

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
    # A recoilless rifle over the right shoulder: the right hand on the pistol grip under the tube, just
    # ahead of the shoulder, the left reaching across to the front grip.
    "shoulder": dict(
        R=Vector((-0.19, -0.27, 1.32)), L=Vector((-0.13, -0.47, 1.36)),
        pole_r=Vector((-0.6, 0.3, -1.0)), pole_l=Vector((0.6, 0.1, -1.0)),
    ),
}
STANCE_PREFIX = {"rifle": "", "pistol": "pistol_", "hold": "hold_", "shoulder": "shoulder_", "free": "free_"}
# `free` is empty hands: the arms hang and swing, and in the pack's clips they move as the pack's do.


# The weapons' colours (the bodies' are in `bodies.py`).
DARK = srgb(0x2A2C31)
GUN_BODY = srgb(0x3B3F47)
GUN_TRIM = srgb(0xF2A33A)
WOOD = srgb(0x7A4A2A)
OLIVE = srgb(0x4F5D2E)
TUBE = srgb(0x55613A)
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
    # The four fingers together from the knuckles, and the thumb from the heel of the hand.
    bone(f"fingers_{side}", f"hand_{side}", (wr[0], wr[1], wr[2] - KNUCKLES), (wr[0], wr[1], wr[2] - KNUCKLES - FINGERS))
    bone(f"thumb_{side}", f"hand_{side}", (wr[0] - sx * 0.004, -0.03, wr[2] - 0.012), (wr[0] - sx * 0.012, -0.058, wr[2] - 0.068))
    # The toes from the ball of the foot.
    bone(f"toes_{side}", f"foot_{side}", (sx * LEG_X, BALL_Y, 0.035), (sx * LEG_X, TOE_Y, 0.035))


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
# Every mesh's vertex groups, one per bone in the rig's order, so every mesh binds to the same joints.
GROUPS = {name: i for i, name in enumerate(BONES)}
# The mesh being built: `start_mesh` begins one, and `box`, `prism` and `blob` build into it.
B = Builder(GROUPS)


def start_mesh():
    global B
    B = Builder(GROUPS)


def box(*args, **kwargs):
    return B.box(*args, **kwargs)


def prism(*args, **kwargs):
    return B.prism(*args, **kwargs)


def blob(*args, **kwargs):
    return B.blob(*args, **kwargs)


# The builder's dimensions, as `bodies` reads them.
DIMS = type("Dims", (), {k: v for k, v in globals().items() if k.isupper() and isinstance(v, float)})


def pose_to_rest(stance):
    """Poses the rig in `stance` and returns the matrix that un-poses points onto the right hand bone."""
    reset_pose()
    carry_pose(stance)
    bpy.context.view_layer.update()
    return skin_matrix("hand_R").inverted()


def build_weapon(parts, stance):
    """`parts`: (kind, centre offset from the grip, size, colour) in the stance's pose space. A `tube` runs
    along the body's forward axis: its size is (radius at the back, radius at the front, length)."""
    rest_from_pose = pose_to_rest(stance)
    g = STANCES[stance]["R"]
    for kind, off, size, color in parts:
        if kind == "box":
            box(g + Vector(off), size, "hand_R", MAT_GEAR, color, transform=rest_from_pose)
        elif kind == "tube":
            back, front, length = size
            c = g + Vector(off)
            prism(c + Vector((0, length / 2, 0)), c - Vector((0, length / 2, 0)), back, front, "hand_R", MAT_GEAR, color, sides=8, transform=rest_from_pose)
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
    (
        # Carl Gustaf: an olive tube over the shoulder, the venturi flaring out behind, a sight on its left.
        "carl_gustaf",
        "shoulder",
        [
            ("tube", (-0.03, 0.20, 0.20), (0.05, 0.05, 1.0), TUBE),  # barrel
            ("tube", (-0.03, 0.74, 0.20), (0.075, 0.052, 0.10), DARK),  # venturi
            ("tube", (-0.03, -0.31, 0.20), (0.058, 0.058, 0.06), DARK),  # muzzle ring
            ("tube", (-0.03, 0.20, 0.20), (0.058, 0.058, 0.07), DARK),  # shoulder piece band
            ("box", (-0.01, -0.02, 0.07), (0.04, 0.05, 0.15), GUN_TRIM),  # pistol grip
            ("box", (-0.02, -0.20, 0.10), (0.035, 0.04, 0.12), DARK),  # front grip
            ("box", (0.05, -0.06, 0.23), (0.04, 0.12, 0.05), DARK),  # sight
            ("box", (0.05, 0.01, 0.23), (0.05, 0.03, 0.055), DARK),  # sight eyepiece
            ("box", (-0.03, 0.30, 0.14), (0.05, 0.12, 0.03), DARK),  # shoulder rest
        ],
        (-0.03, -0.34, 0.20),
    ),
]

outfit_mat = bpy.data.materials.new("outfit")
gear_mat = bpy.data.materials.new("gear")
skin_mat = bpy.data.materials.new("skin")
for m in (outfit_mat, gear_mat, skin_mat):
    m.use_nodes = True
    nodes = m.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 1.0
    bsdf.inputs["Metallic"].default_value = 0.0
    attr = nodes.new("ShaderNodeVertexColor")
    attr.layer_name = "Color"
    m.node_tree.links.new(attr.outputs["Color"], bsdf.inputs["Base Color"])


def make_object(name, builder, armature=None, groups=None):
    """Turns what `builder` put in the current bmesh into an object skinned to `armature` (the source
    rig by default)."""
    armature = armature or arm_obj
    groups = groups or GROUPS
    mesh_ = bpy.data.meshes.new(name)
    builder.weld()
    builder.bm.to_mesh(mesh_)
    builder.bm.free()
    sharpen(mesh_)
    mesh_.attributes.active_color = mesh_.color_attributes["Color"]
    mesh_.materials.append(outfit_mat)
    mesh_.materials.append(gear_mat)
    mesh_.materials.append(skin_mat)
    obj = bpy.data.objects.new(name, mesh_)
    scene.collection.objects.link(obj)
    for gname in groups:
        obj.vertex_groups.new(name=gname)
    mod_ = obj.modifiers.new("Armature", "ARMATURE")
    mod_.object = armature
    obj.parent = armature
    return obj, mesh_


# The source rig's body: only for settling the lying clips on the ground. The game draws the packs'
# people (`characters.py`), onto which every clip is retargeted at the end.
look_objects = []
for look in ("soldier",):
    start_mesh()
    bodies.build(look, B, DIMS)
    obj, look_mesh = make_object(look if look.startswith("hat_") else f"body_{look}", B)
    for gname, index in GROUPS.items():
        assert obj.vertex_groups[gname].index == index
    print(f"{obj.name}: {sum(len(p.vertices) - 2 for p in look_mesh.polygons)} tris")
    look_objects.append(obj)
# The soldier is the one the lying clips are settled on the ground by.
body = look_objects[0]
mesh = body.data

# Weapons: one mesh each, in the pose it is held in, and the muzzle where its barrel ends.
weapon_objects = []
MUZZLES = {}  # bone name -> (stance, tip in rest space)
for wname, wstance, parts, muzzle in WEAPONS:
    start_mesh()
    build_weapon(parts, wstance)
    obj, _ = make_object(f"weapon_{wname}", B)
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
    for name, e in CARRY.get(CURRENT, {}).items():
        arm_obj.pose.bones[name].rotation_euler = e.copy()


# How far each hand's fingers curl round what the stance holds, radians at the knuckles: the right
# hand round the grip, the left round the foregrip, the revolver's other hand or nothing much.
GRIP_CURL = {
    "rifle": {"R": 1.45, "L": 1.25},
    "pistol": {"R": 1.45, "L": 1.2},
    "hold": {"R": 1.3, "L": 0.45},
    "shoulder": {"R": 1.45, "L": 1.3},
    "free": {"R": 0.35, "L": 0.35},
}
# Fingers curled a little, as a hand hangs with nothing in it.
RELAXED = 0.3


def curl_hand(side, curl, thumb=None):
    """Curls `side`'s fingers `curl` radians in towards the palm, and the thumb across it (by default
    as far as the fingers take it). The palms face in at rest, so the curl turns about the forward axis."""
    s = 1.0 if side == "L" else -1.0
    thumb = min(curl, 1.0) * 0.55 if thumb is None else thumb
    pose_rot(f"fingers_{side}", 0, s * curl, 0)
    pose_rot(f"thumb_{side}", -0.2 * thumb, s * thumb, 0)


def grip_hands():
    """Both hands closed on what the stance holds: every clip starts so."""
    for side, curl in GRIP_CURL[CURRENT].items():
        curl_hand(side, curl)


def relax_hands(curl=RELAXED):
    """Both hands open, for a clip whose hands hold nothing (swimming, falling, a zombie's)."""
    for side in ("L", "R"):
        curl_hand(side, curl, thumb=0.15)


# Where each toe's tip and the ball it bends at are in the rest pose: the toes bend up to keep the
# tip out of the ground.
TOE_TIP = {side: Vector((sx * LEG_X, TOE_Y + 0.005, 0.002)) for side, sx in (("L", 1), ("R", -1))}
# The most the toes bend up, radians.
TOE_BEND = 1.0


def roll_toes():
    """Bends each foot's toes up at the ball until the tip is out of the ground, so a heel that lifts
    rolls the foot over the ball instead of pushing the toes into the ground. Runs on the posed rig."""
    for side in ("L", "R"):
        pb = arm_obj.pose.bones[f"toes_{side}"]
        pb.rotation_euler = (0, 0, 0)
    bpy.context.view_layer.update()
    for side in ("L", "R"):
        pb = arm_obj.pose.bones[f"toes_{side}"]
        posed = pb.matrix @ arm_data.bones[pb.name].matrix_local.inverted()
        ball = posed @ arm_data.bones[pb.name].head_local
        tip = posed @ TOE_TIP[side]
        if tip.z >= 0.0:
            continue
        # The foot's own lateral axis: the toes bend about it, the way that lifts the tip.
        across = (posed.to_3x3() @ Vector((1, 0, 0))).normalized()
        height = lambda angle: (ball + Matrix.Rotation(angle, 3, across) @ (tip - ball)).z
        up = TOE_BEND if height(0.3) > height(-0.3) else -TOE_BEND
        # Bisect between bent enough (`out`) and not (`into`) for the least bend that clears the ground.
        out, into = up, 0.0
        if height(out) < 0.0:
            into = out
        for _ in range(16):
            mid = (out + into) / 2
            if height(mid) < 0.0:
                into = mid
            else:
                out = mid
        pose_rot(f"toes_{side}", out, 0, 0)


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


# Every clip baked on the source rig, by name: these are retargeted onto the packs' people.
BAKED = set()


def bake(name, frames, pose_fn, loop=True):
    """One action: `pose_fn(phase)` poses the rig for phase 0..1. Looping clips repeat the first key last."""
    BAKED.add(name)
    new_action(name)
    scene.frame_start, scene.frame_end = 0, frames
    for f in range(frames + 1):
        reset_pose()
        set_arms_carry()
        grip_hands()
        pose_fn((f % frames) / frames if loop else f / frames)
        roll_toes()
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
            if CURRENT == "free":
                # Empty hands swing against the leg on their side, the elbow bending as it comes forward.
                sx = 1.0 if side == "L" else -1.0
                swing = 0.7 * amp * s * df
                pose_rot(f"upper_arm_{side}", swing, -sx * 0.12, 0)
                pose_rot(f"forearm_{side}", -0.25 - 0.3 * max(0.0, -s * df) * amp, 0, 0)
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
        if CURRENT == "free":
            sx = 1.0 if side == "L" else -1.0
            pose_rot(f"upper_arm_{side}", 0.02 * breath, -sx * 0.1, 0)
            pose_rot(f"forearm_{side}", -0.15, 0, 0)


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


# Falling with the chute shut: the body pitches belly-down this far (rad) about `FALL_PIVOT` (metres up,
# about the chest), which stays where it stands so the capsule still holds the body.
FALL_PITCH = 1.3
FALL_PIVOT = 1.0


def freefall(t):
    """A skydiver's box: belly to the ground, back arched, arms out with the forearms up, knees bent and
    spread. The air flutters the limbs."""
    phi = TAU * t
    flutter = lambda k, off=0.0: 0.05 * math.sin(k * phi + off)
    pose_rot("root", FALL_PITCH, 0, 0)
    pose_loc("root", (0, FALL_PIVOT * math.sin(FALL_PITCH), FALL_PIVOT * (1 - math.cos(FALL_PITCH))))
    pose_rot("hips", -0.1, 0, 0.02 * math.sin(phi))
    pose_rot("spine", -0.2, 0, 0)
    pose_rot("chest", -0.2 + flutter(2), 0, 0)
    # Looking ahead to the ground coming up, not at it.
    pose_rot("head", -0.5 + flutter(3, 1.0), 0, flutter(1))
    for side, s in (("L", 1), ("R", -1)):
        off = 0.0 if side == "L" else 1.7
        pose_rot(f"upper_arm_{side}", -0.25 + flutter(3, off), -s * 1.35, 0)
        pose_rot(f"forearm_{side}", 0, -s * 1.6 + flutter(4, off), 0)
        pose_rot(f"hand_{side}", 0, 0, 0)
        pose_rot(f"thigh_{side}", 0.3 + flutter(3, off + 0.5), -s * 0.35, 0)
        pose_rot(f"shin_{side}", 1.1 + flutter(4, off), 0, 0)
        pose_rot(f"foot_{side}", 0.4, 0, 0)


def chute(t):
    """Hanging in the harness: hands up on the toggles, legs dangling and swinging a little."""
    phi = TAU * t
    sway = math.sin(phi)
    pose_loc("hips", (0, 0, -0.01))
    pose_rot("hips", 0.06, 0, 0.03 * sway)
    pose_rot("spine", 0.02, 0, -0.02 * sway)
    pose_rot("chest", 0.03 + 0.015 * math.sin(2 * phi), 0, 0)
    # Looking down at where it will land, glancing about.
    pose_rot("head", 0.25, 0, 0.12 * math.sin(phi + 0.6))
    for side, s in (("L", 1), ("R", -1)):
        pull = 0.12 * math.sin(phi + (0.0 if side == "L" else math.pi))
        pose_rot(f"upper_arm_{side}", -0.1, -s * (2.65 - pull), 0)
        pose_rot(f"forearm_{side}", 0, -s * (0.45 + pull), 0)
        pose_rot(f"hand_{side}", 0, 0, 0)
        swing = 0.1 * math.sin(phi + (0.0 if side == "L" else 1.2))
        pose_rot(f"thigh_{side}", -0.15 + swing, -s * 0.06, 0)
        pose_rot(f"shin_{side}", 0.3 - swing * 0.5, 0, 0)
        pose_rot(f"foot_{side}", 0.5, 0, 0)



def ease(x):
    x = min(1.0, max(0.0, x))
    return x * x * (3 - 2 * x)


def mix(a, b, k):
    return a + (b - a) * k


# A deep squat, the middle of getting up: hips low, knees up, arms forward for balance.
SQUAT = {"hips_z": -0.62, "hips": 0.5, "spine": 0.2, "chest": 0.1, "head": -0.1, "thigh": -1.9, "shin": 2.3, "foot": -0.4, "upper_arm": -0.6, "forearm": 0.4}


def rise(t, lying, start):
    """Getting up from flat on the ground (`lying`: the root's pitch, -pi/2 on the back with the head
    behind, +pi/2 on the belly with it ahead) by way of `start`, the pose it has pushed itself into by
    a third of the way (bone -> pitch), then a squat, then standing."""
    a = ease(t / 0.35)
    b = ease((t - 0.35) / 0.35)
    c = ease((t - 0.7) / 0.3)
    # Pushed up, then rolled onto the feet into the squat, then up out of it.
    pose = {key: mix(mix(0.0, value, a), SQUAT.get(key, 0.0), b) for key, value in start.items()}
    pose = {key: mix(value, 0.0, c) for key, value in pose.items()}
    pose_rot("root", mix(lying, 0.0, b), 0, 0)
    pose_loc("root", (0, 0, mix(0.13, 0.0, b)))
    pose_loc("hips", (0, 0, mix(0.0, mix(SQUAT["hips_z"], 0.0, c), b)))
    pose_rot("hips", pose["hips"], 0, 0)
    pose_rot("spine", pose["spine"], 0, 0)
    pose_rot("chest", pose["chest"], 0, 0)
    pose_rot("head", pose["head"], 0, 0)
    for side, s in (("L", 1), ("R", -1)):
        pose_rot(f"thigh_{side}", pose["thigh"], s * 0.1 * (1 - c), 0)
        pose_rot(f"shin_{side}", pose["shin"], 0, 0)
        pose_rot(f"foot_{side}", pose["foot"], 0, 0)
        pose_rot(f"upper_arm_{side}", pose["upper_arm"], -s * 0.15 * (1 - c), 0)
        pose_rot(f"forearm_{side}", pose["forearm"], 0, 0)
        pose_rot(f"hand_{side}", 0, 0, 0)


def getup_back(t):
    """Up from flat on the back: sits up with the hands pushing behind, feet drawn in, rolls forward
    onto them into a squat and stands."""
    rise(t, -math.pi / 2, {"hips_z": 0.0, "hips": 1.45, "spine": 0.15, "chest": 0.1, "head": 0.2, "thigh": -1.95, "shin": 1.4, "foot": -0.3, "upper_arm": 0.7, "forearm": 0.2})


def getup_front(t):
    """Up from flat on the belly: pushes the chest up on straight arms, brings the knees under into a
    squat and stands."""
    rise(t, math.pi / 2, {"hips_z": 0.0, "hips": -0.6, "spine": -0.2, "chest": -0.1, "head": -0.4, "thigh": 0.0, "shin": 0.3, "foot": 0.3, "upper_arm": -1.45, "forearm": 0.0})


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
GUN_AHEAD = {"rifle": Vector((0.17, -0.3, -0.12)), "pistol": Vector((0.2, -0.4, -0.1)), "shoulder": Vector((0.12, -0.3, -0.12))}


def lying_arms(t, moving, back=False):
    """A gun is carried forward in both hands under the chin, nudged ahead with each push. A grenade or a
    bandage stays in the right hand while the hands take turns pulling along the ground."""
    bpy.context.view_layer.update()
    shoulder = {n: arm_obj.pose.bones[f"upper_arm_{n}"].head.copy() for n in ("R", "L")}
    hands = CARRY_HAND.get(CURRENT, CARRY_HAND["hold"])
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
        turned = hands["R"][0] if n == "R" and CURRENT != "free" else FLAT_HAND[n]
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


# Getting out of a car's front seat through the side door (`car_out`, to the soldier's left, and
# `car_out_r`, to the right), played once forward to get out and backward to get in. It starts in `sit`
# and ends in `idle`, with the root kept still: the client carries the body from the seat to beside the
# door, while the clip raises the hips by `SIT_DROP` from seated to standing.
CAR_OUT_FRAMES = 48
# How high the top of the door opening is over the ground, metres: the head ducks under it on the way out.
CAR_ROOF = 1.25
THIGH = HIP_Z - KNEE_Z
SHIN = KNEE_Z - ANKLE_Z


def planted(drop, pitch, ahead, lift=0.0):
    """A leg with its foot flat `lift` metres over the ground and `ahead` metres in front of the hip, the
    knee forward, with the hips dropped by `drop` and pitched forward by `pitch` (rad): the thigh's,
    shin's and foot's turns about x for `pose_rot`."""
    down = HIP_Z + drop - ANKLE_Z - lift
    length = min(math.hypot(ahead, down), (THIGH + SHIN) * 0.999)
    along = math.atan2(ahead, down)
    at_hip = math.acos((THIGH**2 + length**2 - SHIN**2) / (2 * THIGH * length))
    at_ankle = math.acos((SHIN**2 + length**2 - THIGH**2) / (2 * SHIN * length))
    thigh, shin = along + at_hip, along - at_ankle
    return -thigh - pitch, thigh - shin, shin


def car_out_key(t, yaw, drop, hips, spine, chest, head, twist, look, outer, inner):
    """One key of `CAR_OUT_KEYS`: each leg is `(thigh x, thigh y, thigh out, shin, foot)`, the y and the
    out (a turn about the hips' z, away from the body's middle) as on the soldier's left."""
    return (t, (yaw, drop, hips, spine, chest, head, twist, look) + outer + inner)


def leg(drop, pitch, ahead, y=0.0, out=0.0, lift=0.0):
    """A leg for `car_out_key` with its foot flat `lift` metres over the ground, `ahead` metres in front of the hip."""
    thigh, shin, foot = planted(drop, pitch, ahead, lift)
    return (thigh, y, out, shin, foot)


CAR_OUT_KEYS = [
    # Sat as `sit` sits.
    car_out_key(0.0, 0.0, -SIT_DROP, -0.05, 0.03, 0.02, -0.02, 0.0, 0.0, (-1.52, -0.1, 0.0, 0.72, 0.72), (-1.52, 0.1, 0.0, 0.72, 0.72)),
    # Swivelling on the seat towards the door: the outer leg swings out over the sill, its foot just off the ground,
    # the chest lagging and the head turning to look out.
    car_out_key(0.24, 0.75, -SIT_DROP, -0.02, 0.06, 0.06, 0.12, -0.25, 0.25, leg(-SIT_DROP, -0.02, 0.5, -0.1, 0.35, lift=0.06), (-1.5, 0.1, 0.0, 0.9, 0.5)),
    # The outer foot is down outside and the body leans out over it, ducking under the roof; the inner
    # leg follows, its foot still up on the car's floor.
    car_out_key(0.44, 1.0, -0.40, 0.4, 0.2, 0.15, 0.3, -0.15, 0.1, leg(-0.40, 0.4, 0.42, -0.05, 0.25), leg(-0.40, 0.4, 0.34, 0.05, 0.1, lift=0.14)),
    # Rising on both feet, still bent double under the roof line.
    car_out_key(0.64, 0.8, -0.24, 0.6, 0.25, 0.15, 0.2, -0.05, 0.0, leg(-0.24, 0.6, 0.26, 0.0, 0.12), leg(-0.24, 0.6, 0.18, 0.0, 0.0)),
    # Clear of the car and nearly up, turning back square.
    car_out_key(0.84, 0.25, -0.06, 0.15, 0.05, 0.03, 0.0, 0.0, 0.0, leg(-0.06, 0.15, 0.08, 0.03, 0.02), leg(-0.06, 0.15, 0.06, -0.03, 0.0)),
    # Stood as `idle` stands.
    car_out_key(1.0, 0.0, -0.015, -0.03, 0.01, 0.01, -0.02, 0.0, 0.0, (0.05, 0.06, 0.0, 0.08, -0.11), (0.05, -0.06, 0.0, 0.08, -0.11)),
]


def car_out(s):
    """Out of a car seat to the soldier's left (`s` 1) or right (`s` -1), mirrored by turning the
    other way and swapping which leg leads."""
    outer, inner = ("L", "R") if s > 0 else ("R", "L")

    def fn(t):
        v = track(t, CAR_OUT_KEYS)
        yaw, drop, hips, spine, chest, head, twist, look = v[:8]
        pose_loc("hips", (0, 0, drop))
        pose_rot("hips", hips, 0, s * yaw)
        pose_rot("spine", spine, 0, s * twist * 0.5)
        pose_rot("chest", chest, 0, s * twist * 0.5)
        pose_rot("head", head, 0, s * look)
        for side, (tx, ty, out, shin, foot) in ((outer, v[8:13]), (inner, v[13:18])):
            pose_rot(f"thigh_{side}", tx, s * ty, s * out)
            pose_rot(f"shin_{side}", shin, 0, 0)
            pose_rot(f"foot_{side}", foot, 0, 0)

    return fn


# --------------------------------------------------------------------------------------------------
# On skis. Forward is -Y. The hands are placed by `reach`, the poles being drawn in them by the client.
# --------------------------------------------------------------------------------------------------
def ski_body(lean, legs):
    """Bent forward `lean` from the hips and up the spine, the head kept up to look ahead, on `legs`:
    (side, swing, knee, spread) each, a thigh swung `swing` forward of straight down whatever the
    hips are doing, the knee bent `knee`. The hips come down until the lower foot is on the ski, which
    stays flat under it: with `knee` twice `swing` the ankle is right under the hip, the shin leaning
    into the boot as it does on skis."""
    hip = lean * 0.45
    thigh, shin = HIP_Z - KNEE_Z, KNEE_Z - ANKLE_Z
    drop = min(thigh * (1 - math.cos(swing)) + shin * (1 - math.cos(knee - swing)) for _, swing, knee, _ in legs)
    pose_loc("hips", (0, 0, -drop))
    pose_rot("hips", hip, 0, 0)
    pose_rot("spine", lean * 0.3, 0, 0)
    pose_rot("chest", lean * 0.25, 0, 0)
    pose_rot("head", -lean * 0.8, 0, 0)
    for side, swing, knee, spread in legs:
        s = 1 if side == "L" else -1
        pose_rot(f"thigh_{side}", -swing - hip, -s * (0.05 + spread), 0)
        pose_rot(f"shin_{side}", knee, 0, 0)
        pose_rot(f"foot_{side}", swing - knee, 0, 0)


def ski_hand(side, ahead, down, out=0.16):
    """The wrist `ahead` of its shoulder and `down` below it, `out` to its side, gripping the pole."""
    bpy.context.view_layer.update()
    shoulder = arm_obj.pose.bones[f"upper_arm_{side}"].head
    wide = 1 if shoulder.x > 0 else -1
    reach(side, shoulder + Vector((wide * out, -ahead, -down)), FLAT_HAND[side], Vector((wide * 0.5, 0.7, -0.2)))


def ski_glide(t):
    """Gliding, easy, tipped a little forward over the feet."""
    breath = 0.02 * math.sin(TAU * t)
    ski_body(0.3, [("L", 0.2 + breath, 0.42 + 2 * breath, 0), ("R", 0.2 + breath, 0.42 + 2 * breath, 0)])
    for side in ("L", "R"):
        ski_hand(side, 0.2, 0.42)


def ski_stride(reach_out, kick, lean):
    """Diagonal stride: a long kick behind, the opposite arm reaching `reach_out` with the pole."""

    def fn(t):
        phi = TAU * t
        legs = []
        for side, offset in (("L", 0.0), ("R", math.pi)):
            s = math.sin(phi + offset)
            legs.append((side, 0.14 + kick * s, 0.34 + 0.4 * max(0.0, s) + 0.2 * max(0.0, math.cos(phi + offset)), 0))
        ski_body(lean + 0.04 * math.cos(2 * phi), legs)
        for side, offset in (("L", 0.0), ("R", math.pi)):
            s = math.sin(phi + offset)
            ski_hand(side, 0.14 - reach_out * s, 0.34 + 0.12 * s)

    return fn


def ski_pole(reach_out, drive, fold):
    """Double poling: arms `reach_out` ahead, then the whole body folding `fold` on them, driving `drive` back."""

    def fn(t):
        phi = TAU * t
        crunch = 0.5 - 0.5 * math.cos(phi)
        swing = 0.16 + 0.2 * crunch
        ski_body(0.2 + fold * crunch, [("L", swing, swing * 2, 0), ("R", swing, swing * 2, 0)])
        for side in ("L", "R"):
            ski_hand(side, reach_out - (reach_out + drive) * crunch, 0.2 + 0.36 * crunch)

    return fn


def ski_climb(t):
    """Herringbone: bent into the hill, stepping each ski up and out with the knee high, the opposite pole
    driving from behind. The client splays the skis in their V."""
    phi = TAU * t
    legs = []
    for side, offset in (("L", 0.0), ("R", math.pi)):
        s = math.sin(phi + offset)
        step = max(0.0, s)
        legs.append((side, 0.3 + 0.55 * step - 0.12 * max(0.0, -s), 0.6 + 0.95 * step, 0.26))
    ski_body(0.85 + 0.05 * math.sin(2 * phi), legs)
    pose_rot("hips", 0.85 * 0.45, 0, 0.1 * math.sin(phi))
    for side, offset in (("L", 0.0), ("R", math.pi)):
        ski_hand(side, 0.1 + 0.32 * math.sin(phi + offset), 0.42, out=0.3)


def ski_tuck(t):
    """The tuck: down on the thighs with the chest low over them, the hands tucked in under the chin."""
    wob = 0.012 * math.sin(TAU * t)
    ski_body(1.2, [("L", 0.95 + wob, 1.9 + 2 * wob, 0), ("R", 0.95 + wob, 1.9 + 2 * wob, 0)])
    for side in ("L", "R"):
        ski_hand(side, 0.26, 0.2, out=0.0)


def ski_plow(t):
    """The snowplow: knees in, tails pushed wide, weight forward on the shins."""
    wob = 0.01 * math.sin(TAU * t)
    ski_body(0.35, [("L", 0.42 + wob, 0.84 + 2 * wob, 0.32), ("R", 0.42 + wob, 0.84 + 2 * wob, 0.32)])
    for side in ("L", "R"):
        ski_hand(side, 0.26, 0.34, out=0.24)


def ski_air(t):
    """In the air: skis together, knees soft, hands out for balance."""
    wob = 0.03 * math.sin(TAU * t)
    ski_body(0.4, [("L", 0.34, 0.68, 0), ("R", 0.34, 0.68, 0)])
    for side in ("L", "R"):
        ski_hand(side, 0.18 + wob, 0.3, out=0.32)


def armed_body(lean, legs):
    """`ski_body` with the poles stowed: the hips fold `lean` but the chest stays up, so the gun in the
    stance's grip stays level, wherever the legs are."""
    ski_body(lean, legs)
    pose_rot("chest", -lean * 0.75, 0, 0)
    pose_rot("head", 0, 0, 0)
    set_arms_carry()


def armed_glide(t):
    """Gliding with a gun up."""
    breath = 0.02 * math.sin(TAU * t)
    armed_body(0.25, [("L", 0.2 + breath, 0.42 + 2 * breath, 0), ("R", 0.2 + breath, 0.42 + 2 * breath, 0)])


def armed_skate(t):
    """A short kick off each ski in turn, tails out: all a skier has with no hand for a pole."""
    phi = TAU * t
    legs = []
    for side, offset in (("L", 0.0), ("R", math.pi)):
        s = math.sin(phi + offset)
        legs.append((side, 0.16 + 0.3 * s, 0.36 + 0.3 * max(0.0, s) + 0.12 * max(0.0, math.cos(phi + offset)), 0.1))
    armed_body(0.3 + 0.03 * math.cos(2 * phi), legs)


def armed_climb(t):
    """Herringbone with a gun up."""
    phi = TAU * t
    legs = []
    for side, offset in (("L", 0.0), ("R", math.pi)):
        s = math.sin(phi + offset)
        step = max(0.0, s)
        legs.append((side, 0.3 + 0.55 * step - 0.12 * max(0.0, -s), 0.6 + 0.95 * step, 0.26))
    armed_body(0.6 + 0.05 * math.sin(2 * phi), legs)


def armed_tuck(t):
    """Down on the thighs, as in the tuck, with the gun still level."""
    wob = 0.012 * math.sin(TAU * t)
    armed_body(0.9, [("L", 0.95 + wob, 1.9 + 2 * wob, 0), ("R", 0.95 + wob, 1.9 + 2 * wob, 0)])


def armed_plow(t):
    """The snowplow with a gun up."""
    wob = 0.01 * math.sin(TAU * t)
    armed_body(0.3, [("L", 0.42 + wob, 0.84 + 2 * wob, 0.32), ("R", 0.42 + wob, 0.84 + 2 * wob, 0.32)])


def armed_air(t):
    """In the air with a gun up."""
    armed_body(0.3, [("L", 0.34, 0.68, 0), ("R", 0.34, 0.68, 0)])


# --------------------------------------------------------------------------------------------------
# Golf: one swing, from the address over the ball up to the top, through the ball to the finish, and
# back to the address. The game scrubs it: the backswing as the swing is charged, the rest once it is
# let go (`golf_swing`, `GOLF_*` below and in client/src/render/character.ts).
#
# The golfer stands side-on to the target: the hips turn a quarter right, so the target (the game's
# forward) is on their left and the ball in front of their toes. Positions below are in the golfer's
# own axes before that turn: x their left, towards the target; -y their front, towards the ball; z up.
# --------------------------------------------------------------------------------------------------
GOLF_TURN = Matrix.Rotation(-math.pi / 2, 3, "Z")
# The ball, where the club meets it at address and impact.
GOLF_BALL = Vector((0.03, -0.62, 0.04))
# How far down the grip the right hand is below the left, metres.
GOLF_GRIP_GAP = 0.09
# Forward tilt from the hips, knee flex, and the drop the flexed knees take the hips down by.
GOLF_BEND = 0.5
GOLF_KNEE = 0.3
GOLF_FINISH_BEND = 0.1
GOLF_DROP = (HIP_Z - ANKLE_Z) * (1 - math.cos(GOLF_KNEE))
# Phases of the clip: the top of the backswing, impact, the finish, and the finish held until.
GOLF_TOP = 0.40
GOLF_IMPACT = 0.56
GOLF_FINISH = 0.75
GOLF_HOLD = 0.86


def golf_address_dir():
    return (GOLF_BALL - Vector((0.04, -0.30, 0.80))).normalized()


# (phase, left hand's palm, the club's way from the hands to the head, the torso's turn about the spine).
GOLF_KEYS = [
    (0.00, Vector((0.04, -0.30, 0.80)), golf_address_dir(), 0.0),
    (0.12, Vector((-0.20, -0.34, 0.83)), Vector((-0.70, -0.30, -0.65)), -0.55),
    (0.25, Vector((-0.42, -0.26, 1.05)), Vector((-0.20, 0.05, 1.0)), -1.15),
    (GOLF_TOP, Vector((-0.24, -0.04, 1.42)), Vector((0.95, 0.25, -0.10)), -1.65),
    (0.50, Vector((-0.36, -0.24, 1.02)), Vector((-0.40, 0.05, 0.92)), -0.85),
    (GOLF_IMPACT, Vector((0.07, -0.31, 0.80)), golf_address_dir(), 0.2),
    (0.64, Vector((0.40, -0.30, 0.95)), Vector((0.80, -0.25, -0.50)), 0.95),
    (GOLF_FINISH, Vector((0.26, -0.02, 1.42)), Vector((-0.85, 0.35, -0.25)), 1.65),
    (GOLF_HOLD, Vector((0.26, -0.02, 1.42)), Vector((-0.85, 0.35, -0.25)), 1.65),
    (1.00, Vector((0.04, -0.30, 0.80)), golf_address_dir(), 0.0),
]


def catmull(p0, p1, p2, p3, u):
    return 0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u * u + (-p0 + 3 * p1 - 3 * p2 + p3) * u * u * u)


def golf_key(t):
    """The hands, the club's way and the turn at phase `t`, through the keys on a smooth curve. The held
    finish stays put."""
    keys = GOLF_KEYS
    i = max(k for k in range(len(keys) - 1) if keys[k][0] <= t)
    (t1, h1, d1, b1), (t2, h2, d2, b2) = keys[i], keys[i + 1]
    u = (t - t1) / (t2 - t1)
    if h1 == h2 and d1 == d2:
        return h1.copy(), d1.normalized(), b1
    (_, h0, d0, b0), (_, h3, d3, b3) = keys[max(i - 1, 0)], keys[min(i + 2, len(keys) - 1)]
    # Into and out of the held finish, and from the address, the curve does not overshoot.
    if keys[i][0] == GOLF_HOLD or i == 0:
        h0, d0, b0 = h1, d1, b1
    if keys[i + 1][0] == GOLF_FINISH or i + 2 >= len(keys):
        h3, d3, b3 = h2, d2, b2
    hand = catmull(h0, h1, h2, h3, u)
    way = catmull(d0.normalized(), d1.normalized(), d2.normalized(), d3.normalized(), u).normalized()
    turn = catmull(b0, b1, b2, b3, u)
    return hand, way, turn


def set_bone_matrix(name, head, rotation):
    """Poses bone `name` to `rotation` (armature axes) at its posed head, rotation only."""
    pb = arm_obj.pose.bones[name]
    pb.matrix = Matrix.Translation(head) @ rotation.to_4x4()
    pb.location = (0, 0, 0)
    bpy.context.view_layer.update()


def golf_arm(side, wrist, pole, way, face):
    """Reaches arm `side` so its wrist is at `wrist` (armature axes), the elbow towards `pole`, and lays
    the hand along the club (`way`), its +Z towards the club's face (`face`)."""
    shoulder = arm_obj.pose.bones[f"upper_arm_{side}"].head.copy()
    to_target = wrist - shoulder
    reach = min(to_target.length, (UPPER_ARM + FOREARM) * 0.985)
    direction = to_target.normalized()
    a = (UPPER_ARM**2 - FOREARM**2 + reach**2) / (2 * reach)
    h = math.sqrt(max(UPPER_ARM**2 - a**2, 0.0))
    off = pole - direction * pole.dot(direction)
    off.normalize()
    elbow = shoulder + direction * a + off * h
    reached = shoulder + direction * reach
    down = Vector((0, 0, -1))
    for part, head, tail in (("upper_arm", shoulder, elbow), ("forearm", elbow, reached)):
        name = f"{part}_{side}"
        turn = down.rotation_difference((tail - head).normalized()).to_matrix()
        set_bone_matrix(name, head, turn @ REST3[name])
    y = way.normalized()
    z = (face - y * face.dot(y)).normalized()
    x = y.cross(z)
    hand = Matrix((x, y, z)).transposed()
    set_bone_matrix(f"hand_{side}", reached, hand)


def golf_swing(t):
    hand, way, turn = golf_key(t)
    # Weight goes back onto the right foot in the backswing and through onto the left.
    shift = 0.04 * math.sin(math.pi * min(1.0, t / GOLF_IMPACT)) * (-1 if t < GOLF_TOP else 1) if t < GOLF_HOLD else 0.0
    pose_loc("hips", GOLF_TURN @ Vector((shift, 0, -GOLF_DROP)))
    # Bent over the ball until it is struck, standing tall at the finish.
    tall = ease((t - GOLF_IMPACT) / (GOLF_FINISH - GOLF_IMPACT)) * (1 - ease((t - GOLF_HOLD) / (1 - GOLF_HOLD)))
    bend = GOLF_BEND + (GOLF_FINISH_BEND - GOLF_BEND) * tall
    pose_rot("hips", bend, 0, -math.pi / 2)
    # The hips turn a little with the shoulders, the legs staying planted.
    hips_turn = 0.45 * turn
    pose_rot("spine", 0, 0, hips_turn * 0.6)
    pose_rot("chest", 0.05, 0, turn - hips_turn * 0.6)
    for side, spread in (("L", -0.1), ("R", 0.1)):
        pose_rot(f"thigh_{side}", -bend - GOLF_KNEE, spread, 0)
        pose_rot(f"shin_{side}", 2 * GOLF_KNEE, 0, 0)
        pose_rot(f"foot_{side}", -GOLF_KNEE, 0, 0)
    # The eyes stay on the ball until it is struck, then follow it towards the target.
    looking = tall
    pose_rot("neck", 0.3 - 0.25 * looking, 0, -0.5 * turn * (1 - looking))
    pose_rot("head", 0.25 - 0.2 * looking, 0, -0.4 * turn * (1 - looking) + 0.5 * looking)
    bpy.context.view_layer.update()
    # The hands, on the club, both laid along it: the left at the top of the grip, the right below.
    target = Vector((1, 0, 0))
    plane = target.cross(golf_address_dir()).normalized()
    face = way.cross(plane)
    pole_l = GOLF_TURN @ Vector((0.3, 0.4, -1.0))
    pole_r = GOLF_TURN @ Vector((-0.3, 0.4, -1.0))
    left = hand
    right = hand + way * GOLF_GRIP_GAP
    w, f = GOLF_TURN @ way, GOLF_TURN @ face
    golf_arm("L", GOLF_TURN @ (left - way * HAND_LEN * 0.5), pole_l, w, f)
    golf_arm("R", GOLF_TURN @ (right - way * HAND_LEN * 0.5), pole_r, w, f)


# One crawl cycle, two pushes, at 60 fps.
CRAWL_FRAMES = 78


# --------------------------------------------------------------------------------------------------
# Swimming. A swimmer's capsule centre floats `SWIM_FLOAT` (0.45 m) under the still surface, so the
# surface is about 1.3 m over the origin: treading water the body is upright with the head out; a
# surface stroke tips it forward about `SWIM_PIVOT`, which keeps the head out; under the water it lies
# flat about `DIVE_PIVOT`, near the capsule centre, which the client tips towards where it swims.
# --------------------------------------------------------------------------------------------------
SWIM_PITCH = 1.25
SWIM_PIVOT = 1.2
DIVE_PITCH = 1.5
DIVE_PIVOT = 0.9
# The right side runs this much of a cycle behind the left: nobody swims quite evenly.
SWIM_SIDES = (("L", 1, 0.0), ("R", -1, 0.025))


def keyed(t, keys):
    """The values at phase `t` along `keys`, `[(phase, (value, ...)), ...]` in rising phase, running on
    from the last back round to the first: a smooth curve through them all (a Hermite spline whose
    speed at each key is the mean of its neighbours'), so nothing stops dead at a key."""
    t %= 1.0
    n = len(keys)

    def key(i):
        turns, k = divmod(i, n)
        return keys[k][0] + turns, keys[k][1]

    def slope(i):
        (ta, a), (tb, b) = key(i - 1), key(i + 1)
        return [(y - x) / (tb - ta) for x, y in zip(a, b)]

    for i in range(-1, n):
        (t0, a), (t1, b) = key(i), key(i + 1)
        if t0 <= t < t1:
            span = t1 - t0
            u = (t - t0) / span
            h00, h10, h01, h11 = 2 * u**3 - 3 * u**2 + 1, u**3 - 2 * u**2 + u, -2 * u**3 + 3 * u**2, u**3 - u**2
            return [h00 * x + h10 * span * m0 + h01 * y + h11 * span * m1 for x, y, m0, m1 in zip(a, b, slope(i), slope(i + 1))]
    return list(keys[0][1])


def pitch_about(angle, pivot, lift=0.0, surge=0.0):
    """Tips the whole body forward by `angle` (rad) about the point `pivot` metres over the origin,
    raised by `lift` and moved `surge` ahead, metres."""
    pose_rot("root", angle, 0, 0)
    pose_loc("root", (0, pivot * math.sin(angle) - surge, pivot * (1 - math.cos(angle)) + lift))


def swim_trunk(hips, spine, chest, roll=0.0, twist=0.0):
    """Bends the trunk (negative arches the back) with a roll and a twist of the chest over the hips.
    Returns how far the chest leans from the root, for `stroke_arm`."""
    pose_rot("hips", hips, 0, 0)
    pose_rot("spine", spine, roll * 0.5, twist * 0.5)
    pose_rot("chest", chest, roll * 0.5, twist * 0.5)
    return hips + spine + chest


def stroke_arm(side, pose, lean=0.0, wrist=0.0):
    """One arm by where it points in the root's frame (+Z to the head, -Y to the belly): `pose` is the
    upper arm's `(out, front, up)` and then the forearm's, out being to the arm's own side, so a wide
    sweep is wide on both. `lean` is the chest's (`swim_trunk`), taken back out, so an arched back
    does not lift the arms out of the water. `wrist` bends the hand."""
    s = 1 if side == "L" else -1
    down = Vector((0, 0, -1))
    level = Matrix.Rotation(-lean, 3, "X")
    u = level @ Vector((s * pose[0], -pose[1], pose[2])).normalized()
    f = level @ Vector((s * pose[3], -pose[4], pose[5])).normalized()
    turn = down.rotation_difference(u).to_matrix()
    pose_mat(f"upper_arm_{side}", turn)
    pose_mat(f"forearm_{side}", turn.inverted() @ down.rotation_difference(f).to_matrix())
    pose_rot(f"hand_{side}", wrist, 0, 0)


def stroke_leg(side, pose):
    """One leg of a frog kick: `pose` is the thigh forward (negative draws the knee up), its spread out
    to the side, the shin bent back at the knee, and the foot pointed (positive) or flexed."""
    s = 1 if side == "L" else -1
    thigh, spread, shin, foot = pose
    pose_rot(f"thigh_{side}", thigh, -s * spread, 0)
    pose_rot(f"shin_{side}", max(0.03, shin), 0, 0)
    pose_rot(f"foot_{side}", foot, 0, 0)


# A breaststroke at the surface, the arms under it all the way: from the glide, hands just under the
# surface, they sweep out wide and down, the elbows bend and the hands come in under the chest, meet
# under the chin and slide forward again as the legs kick; then a glide.
# (phase, (upper out, front, up, forearm out, front, up)) and (phase, (thigh, spread, shin, foot)).
STROKE_ARMS = [
    (0.0, (0.1, 0.56, 0.83, 0.08, 0.52, 0.85)),
    (0.14, (0.3, 0.58, 0.78, 0.35, 0.55, 0.75)),
    (0.3, (0.85, 0.5, 0.45, 0.9, 0.6, 0.3)),
    (0.42, (0.9, 0.6, 0.0, 0.25, 0.95, 0.25)),
    (0.54, (0.4, 0.85, -0.3, -0.35, 0.6, 0.7)),
    (0.7, (0.1, 0.58, 0.82, 0.06, 0.54, 0.85)),
]
STROKE_LEGS = [
    (0.0, (0.0, 0.06, 0.1, 0.6)),
    (0.4, (-0.05, 0.08, 0.15, 0.5)),
    (0.6, (-0.95, 0.35, 1.9, -0.3)),
    (0.72, (-0.4, 0.65, 0.6, -0.3)),
    (0.86, (0.02, 0.1, 0.05, 0.5)),
]
# Under the water the pull sweeps out wide and goes all the way down to the thighs, the hands come
# back up along the chest, and a long glide follows.
DIVE_ARMS = [
    (0.0, (0.1, 0.1, 1.0, 0.08, 0.1, 1.0)),
    (0.18, (0.8, 0.2, 0.6, 0.85, 0.2, 0.5)),
    (0.3, (0.95, 0.3, -0.05, 0.5, 0.6, -0.6)),
    (0.42, (0.25, 0.1, -1.0, 0.15, 0.05, -1.0)),
    (0.6, (0.22, 0.12, -1.0, 0.14, 0.08, -1.0)),
    (0.78, (0.3, 0.8, -0.5, -0.3, 0.5, 0.8)),
    (0.92, (0.1, 0.1, 1.0, 0.08, 0.1, 1.0)),
]
DIVE_LEGS = [
    (0.0, (0.0, 0.06, 0.1, 0.6)),
    (0.6, (-0.03, 0.08, 0.15, 0.55)),
    (0.8, (-0.95, 0.35, 1.9, -0.3)),
    (0.9, (-0.4, 0.65, 0.6, -0.3)),
    (0.98, (0.02, 0.1, 0.05, 0.5)),
]


def swim_tread(t):
    """Treading water upright, the chin at the surface: the arms scull wide out to the sides under
    it, each a little out of step with the other, and the legs turn in an eggbeater."""
    phi = TAU * t
    # Each scull lifts the body a little; the head stays level over a trunk that sways.
    pose_loc("hips", (0.01 * math.sin(phi), 0, -0.08 + 0.025 * math.sin(2 * phi + 0.6)))
    lean = swim_trunk(0.14, -0.04, -0.05 + 0.02 * math.sin(2 * phi), roll=0.04 * math.sin(phi), twist=0.05 * math.sin(phi + 0.8))
    pose_rot("neck", -0.05, -0.02 * math.sin(phi), 0)
    pose_rot("head", -0.1 + 0.02 * math.sin(2 * phi + 1.0), 0, -0.05 * math.sin(phi + 0.8) + 0.06 * math.sin(phi * 1.0 + 2.0))
    for side, s, late in SWIM_SIDES:
        a = 2 * phi - TAU * late * 4
        arm = (0.85, 0.3 + 0.1 * math.sin(a), -0.55 + 0.06 * math.cos(a), 0.55 + 0.4 * math.sin(a - 0.5), 0.75, -0.3 + 0.1 * math.cos(a - 0.5))
        stroke_arm(side, arm, lean, wrist=0.3 * math.cos(a - 0.5))
        # The knee circles: out as it comes up, in as the foot sweeps down and round.
        b = 2 * phi + (0.0 if side == "L" else math.pi)
        pose_rot(f"thigh_{side}", -0.9 + 0.12 * math.sin(b), -s * (0.5 + 0.15 * math.cos(b)), s * 0.25 * math.cos(b))
        pose_rot(f"shin_{side}", 1.4 + 0.45 * math.sin(b + 1.2), 0, 0)
        pose_rot(f"foot_{side}", -0.2 + 0.2 * math.sin(b + 2.0), 0, 0)


def swim_fwd(t):
    """A breaststroke at the surface: nearly flat, the arms under the water, the head held out of it.
    The chest comes up for a breath as the hands sweep in, and the body sinks back into the glide."""
    phi = TAU * t
    breath = (0.5 + 0.5 * math.cos(TAU * (t - 0.5))) ** 1.5
    # The kick drives it forward; the recovery checks it.
    surge = 0.04 * math.sin(TAU * (t - 0.68))
    pitch_about(SWIM_PITCH - 0.2 * breath, SWIM_PIVOT, lift=0.02 * breath, surge=surge)
    lean = swim_trunk(0.05 + 0.1 * breath, -0.12 - 0.08 * breath, -0.15 - 0.1 * breath, roll=0.025 * math.sin(phi), twist=0.02 * math.sin(phi + 1.0))
    # The head stays level while the chest comes up under it.
    pose_rot("neck", -0.35 + 0.12 * breath, 0, 0)
    pose_rot("head", -0.42 + 0.1 * breath, 0, 0.03 * math.sin(phi + 0.5))
    for side, _, late in SWIM_SIDES:
        # The hand trails the arm: bent back on the push forward, cupped on the pull.
        stroke_arm(side, keyed(t - late, STROKE_ARMS), lean, wrist=0.3 * math.sin(TAU * (t - late - 0.3)))
        stroke_leg(side, keyed(t - late * 0.6, STROKE_LEGS))


def dive_idle(t):
    """Hanging flat under the water: the arms scull slowly out wide, the legs drift and flutter, and
    the whole body rocks a little."""
    phi = TAU * t
    pitch_about(DIVE_PITCH - 0.12 + 0.04 * math.sin(phi), DIVE_PIVOT, lift=0.02 * math.sin(phi + 1.0))
    lean = swim_trunk(0.06, -0.05, -0.05 + 0.02 * math.sin(phi), roll=0.05 * math.sin(phi + 0.4), twist=0.03 * math.sin(2 * phi))
    pose_rot("neck", -0.25, 0, 0)
    pose_rot("head", -0.35 + 0.03 * math.sin(phi + 2.0), 0, 0.08 * math.sin(phi + 0.9))
    for side, s, late in SWIM_SIDES:
        a = 2 * phi - TAU * late * 6
        arm = (0.8, 0.35 + 0.08 * math.sin(a), 0.45 + 0.08 * math.cos(a), 0.6 + 0.3 * math.sin(a - 0.6), 0.5, 0.55 + 0.1 * math.cos(a - 0.6))
        stroke_arm(side, arm, lean, wrist=0.25 * math.cos(a - 0.6))
        kick = math.sin(phi + (0.0 if side == "L" else math.pi))
        pose_rot(f"thigh_{side}", 0.08 + 0.12 * kick, -s * 0.1, 0)
        pose_rot(f"shin_{side}", 0.4 - 0.15 * kick, 0, 0)
        pose_rot(f"foot_{side}", 0.5 + 0.1 * kick, 0, 0)


def dive_fwd(t):
    """An underwater breaststroke: flat, the head in line, a wide pull to the thighs, a kick that runs
    through the body like a wave, and a long glide."""
    phi = TAU * t
    # The kick's wave: the hips lead, the chest follows.
    wave = math.sin(TAU * (t - 0.82))
    pitch_about(DIVE_PITCH + 0.05 * math.sin(TAU * (t - 0.3)), DIVE_PIVOT, lift=0.02 * wave, surge=0.05 * math.sin(TAU * (t - 0.6)))
    lean = swim_trunk(0.03 + 0.08 * wave, -0.03 - 0.05 * math.sin(TAU * (t - 0.9)), -0.03 - 0.04 * math.sin(TAU * (t - 0.98)), roll=0.03 * math.sin(phi))
    pose_rot("neck", -0.1, 0, 0)
    pose_rot("head", -0.15 + 0.05 * math.sin(TAU * (t - 0.35)), 0, 0.03 * math.sin(phi + 0.7))
    for side, _, late in SWIM_SIDES:
        stroke_arm(side, keyed(t - late, DIVE_ARMS), lean, wrist=0.3 * math.sin(TAU * (t - late - 0.2)))
        stroke_leg(side, keyed(t - late * 0.6, DIVE_LEGS))


# --------------------------------------------------------------------------------------------------
# Zombies (docs/ZOMBIES.md): the soldier's rig with empty hands, arms out after whoever it smells.
# --------------------------------------------------------------------------------------------------
# One zombie cycle: the left leg steps, the right drags behind it.
ZOMBIE_SHAMBLE = [
    # phase, (left thigh, left shin, right thigh, right shin, hips drop)
    (0.0, (-0.40, 0.15, 0.22, 0.25, 0.00)),
    (0.25, (-0.15, 0.70, 0.12, 0.20, -0.04)),
    (0.5, (0.25, 0.20, -0.18, 0.30, -0.01)),
    (0.75, (0.20, 0.10, -0.05, 0.45, -0.05)),
]


def zombie_arms(t, reach, sway, hunch):
    """Both arms held out in front at `reach` (radians up from hanging, in the world, the trunk's
    `hunch` taken back out), forearms and hands limp, each swaying by `sway` on its own beat."""
    for side, s, late in (("L", 1, 0.0), ("R", -1, 0.37)):
        wave = math.sin(TAU * (t + late))
        pose_rot(f"upper_arm_{side}", -reach - hunch + sway * wave, s * 0.12, 0)
        pose_rot(f"forearm_{side}", 0.22 + 0.08 * wave, 0, 0)
        pose_rot(f"hand_{side}", 0.35, 0, 0)


def zombie_trunk(t, hunch, loll):
    """Hunched forward by `hunch` radians in all, from the hips up, the head lolling over to one side by
    `loll`. A trunk bone turned by +x leans forward, where a hanging arm turned by -x does. The arms
    hang off the chest, so they tip down with it: `zombie_arms` takes the hunch back out."""
    phi = TAU * t
    pose_rot("hips", hunch * 0.2, 0.06 * math.sin(phi), 0.05 * math.sin(phi))
    pose_rot("spine", hunch * 0.3, 0, 0.05 * math.sin(phi))
    pose_rot("chest", hunch * 0.5, 0.04 * math.sin(phi), -0.04 * math.sin(phi))
    pose_rot("neck", -hunch * 0.3, loll, 0)
    pose_rot("head", -hunch * 0.2 + 0.05 * math.sin(phi * 2), loll * 0.6, 0.06 * math.sin(phi))


def zombie_idle(t):
    phi = TAU * t
    pose_loc("hips", (0.03 * math.sin(phi), 0, -0.03))
    zombie_trunk(t, 0.35, 0.3)
    zombie_arms(t, 0.7, 0.12, 0.35)
    for side, s in (("L", 1), ("R", -1)):
        pose_rot(f"thigh_{side}", 0.08, s * 0.1, 0)
        pose_rot(f"shin_{side}", 0.18, 0, 0)
        pose_rot(f"foot_{side}", -0.2, 0, s * 0.2)


def zombie_walk(t):
    lt, ls, rt, rs, drop = keyed(t, ZOMBIE_SHAMBLE)
    phi = TAU * t
    pose_loc("hips", (0.05 * math.sin(phi), 0, drop - 0.03))
    pose_rot("thigh_L", lt, 0.08, 0)
    pose_rot("shin_L", ls, 0, 0)
    pose_rot("foot_L", -(lt + ls) * 0.8, 0, 0)
    # The dragged foot is turned out and scuffs along on its toes.
    pose_rot("thigh_R", rt, -0.1, -0.25)
    pose_rot("shin_R", rs, 0, 0)
    pose_rot("foot_R", 0.25, 0, 0)
    zombie_trunk(t, 0.5, 0.35)
    zombie_arms(t, 1.35, 0.15, 0.5)


# A claw: the right arm goes up and back, rakes down and across, and comes back out in front.
ZOMBIE_CLAW = [
    # phase, (right upper arm up, right forearm, chest hunch, left arm reach)
    (0.0, (1.35, 0.22, 0.5, 1.35)),
    (0.45, (2.75, 0.9, 0.15, 1.1)),
    (0.6, (0.6, 0.1, 0.85, 1.5)),
    (0.8, (0.9, 0.2, 0.7, 1.4)),
]


def zombie_claw(t):
    up, fore, hunch, reach = keyed(t, ZOMBIE_CLAW)
    pose_loc("hips", (0, 0, -0.04))
    zombie_trunk(0.0, hunch, 0.2)
    # The chest twists into the blow.
    pose_rot("chest", hunch * 0.5, 0, -0.25 * math.sin(TAU * t))
    pose_rot("upper_arm_R", -up - hunch, -0.25, 0)
    pose_rot("forearm_R", fore, 0, 0)
    pose_rot("hand_R", -0.3, 0, 0)
    pose_rot("upper_arm_L", -reach - hunch, 0.12, 0)
    pose_rot("forearm_L", 0.25, 0, 0)
    pose_rot("hand_L", 0.35, 0, 0)
    for side, s in (("L", 1), ("R", -1)):
        pose_rot(f"thigh_{side}", -0.2 if side == "L" else 0.2, s * 0.1, 0)
        pose_rot(f"shin_{side}", 0.3, 0, 0)
        pose_rot(f"foot_{side}", -0.1, 0, 0)


# Rising from the grave (`zombie_rise`, played once over `ZOMBIE_RISE_TICKS`): buried upright with the
# head under the ground, it punches the right hand up through the soil, then the left, the head breaks
# the surface, it plants both hands on the ground and heaves itself up and out, slips back, heaves
# again, gets a knee up onto the ground, and stands into the hunch of `zombie_idle`. The root carries
# the body down into the ground, so the client draws it where it stands and the ground hides the rest.
RISE_KEYS = [
    # phase, (root drop, hips, spine, chest, neck, head, left thigh, left shin, right thigh, right shin)
    (0.00, (-2.00, 0.00, 0.00, 0.05, -0.15, -0.25, 0.0, 0.05, 0.0, 0.05)),
    (0.10, (-1.95, 0.05, 0.05, 0.10, -0.25, -0.30, 0.0, 0.05, 0.0, 0.05)),
    (0.22, (-1.80, 0.05, 0.10, 0.15, -0.30, -0.30, 0.0, 0.10, 0.0, 0.10)),
    (0.32, (-1.58, 0.10, 0.15, 0.25, -0.20, -0.20, -0.1, 0.20, 0.0, 0.15)),
    (0.42, (-1.38, 0.25, 0.25, 0.40, -0.35, -0.25, -0.2, 0.30, 0.1, 0.20)),
    (0.54, (-1.02, 0.45, 0.35, 0.55, -0.45, -0.25, -0.5, 0.80, 0.2, 0.30)),
    (0.61, (-1.08, 0.40, 0.30, 0.50, -0.30, -0.20, -0.4, 0.70, 0.2, 0.30)),
    (0.72, (-0.58, 0.55, 0.35, 0.50, -0.45, -0.25, -1.55, 1.90, 0.3, 0.40)),
    (0.85, (-0.16, 0.30, 0.20, 0.30, -0.15, -0.10, -0.55, 0.90, -0.9, 1.50)),
    (1.00, (0.00, 0.07, 0.11, 0.18, -0.11, -0.07, 0.08, 0.18, 0.08, 0.18)),
]
# Each hand by how far it is planted on the ground and how far it is out in front for the shamble,
# the rest of the way reaching up; and how far it has stretched up out of the shoulder.
RISE_HANDS = {
    # phase, (planted, in front, stretched)
    "R": [(0.00, (0.0, 0.0, 0.0)), (0.07, (0.0, 0.0, 1.0)), (0.30, (0.0, 0.0, 1.0)), (0.38, (1.0, 0.0, 1.0)), (0.70, (1.0, 0.0, 1.0)), (0.80, (0.3, 0.7, 1.0)), (0.92, (0.0, 1.0, 1.0)), (1.00, (0.0, 1.0, 1.0))],
    "L": [(0.00, (0.0, 0.0, 0.0)), (0.17, (0.0, 0.0, 0.0)), (0.25, (0.0, 0.0, 1.0)), (0.34, (0.0, 0.0, 1.0)), (0.41, (1.0, 0.0, 1.0)), (0.75, (1.0, 0.0, 1.0)), (0.86, (0.3, 0.7, 1.0)), (1.00, (0.0, 1.0, 1.0))],
}


def track(t, keys):
    """The values at `t` along `keys`, `[(t, (value, ...)), ...]` in rising `t`: a Catmull-Rom curve
    through them that holds still at either end and does not loop, so nothing stops dead in between."""
    i = max(k for k in range(len(keys) - 1) if keys[k][0] <= t) if t < keys[-1][0] else len(keys) - 2
    (t1, a), (t2, b) = keys[i], keys[i + 1]
    u = min(1.0, max(0.0, (t - t1) / (t2 - t1)))
    pre, post = keys[max(i - 1, 0)][1], keys[min(i + 2, len(keys) - 1)][1]
    return [catmull(p0, p1, p2, p3, u) for p0, p1, p2, p3 in zip(pre, a, b, post)]


def zombie_rise(t):
    drop, hips, spine, chest, neck, head, lt, ls, rt, rs = track(t, RISE_KEYS)
    # It struggles: the effort shakes it, harder while it is deep, and the trunk rolls with each pull.
    effort = 1.0 - ease((t - 0.8) / 0.2)
    shake = effort * (0.012 * math.sin(TAU * t * 21) + 0.008 * math.sin(TAU * t * 34 + 1.0))
    pose_loc("root", (0, 0, drop + shake))
    roll = effort * 0.12 * math.sin(TAU * t * 3.5)
    pose_rot("hips", hips, 0.05 * roll, 0)
    pose_rot("spine", spine, roll * 0.5, 0.08 * effort * math.sin(TAU * t * 2.5))
    pose_rot("chest", chest, roll * 0.5, -0.1 * effort * math.sin(TAU * t * 2.5 + 0.7))
    pose_rot("neck", neck, 0.2 + 0.1 * math.sin(TAU * t * 1.5), 0)
    pose_rot("head", head, 0.15, 0.1 * effort * math.sin(TAU * t * 4 + 0.3))
    pose_rot("thigh_L", lt, 0.1, 0)
    pose_rot("shin_L", ls, 0, 0)
    pose_rot("foot_L", -0.2, 0, 0.2)
    pose_rot("thigh_R", rt, -0.1, 0)
    pose_rot("shin_R", rs, 0, 0)
    pose_rot("foot_R", -0.2, 0, -0.2)
    bpy.context.view_layer.update()
    for side, s in (("L", 1), ("R", -1)):
        planted, front, stretched = (min(1.0, max(0.0, v)) for v in track(t, RISE_HANDS[side]))
        up_w = max(0.0, 1.0 - planted - front)
        shoulder = arm_obj.pose.bones[f"upper_arm_{side}"].head.copy()
        # Up: clawing at the air over it, the fingers trembling; it starts bent down by the shoulder.
        tremble = Vector((0.03 * math.sin(TAU * t * 17 + s), 0.03 * math.sin(TAU * t * 13), 0.02 * math.sin(TAU * t * 19)))
        up = shoulder + Vector((s * 0.1, -0.15, -0.25 + 0.85 * stretched)) + tremble * effort
        ground = Vector((s * 0.34, -0.42, 0.03))
        ahead = shoulder + Vector((s * 0.06, -0.48, -0.22))
        wrist = up * up_w + ground * planted + ahead * front
        pole = Vector((s, 0.0, 0.0)) * up_w + Vector((s, 0.8, 0.4)) * planted + Vector((s * 0.6, 0.3, -1.0)) * front
        claw = (Matrix.Rotation(-2.9, 3, "X") @ REST3[f"hand_{side}"]).to_quaternion()
        flat = FLAT_HAND[side].to_quaternion()
        limp = (Matrix.Rotation(-1.2, 3, "X") @ REST3[f"hand_{side}"]).to_quaternion()
        hand = claw.slerp(flat, planted / max(planted + up_w, 1e-6)).slerp(limp, front)
        reach(side, wrist, hand.to_matrix(), pole)


def empty_handed(fn, curl=RELAXED):
    """A clip whose hands hold nothing: the fingers open (or hooked, for `curl` larger) before it poses."""

    def posed(t):
        relax_hands(curl)
        fn(t)

    return posed


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
        bake(f"{prefix}car_out", CAR_OUT_FRAMES, car_out(1), loop=False)
        bake(f"{prefix}car_out_r", CAR_OUT_FRAMES, car_out(-1), loop=False)
        # On skis with the poles stowed: the stance's grip on the skier's legs.
        bake(f"{prefix}armed_glide", 120, armed_glide)
        bake(f"{prefix}armed_skate", 54, armed_skate)
        bake(f"{prefix}armed_climb", 54, armed_climb)
        bake(f"{prefix}armed_tuck", 120, armed_tuck)
        bake(f"{prefix}armed_plow", 120, armed_plow)
        bake(f"{prefix}armed_air", 60, armed_air)
    CURRENT = "rifle"
    # Both hands on the club: whatever stance a club is held in.
    bake("golf_swing", 150, golf_swing, loop=False)
    # A zombie's own: its hands are empty, whatever stance it would have.
    bake("zombie_idle", 120, empty_handed(zombie_idle, 0.7))
    bake("zombie_walk", 60, empty_handed(zombie_walk, 0.7))
    bake("zombie_claw", 42, empty_handed(zombie_claw, 0.85))
    bake("zombie_rise", 240, empty_handed(zombie_rise, 0.6), loop=False)
    # Falling lets go of the item's grip too: shared by every stance.
    bake("freefall", 90, empty_handed(freefall, 0.15))
    bake("chute", 120, empty_handed(chute, 1.3))
    # Up again after being knocked flat: one off the back, one off the belly.
    bake("getup_back", 54, empty_handed(getup_back, 0.1), loop=False)
    bake("getup_front", 54, empty_handed(getup_front, 0.1), loop=False)
    # The arms are on the poles, whatever is carried: every stance's.
    bake("ski_glide", 120, ski_glide)
    bake("ski_stride", 54, ski_stride(0.42, 0.5, 0.5))
    bake("ski_pole", 60, ski_pole(0.46, 0.4, 0.75))
    # Poling hard: the same with everything thrown into it.
    bake("ski_stride_hard", 54, ski_stride(0.62, 0.66, 0.68))
    bake("ski_pole_hard", 60, ski_pole(0.64, 0.62, 1.05))
    bake("ski_climb", 54, ski_climb)
    bake("ski_tuck", 120, ski_tuck)
    bake("ski_plow", 120, ski_plow)
    bake("ski_air", 60, ski_air)
    # In the water the gun is put away: every stance's.
    bake("swim_tread", 72, empty_handed(swim_tread, 0.15))
    bake("swim_fwd", 48, empty_handed(swim_fwd, 0.1))
    bake("dive_idle", 120, empty_handed(dive_idle, 0.15))
    bake("dive_fwd", 60, empty_handed(dive_fwd, 0.1))
    reset_pose()
    arm_obj.animation_data.action = bpy.data.actions["idle"]


build_actions()
anim_action = arm_obj.animation_data.action
arm_obj.animation_data.action = None
print(f"crawl: {crawl_stride() / (CRAWL_FRAMES / FPS):.2f} m/s")
arm_obj.animation_data.action = anim_action


# --------------------------------------------------------------------------------------------------
# Onto the packs' people: every clip retargeted from the source rig onto their skeleton, the guns
# moved onto their right hand, and only they exported.
# --------------------------------------------------------------------------------------------------
if not characters.available():
    sys.exit(f"The character packs are not in {characters.PACKS}: see docs/CHARACTER-LOOKS.md.")

# The source rig's own guns and body give their names up to the target's.
for obj in look_objects + weapon_objects:
    obj.name = "source_" + obj.name
    obj.data.name = "source_" + obj.data.name

# The target's vertex groups, one per bone in its order.
TGROUPS = {name: i for i, name in enumerate(characters.PARENT)}
target, people, JOINTS, skeleton = characters.build(TGROUPS)
people_objects = []
for name, builder in people.items():
    obj, me = make_object(f"body_{name}", builder, target, TGROUPS)
    print(f"{obj.name}: {sum(len(p.vertices) - 2 for p in me.polygons)} tris")
    people_objects.append(obj)

# The source rig in the target's rest pose, a T-pose: arms out level, palms down.
reset_pose()
pose_rot("upper_arm_L", 0, -math.pi / 2, 0)
pose_rot("upper_arm_R", 0, math.pi / 2, 0)
bpy.context.view_layer.update()
SOURCE_OF = {"root": "root", "hips": "hips", "spine": "spine", "chest": "chest", "neck": "neck", "head": "head"}
for _s in ("L", "R"):
    SOURCE_OF.update({f"{p}_{_s}": f"{q}_{_s}" for p, q in (("upper_arm", "upper_arm"), ("forearm", "forearm"), ("hand", "hand"), ("palm", "fingers"), ("thumb", "thumb"), ("thigh", "thigh"), ("shin", "shin"), ("foot", "foot"))})
REFERENCE = {b: (arm_obj.matrix_world @ arm_obj.pose.bones[b].matrix).to_3x3().normalized() for b in SOURCE_OF.values()}
reset_pose()
bpy.context.view_layer.update()
# The target's legs and arms against the source's: the hips move by the one, the hands reach by the other.
LEG_SCALE = JOINTS["thigh_L"].z / HIP_Z
ARM_SCALE = ((JOINTS["forearm_L"] - JOINTS["upper_arm_L"]).length + (JOINTS["hand_L"] - JOINTS["forearm_L"]).length) / (UPPER_ARM + FOREARM)
HIPS_SHIFT = JOINTS["hips"] - Vector((0, 0, HIP_Z)) * LEG_SCALE
print(f"legs x{LEG_SCALE:.3f}, arms x{ARM_SCALE:.3f}")


def place(p):
    return p * LEG_SCALE


retarget_moved = ("root", "hips")
def placed(name, p):
    """A source bone's head in the target's world: the hips with their rest offset, the rest scaled."""
    return place(p) + (HIPS_SHIFT if name == "hips" else Vector())


mover = retarget.Retarget(arm_obj, target, SOURCE_OF, REFERENCE, place=placed, moved=retarget_moved)


def grips():
    """Where the source's hands are against its shoulders, scaled to the target's arms, with the
    elbows' sides as poles: the target's arms are solved onto these."""
    out = {}
    pbs = arm_obj.pose.bones
    for side in ("L", "R"):
        shoulder = (arm_obj.matrix_world @ pbs[f"upper_arm_{side}"].matrix).to_translation()
        elbow = (arm_obj.matrix_world @ pbs[f"forearm_{side}"].matrix).to_translation()
        wrist = (arm_obj.matrix_world @ pbs[f"hand_{side}"].matrix).to_translation()
        out[side] = ((wrist - shoulder) * ARM_SCALE, elbow - (shoulder + wrist) / 2, None)
    return out


# Clips whose hands hold something keep them where the source has them.
FREE_HANDS = {"zombie_idle", "zombie_walk", "zombie_claw", "zombie_rise", "freefall", "chute", "getup_back", "getup_front", "swim_tread", "swim_fwd", "dive_idle", "dive_fwd"}
source_actions = [a for a in bpy.data.actions if a.name in BAKED]
for act in source_actions:
    arm_obj.animation_data.action = act
    start, end = (int(x) for x in act.frame_range)
    out = retarget.new_action(target, "T:" + act.name)
    holding = act.name not in FREE_HANDS and not act.name.startswith("free_")
    for f in range(start, end + 1):
        scene.frame_set(f)
        mover.pose(grips() if holding else None)
        mover.key(f)
    # Read: the source's action is no longer needed.
arm_obj.animation_data.action = None
# Quaternius' Universal Animation Library takes over the clips it has (library.py): standing,
# jogging (the game's 4 m/s walk), sprinting, crouching, and a zombie's standing and walking. With
# empty hands, and for a zombie, the clip is the library's whole. With something held, the library's
# hips and legs carry the stance's own upper body, turned in the world as the stance's clip turns it,
# so the gun stays level and in both hands however far the library's trunk leans. Cycles are turned
# to start where ours do, so the clips still blend in step with the strafes, which stay ours.
# (our clip, the library's, whether every stance has one, how it is fitted). How:
# - `once`: played once through by progress (getting up), not looped;
# - `fit`: a bone whose average place over the clip is moved onto where our own clip has it: the
#   hips for a seat, the head for the water line. The client places those bodies by our clips' numbers.
# - `upper`: the clip of ours whose upper body holds the item, where we have none of that name (a
#   landing has the stance's idle arms);
# - `upright`: our upper body with empty hands too, not the library's: its sitter leans its head 0.2 m
#   forward, into a car's windscreen, from where the seat puts it (`SIT_HEAD_BACK` in the client).
FROM_LIBRARY = [
    ("idle", "Idle_Loop", True, {}),
    ("walk_fwd", "Jog_Fwd_Loop", True, {}),
    ("run_fwd", "Sprint_Loop", True, {}),
    ("crouch_idle", "Crouch_Idle_Loop", True, {}),
    ("crouch_fwd", "Crouch_Fwd_Loop", True, {}),
    ("zombie_idle", "Zombie_Idle_Loop", False, {}),
    ("zombie_walk", "Zombie_Walk_Fwd_Loop", False, {}),
    ("jump_air", "Jump_Loop", True, {}),
    ("sit", "Sitting_Idle_Loop", True, {"fit": "hips", "upright": True}),
    ("swim_tread", "Swim_Idle_Loop", False, {"fit": "head"}),
    ("swim_fwd", "Swim_Fwd_Loop", False, {"fit": "head"}),
    ("getup_back", "LayToIdle", False, {"once": True}),
    ("jump_land", "Jump_Land", True, {"once": True, "upper": "idle"}),
]
# Cycles whose feet are lined up with ours.
STEPPING = {"walk_fwd", "run_fwd", "crouch_fwd", "zombie_walk"}
# What the stance's clip turns when something is held: everything over the hips.
UPPER_BODY = ["pelvis", "spine", "chest", "neck", "head"] + [f"{p}_{s}" for s in ("L", "R") for p in ("clavicle", "upper_arm", "forearm", "hand", "palm", "fingers", "thumb", "thumb2")]
if not library.available():
    sys.exit(f"The Universal Animation Library is not in {library.UAL}: see docs/CHARACTER-LOOKS.md.")
LIBRARY = library.Library()
lib_arm = LIBRARY.armature
LIB_SCALE = JOINTS["thigh_L"].z / LIBRARY.thigh
lib_mover = retarget.Retarget(
    lib_arm, target, {t: s for s, t in library.BONE_MAP.items()}, place=lambda name, p: JOINTS["hips"] + (p - LIBRARY.hips) * LIB_SCALE, moved=("root", "hips")
)
# The root has no bone of the library's, so it is keyed where it rests: unkeyed, Blender would keep
# whatever the clip before left there, and the build would measure the library's clips off it.


def at_phase(obj, act, t, once=False):
    """Poses `obj` by `act` at phase `t` (0..1) of its cycle, or of its once through."""
    retarget.play(obj, act)
    start, end = act.frame_range
    x = start + (min(max(t, 0.0), 1.0) if once else t % 1.0) * (end - start)
    scene.frame_set(int(x), subframe=x - int(x))


def forward_phase(obj, act, foot):
    """The phase at which `foot` (a bone of `obj`) is furthest forward: where the cycles are lined up."""
    best, at = None, 0.0
    for i in range(48):
        at_phase(obj, act, i / 48)
        y = (obj.matrix_world @ obj.pose.bones[foot].matrix).to_translation().y
        if best is None or y < best:
            best, at = y, i / 48
    return at


# How fast each stepping clip goes, m/s: the library's own travel per cycle, scaled to our legs.
LIB_TRAVEL = LIBRARY.travel({theirs for ours, theirs, _, _ in FROM_LIBRARY if ours in STEPPING or ours == "swim_fwd"})
LIB_SPEED = {}


def mean_place(act, bone):
    """Where `bone` of the target is on average over clip `act`, in the world."""
    target.animation_data.action = act
    start, end = (int(x) for x in act.frame_range)
    total = Vector()
    for f in range(start, end + 1):
        scene.frame_set(f)
        total += (target.matrix_world @ target.pose.bones[bone].matrix).to_translation()
    return total / (end - start + 1)


def move_hips(act, by):
    """Moves the hips of clip `act` by `by` (world metres) at every frame, re-keyed."""
    target.animation_data.action = act
    hips = target.pose.bones["hips"]
    start, end = (int(x) for x in act.frame_range)
    for f in range(start, end + 1):
        scene.frame_set(f)
        m = hips.matrix.copy()
        m.translation += by
        hips.matrix = m
        hips.keyframe_insert("location", frame=f, group="hips")


for ours, theirs, stanced, how in FROM_LIBRARY:
    once = how.get("once", False)
    lib_act = LIBRARY.clips[theirs]
    seconds = (lib_act.frame_range[1] - lib_act.frame_range[0]) / scene.render.fps
    frames = max(2, round(seconds * FPS))
    shift = 0.0
    if ours == "swim_fwd":
        print(f"STROKE swim_fwd: {LIB_TRAVEL[theirs] * LIB_SCALE:.2f} m a stroke (the library's)")
    if ours in STEPPING:
        LIB_SPEED[ours] = LIB_TRAVEL[theirs] * LIB_SCALE / seconds
        shift = forward_phase(lib_arm, lib_act, "foot_l") - forward_phase(arm_obj, bpy.data.actions[ours], "foot_L")
    for prefix in STANCE_PREFIX.values() if stanced else ("",):
        name = prefix + ours
        source = bpy.data.actions[prefix + how.get("upper", ours)]
        out = retarget.new_action(target, "P:" + name)
        held = stanced and (how.get("upright", False) or not name.startswith("free_"))
        for f in range(frames + 1):
            t = (f if once else f % frames) / frames
            upper = None
            if held:
                at_phase(arm_obj, source, t, once)
                # Empty hands are retargeted as they are, not solved onto a grip.
                mats = mover.pose(None if name.startswith("free_") else grips())
                upper = {n: retarget.rot3(mats[n]) for n in UPPER_BODY}
            at_phase(lib_arm, lib_act, t + shift, once)
            lib_mover.pose(None, fixed=upper)
            lib_mover.key(f)
        fitted = ""
        if "fit" in how:
            # Our own clip of that name, retargeted, says where the body goes; sideways it stays centred.
            by = mean_place(bpy.data.actions["T:" + name], how["fit"]) - mean_place(out, how["fit"])
            by.x = 0.0
            move_hips(out, by)
            after = mean_place(out, how["fit"]) - mean_place(bpy.data.actions["T:" + name], how["fit"])
            fitted = f", its {how['fit']} moved by ({by.y:+.2f}, {by.z:+.2f}) m, off by ({after.y:+.2f}, {after.z:+.2f})"
        print(f"{name}: from the library's {theirs}, {seconds:.2f} s, shifted {shift:+.2f}{fitted}")
arm_obj.animation_data.action = None
# The source's clips give their names up to the retargeted ones, and the library's take over theirs.
for act in source_actions:
    name = act.name
    bpy.data.actions.remove(act)
    taken = bpy.data.actions.get("P:" + name)
    if taken:
        bpy.data.actions.remove(bpy.data.actions["T:" + name])
        taken.name = name
    else:
        bpy.data.actions["T:" + name].name = name
# The library's clips that are none of ours (a landing) keep their names.
for act in [a for a in bpy.data.actions if a.name.startswith("P:")]:
    act.name = act.name[2:]
# Neither the library nor the packs' own clips are exported.
LIBRARY.remove()
for act in [a for a in bpy.data.actions if "|" in a.name]:
    bpy.data.actions.remove(act)
bpy.data.objects.remove(skeleton.armature, do_unlink=True)


# The people's limbs are not the source rig's, so a body the source laid on the ground floats over it
# or sinks into it once retargeted: an armed crawl, propped on the elbows over the gun, by up to 0.3 m.
# Measured on the soldier's posed body, the crawls are lowered or raised as a whole so the lowest point
# of their cycle touches the ground, and getting up and sliding are raised wherever they would go
# through it. The client stands a lying body's origin on the ground.
GAUGE = next(o for o in people_objects if o.name == "body_soldier_m")


def lowest(act, frame):
    """The lowest point of the soldier's body in clip `act` at `frame`, metres over the origin."""
    target.animation_data.action = act
    scene.frame_set(frame)
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = GAUGE.evaluated_get(depsgraph)
    mesh = evaluated.to_mesh()
    low = min((evaluated.matrix_world @ v.co).z for v in mesh.vertices)
    evaluated.to_mesh_clear()
    return low


def lift_hips(act, lifts):
    """Moves the hips of clip `act` up by `lifts[frame]` metres at each of its frames, re-keyed."""
    target.animation_data.action = act
    hips = target.pose.bones["hips"]
    for frame, up in lifts.items():
        scene.frame_set(frame)
        m = hips.matrix.copy()
        m.translation.z += up
        hips.matrix = m
        hips.keyframe_insert("location", frame=frame, group="hips")


def on_the_ground(name, whole):
    """Puts clip `name` on the ground: as a whole (`whole`, a cycle whose lowest point goes to the
    ground), or frame by frame wherever it is under it."""
    act = bpy.data.actions[name]
    frames = range(int(act.frame_range[0]), int(act.frame_range[1]) + 1)
    lows = {f: lowest(act, f) for f in frames}
    if whole:
        bottom = min(lows.values())
        lift_hips(act, {f: -bottom for f in frames})
        print(f"GROUND {name}: moved {-bottom * 100:+.0f} cm")
    else:
        under = {f: -low for f, low in lows.items() if low < 0}
        lift_hips(act, under)
        print(f"GROUND {name}: raised {len(under)} of {len(lows)} frames, at most {max(under.values(), default=0) * 100:.0f} cm")


for prefix in STANCE_PREFIX.values():
    for clip in ("crawl_idle", "crawl_fwd", "crawl_back", "crawl_left", "crawl_right"):
        on_the_ground(prefix + clip, True)
    on_the_ground(prefix + "slide", False)
for clip in ("getup_back", "getup_front"):
    on_the_ground(clip, False)
target.animation_data.action = None


def ground_speed(name):
    """The speed at which clip `name`'s planted feet slide back under the body, m/s: the ground speed
    at which they do not slide (`SPEED_OF` in client/src/render/character.ts)."""
    act = bpy.data.actions[name]
    target.animation_data.action = act
    start, end = (int(x) for x in act.frame_range)
    speeds = []
    for foot in ("foot_L", "foot_R"):
        track = []
        for f in range(start, end + 1):
            scene.frame_set(f)
            track.append((target.matrix_world @ target.pose.bones[foot].matrix).to_translation())
        low, high = min(p.z for p in track), max(p.z for p in track)
        # The frames this foot is down: in the lowest fifth of its lift.
        for a_, b_ in zip(track, track[1:]):
            if max(a_.z, b_.z) < low + 0.2 * (high - low):
                speeds.append((b_ - a_).xy.length * FPS)
    speeds.sort()
    return speeds[len(speeds) // 2] if speeds else 0.0


for name in ("walk_fwd", "walk_back", "walk_left", "walk_right", "run_fwd", "run_back", "run_left", "run_right", "crouch_fwd", "crouch_back", "crouch_left", "crouch_right", "zombie_walk"):
    if name in LIB_SPEED:
        print(f"SPEED {name}: {LIB_SPEED[name]:.2f} m/s (the library's)")
    else:
        print(f"SPEED {name}: {ground_speed(name):.2f} m/s")
for clip in ("sit", "idle"):
    target.animation_data.action = bpy.data.actions[clip]
    scene.frame_set(0)
    print(f"HEAD {clip}: {(target.matrix_world @ target.pose.bones['head'].matrix).to_translation().z:.3f} m")
target.animation_data.action = None


def target_hand(stance):
    """The target's right hand posed as the stance's idle has it: its world matrix and wrist."""
    target.animation_data.action = bpy.data.actions[STANCE_PREFIX[stance] + "idle"]
    scene.frame_set(0)
    bpy.context.view_layer.update()
    m = target.matrix_world @ target.pose.bones["hand_R"].matrix
    return m, m.to_translation()


# The guns on the target's right hand: each made where the source's hand holds it, moved with the
# wrist to where the target's is, and bound in the hand's rest frame.
target_weapons = []
TMUZZLES = {}
source_wrist = {}
for stance in STANCES:
    reset_pose()
    carry_pose(stance)
    bpy.context.view_layer.update()
    source_wrist[stance] = (arm_obj.matrix_world @ arm_obj.pose.bones["hand_R"].matrix).to_translation()
reset_pose()
for wname, wstance, wparts, muzzle in WEAPONS:
    hand, wrist = target_hand(wstance)
    to_rest = target.data.bones["hand_R"].matrix_local @ hand.inverted() @ Matrix.Translation(wrist - source_wrist[wstance])
    start_mesh()
    B.groups = TGROUPS
    g = STANCES[wstance]["R"]
    for kind, off, size, color in wparts:
        if kind == "box":
            box(g + Vector(off), size, "hand_R", MAT_GEAR, color, transform=to_rest)
        elif kind == "tube":
            back, front, length = size
            c = g + Vector(off)
            prism(c + Vector((0, length / 2, 0)), c - Vector((0, length / 2, 0)), back, front, "hand_R", MAT_GEAR, color, sides=8, transform=to_rest)
        else:
            blob(g + Vector(off), size, "hand_R", MAT_GEAR, color, transform=to_rest)
    obj, _ = make_object(f"weapon_{wname}", B, target, TGROUPS)
    target_weapons.append(obj)
    if muzzle is not None:
        TMUZZLES["muzzle" if wstance == "rifle" else f"muzzle_{wstance}"] = to_rest @ (g + Vector(muzzle))
# Where the left hand holds what the right one does, per stance: a bone on the right hand that the left
# hand lies on in the stance's idle (`grip`, `grip_pistol`, ...). The client pulls the left hand onto it
# (ik.ts), so it stays on the gun whatever bends the arms apart: a blend, a flinch, the aim.
GRIPS = {}
for stance in STANCES:
    target.animation_data.action = bpy.data.actions[STANCE_PREFIX[stance] + "idle"]
    scene.frame_set(0)
    bpy.context.view_layer.update()
    right = target.pose.bones["hand_R"].matrix
    left = target.pose.bones["hand_L"].matrix
    GRIPS["grip" if stance == "rifle" else f"grip_{stance}"] = (stance, target.data.bones["hand_R"].matrix_local @ right.inverted() @ left)


def grip_off(clip, stance):
    """How far clip `clip`'s left hand strays from the stance's grip at worst, metres."""
    act = bpy.data.actions[clip]
    target.animation_data.action = act
    marker = target.data.bones["hand_R"].matrix_local.inverted() @ GRIPS["grip" if stance == "rifle" else f"grip_{stance}"][1]
    worst = 0.0
    for f in range(int(act.frame_range[0]), int(act.frame_range[1]) + 1, 3):
        scene.frame_set(f)
        pbs = target.pose.bones
        want = (pbs["hand_R"].matrix @ marker).to_translation()
        worst = max(worst, (pbs["hand_L"].matrix.to_translation() - want).length)
    return worst


for stance in STANCES:
    prefix = STANCE_PREFIX[stance]
    offs = []
    for act in bpy.data.actions:
        base = act.name[len(prefix):] if prefix else act.name
        if act.name.startswith(prefix) and not any(act.name.startswith(p) for p in STANCE_PREFIX.values() if p and p != prefix) and f"free_{base}" in bpy.data.actions:
            offs.append((grip_off(act.name, stance), act.name))
    print(f"GRIP {stance}: " + ", ".join(f"{name} {off * 100:.0f} cm" for off, name in sorted(offs, reverse=True) if off > 0.03))
target.animation_data.action = None
bpy.context.view_layer.objects.active = target
bpy.ops.object.mode_set(mode="EDIT")
for bname, tip in TMUZZLES.items():
    eb = target.data.edit_bones.new(bname)
    eb.head = tip
    eb.tail = tip + Vector((0, 0, -0.05))
    eb.parent = target.data.edit_bones["hand_R"]
for bname, (stance, m) in GRIPS.items():
    eb = target.data.edit_bones.new(bname)
    eb.head = (0, 0, 0)
    eb.tail = (0, 0.05, 0)
    eb.matrix = m
    eb.parent = target.data.edit_bones["hand_R"]
bpy.ops.object.mode_set(mode="OBJECT")


# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
def export():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    target.select_set(True)
    for obj in people_objects + target_weapons:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = target
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
print("bones:", len(target.data.bones))
print("actions:", sorted(a.name for a in bpy.data.actions))
print("wrote", OUT)


# --------------------------------------------------------------------------------------------------
# Preview renders (optional)
# --------------------------------------------------------------------------------------------------
def golf_preview_club():
    """A club on the right hand for the previews: the shaft along the hand bone, the head at its end."""
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, segments=6, radius1=0.012, radius2=0.012, depth=0.87)
    bmesh.ops.rotate(bm, verts=bm.verts, matrix=Matrix.Rotation(-math.pi / 2, 3, "X"))
    bmesh.ops.translate(bm, verts=bm.verts, vec=Vector((0, 0.355, 0)))
    head = bmesh.ops.create_cube(bm, size=1.0)["verts"]
    bmesh.ops.scale(bm, verts=head, vec=Vector((0.11, 0.05, 0.05)))
    bmesh.ops.translate(bm, verts=head, vec=Vector((0.04, 0.80, -0.01)))
    data = bpy.data.meshes.new("club")
    bm.to_mesh(data)
    obj = bpy.data.objects.new("club", data)
    scene.collection.objects.link(obj)
    obj.parent = arm_obj
    obj.parent_type = "BONE"
    obj.parent_bone = "hand_R"
    # A bone's child hangs from its tail.
    obj.location = (0, -HAND_LEN, 0)
    return obj


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
        if not name.startswith(ONLY):
            return
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
    for clip, frames in (("ski_glide", 1), ("ski_stride", 54), ("ski_pole", 60), ("ski_stride_hard", 54), ("ski_pole_hard", 60), ("ski_climb", 54), ("ski_tuck", 1), ("ski_plow", 1), ("ski_air", 1), ("armed_glide", 1), ("armed_skate", 54), ("armed_climb", 54), ("armed_tuck", 1), ("armed_plow", 1), ("armed_air", 1), ("shoulder_armed_glide", 1), ("pistol_armed_tuck", 1)):
        for i in range(1 if frames == 1 else 4):
            at(clip, i * frames // 4)
            shoot(f"{clip}_{i}", "side")
    for obj in weapon_objects:
        obj.hide_render = obj.name.startswith("weapon_")
    for clip, frames in (("zombie_idle", 120), ("zombie_walk", 60), ("zombie_claw", 42)):
        for i in range(4):
            at(clip, i * frames // 4)
            for view in ("side", "front", "three"):
                shoot(f"{clip}_{i}_{view}", view)
    # Out of the grave, with the ground a slab: what is under it is underground.
    if "zombie_rise".startswith(ONLY) or ONLY.startswith("zombie_rise"):
        bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, -0.005), scale=(3.0, 3.0, 0.01))
        soil = bpy.context.active_object
        views["grave_side"] = (Vector((6, 0, -0.1)), Vector((0, 0, -0.1)))
        views["grave_front"] = (Vector((0, -6, -0.1)), Vector((0, 0, -0.1)))
        views["grave_three"] = (Vector((-4, -5, 2.0)), Vector((0, 0, -0.3)))
        cam_data.ortho_scale = 4.4
        for frame in range(0, 241, 24):
            at("zombie_rise", frame)
            for view in ("grave_side", "grave_front", "grave_three"):
                shoot(f"zombie_rise_{frame:03d}_{view}", view)
        cam_data.ortho_scale = 2.1
        bpy.data.objects.remove(soil)
    for clip, frames in (("walk_fwd", 36), ("walk_right", 36), ("run_fwd", 24), ("run_left", 24)):
        for i in range(4):
            at(clip, i * frames // 4)
            shoot(f"{clip}_{i}", "side" if clip.endswith("fwd") else "front")
    for clip, view in (("slide", "side"), ("slide", "front"), ("crouch_idle", "side"), ("crouch_idle", "three"), ("crouch_fwd", "side"), ("crouch_right", "front"), ("sit", "side"), ("sit", "three"), ("pistol_sit", "side")):
        for i in range(2):
            at(clip, i * (18 if "fwd" in clip or "right" in clip else 60))
            shoot(f"{clip}_{view}_{i}", view)
    # Out of a car: the roof line as a slab over the seat, its edge where the door opening is, and the
    # top of the head through the clip printed against it.
    if "car_out".startswith(ONLY) or ONLY.startswith("car_out"):
        bpy.ops.mesh.primitive_cube_add(size=1.0, location=(-0.25, -0.2, CAR_ROOF + 0.005), scale=(1.1, 1.6, 0.01))
        roof = bpy.context.active_object
        bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, -0.005), scale=(2.0, 2.0, 0.01))
        ground = bpy.context.active_object
        for clip in ("car_out", "car_out_r", "pistol_car_out"):
            tops, feet = [], []
            for frame in range(CAR_OUT_FRAMES + 1):
                at(clip, frame)
                bones = arm_obj.pose.bones
                tops.append(round(bones["head"].tail.z, 2))
                feet.append(round(min(bones[f"foot_{n}"].tail.z for n in ("L", "R")), 2))
            print(f"{clip} head top:", tops)
            print(f"{clip} lowest toe:", feet)
            roof.location.x = -0.25 if not clip.endswith("_r") else 0.25
            for frame in range(0, CAR_OUT_FRAMES + 1, 8):
                at(clip, frame)
                for view in ("side", "front", "three"):
                    shoot(f"{clip}_{frame:02d}_{view}", view)
        bpy.data.objects.remove(roof)
        bpy.data.objects.remove(ground)
    # Lying down in each stance: still from three sides, and a crawl cycle in four steps.
    for stance, item in (("", "ak5"), ("pistol_", "revolver"), ("hold_", "grenade"), ("shoulder_", "carl_gustaf")):
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
            for view in ("side", "three", "front"):
                shoot(f"held_{wname}_{clip}_{view}", view)
    for obj in weapon_objects:
        obj.hide_render = False
    # A swing, a club in hand as the game puts one there (`render/golfclub.ts`): along the hand, its face +Z.
    # Side-on, the golfer faces -X: seen face on, and down the line from behind the ball's flight.
    views["golf_face"] = (Vector((-6, 0, 0.9)), Vector((0, 0, 0.9)))
    views["golf_line"] = (Vector((0, 6, 0.9)), Vector((0, 0, 0.9)))
    club = golf_preview_club()
    for obj in weapon_objects:
        obj.hide_render = True
    for frame in (0, 18, 37, 48, 60, 75, 84, 96, 112, 129, 150):
        at("golf_swing", frame)
        for view in ("golf_face", "golf_line"):
            shoot(f"golf_{frame:03d}_{view}", view)
    for obj in weapon_objects:
        obj.hide_render = False
    bpy.data.objects.remove(club)
    at("jump_air", 10)
    shoot("jump", "side")
    for clip in ("freefall", "chute"):
        for view in ("side", "front", "three"):
            at(clip, 10)
            shoot(f"{clip}_{view}", view)
    # Getting up spans the body's length either way of the feet: framed wider, from the side.
    views["wide"] = (Vector((6, 0, 0.5)), Vector((0, 0, 0.5)))
    cam_data.ortho_scale = 4.4
    for clip in ("getup_back", "getup_front"):
        for frame in (0, 14, 24, 34, 44, 54):
            at(clip, frame)
            shoot(f"{clip}_{frame:02d}", "wide")
    # Swimming, with the still surface where it lies over a floating swimmer's origin (the capsule centre
    # `SWIM_FLOAT` under it, `STAND_CENTRE` over the feet): side on it is a line.
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0, 0, 1.3), scale=(2.0, 2.0, 0.01))
    surface = bpy.context.active_object
    views["swim"] = (Vector((6, 0.2, 1.0)), Vector((0, 0.2, 1.0)))
    views["swim_three"] = (Vector((-4, -5, 3.4)), Vector((0, 0.2, 1.0)))
    # From above and from ahead, for how wide the arms sweep.
    views["swim_top"] = (Vector((0, 0.0, 7.0)), Vector((0, 0.0, 1.0)))
    views["swim_front"] = (Vector((0, -6, 1.2)), Vector((0, 0, 1.2)))
    cam_data.ortho_scale = 3.2
    scene.render.resolution_x, scene.render.resolution_y = 640, 400
    for obj in weapon_objects:
        obj.hide_render = True
    for clip, frames in (("swim_tread", 72), ("swim_fwd", 48), ("dive_idle", 120), ("dive_fwd", 60)):
        for i in range(6):
            at(clip, i * frames // 6)
            shoot(f"{clip}_{i}", "swim")
        surface.hide_render = True
        for i in range(6):
            at(clip, i * frames // 6)
            shoot(f"{clip}_{i}_top", "swim_top")
            shoot(f"{clip}_{i}_front", "swim_front")
        at(clip, 0)
        shoot(f"{clip}_three", "swim_three")
        surface.hide_render = False
    bpy.data.objects.remove(surface)
    for obj in weapon_objects:
        obj.hide_render = False
    scene.render.resolution_x, scene.render.resolution_y = 360, 480
    cam_data.ortho_scale = 2.1


def preview_people(directory):
    """Each body of the packs' people, standing and in the clips that bend it furthest, a gun in hand
    where the clip has one: `<body>_<clip>_<view>.png`. `--only` keeps the shots whose names start so."""
    os.makedirs(directory, exist_ok=True)
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "VERTEX"
    scene.render.resolution_x, scene.render.resolution_y = 360, 480
    for obj in [arm_obj] + look_objects + weapon_objects:
        obj.hide_render = True
    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 2.1
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "front": (Vector((0, -6, 0.9)), Vector((0, 0, 0.9))),
        "side": (Vector((6, 0, 0.9)), Vector((0, 0, 0.9))),
        "three": (Vector((-4, -5, 3.0)), Vector((0, 0, 0.85))),
        "lie": (Vector((-3.5, -4.0, 2.4)), Vector((0, -0.85, 0.2))),
    }
    target.animation_data_create()
    # The water's surface for the swimming clips (`SWIM_DEPTH`, 1.3 m over the origin), seen edge on
    # from the side.
    bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 1.3))
    water = bpy.context.active_object
    water.scale = (0.02, 6, 0.01)
    water.hide_render = True
    # (clip, phase 0..1 through it, view)
    shots = [
        ("idle", 0.0, "front"),
        ("idle", 0.0, "three"),
        ("walk_fwd", 0.25, "side"),
        ("run_fwd", 0.25, "three"),
        ("crouch_fwd", 0.25, "three"),
        ("sit", 0.0, "three"),
        ("car_out", 0.4, "three"),
        ("crawl_fwd", 0.15, "lie"),
        ("zombie_walk", 0.25, "three"),
        ("zombie_claw", 0.5, "three"),
        ("swim_tread", 0.15, "front"),
    ]
    if STRIP:
        shots = [(clip, i / 8, view) for clip in STRIP for view in ("side", "three") for i in range(8)]
    for body_obj in people_objects:
        name = body_obj.name[len("body_"):]
        for obj in people_objects:
            obj.hide_render = obj is not body_obj
        for clip, phase, view in shots:
            shot = f"{name}_{clip}_{view}" + (f"_{round(phase * 8)}" if STRIP else "")
            if not shot.startswith(ONLY):
                continue
            gun = None if clip.startswith(("zombie", "swim", "free_")) else "weapon_ak5"
            for w in target_weapons:
                w.hide_render = w.name != gun
            act = bpy.data.actions[clip]
            target.animation_data.action = act
            x = act.frame_range[0] + phase * (act.frame_range[1] - act.frame_range[0])
            scene.frame_set(int(x), subframe=x - int(x))
            pos, look_at = views[view]
            # A swimmer lies along the water: a wider view.
            cam_data.ortho_scale = 3.4 if clip in ("swim_fwd", "dive_fwd", "dive_idle") else 2.1
            water.hide_render = not (clip.startswith(("swim", "dive")) and view == "side")
            cam.location = pos
            cam.rotation_euler = (look_at - pos).to_track_quat("-Z", "Y").to_euler()
            scene.render.filepath = os.path.join(directory, f"{shot}.png")
            bpy.ops.render.render(write_still=True)


if PREVIEW:
    preview_people(PREVIEW)
# The source rig's own clips, as they are baked before they are retargeted: for working on a clip.
if PREVIEW_SOURCE:
    for obj in people_objects + target_weapons:
        obj.hide_render = True
    for obj in look_objects + weapon_objects:
        obj.hide_render = False
    preview(PREVIEW_SOURCE)
