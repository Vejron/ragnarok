"""
The game's people from Quaternius' Animated Men and Animated Women packs (CC0), downloaded into
`.cache/characters/` (docs/CHARACTER-LOOKS.md): their meshes, recoloured into the game's looks and
bound to one skeleton named as the game names its bones.

`build(groups)` imports every body, builds the target armature from the men's skeleton (scaled
so a body stands 1.72 m, feet at the origin, facing -Y), and returns it with one skinned mesh per
body. The women's meshes are moved onto the men's skeleton by the difference between their rest
bones, which at the same height is a few millimetres.

The packs' colours are flat materials; they become vertex colours in the game's three materials:
`outfit` (white shades, tinted per character), `gear` (final colours) and `skin` (white, tinted with
a skin tone). Uniforms recolour the clothes and add what the pack has not got (a helmet, a vest, a
cap), fitted to the body's own shape.
"""

import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

from parts import MAT_GEAR, MAT_OUTFIT, MAT_SKIN, Builder, bands, chequered, grey, srgb

HERE = os.path.dirname(os.path.abspath(__file__))
PACKS = os.path.normpath(os.path.join(HERE, "../../.cache/characters"))
MEN = os.path.join(PACKS, "Animated-Men-Pack")
WOMEN = os.path.join(PACKS, "Animated-Women-Pack")
HEIGHT = 1.72

# The packs' bones by the game's names. `pelvis`, the clavicles, `palm_*` and `thumb2_*` are extra.
BONE_MAP = {
    "Bone": "root",
    "Body": "hips",
    "Hips": "pelvis",
    "Abdomen": "spine",
    "Torso": "chest",
    "Neck": "neck",
    "Head": "head",
}
for _s in ("L", "R"):
    BONE_MAP.update(
        {
            f"Shoulder.{_s}": f"clavicle_{_s}",
            f"UpperArm.{_s}": f"upper_arm_{_s}",
            f"LowerArm.{_s}": f"forearm_{_s}",
            f"Palm.{_s}": f"hand_{_s}",
            f"MiddleHand.{_s}": f"palm_{_s}",
            f"Fingers.{_s}": f"fingers_{_s}",
            f"Thumb1.{_s}": f"thumb_{_s}",
            f"Thumb2.{_s}": f"thumb2_{_s}",
            f"UpperLeg.{_s}": f"thigh_{_s}",
            f"LowerLeg.{_s}": f"shin_{_s}",
            f"Foot.{_s}": f"foot_{_s}",
        }
    )
# Each target bone's parent: the packs hang the feet off the root (they are IK handles there); here a
# foot hangs off its shin, as a ragdoll and a foot IK want.
PARENT = {
    "root": None,
    "hips": "root",
    "pelvis": "hips",
    "spine": "pelvis",
    "chest": "spine",
    "neck": "chest",
    "head": "neck",
}
for _s in ("L", "R"):
    PARENT.update(
        {
            f"clavicle_{_s}": "chest",
            f"upper_arm_{_s}": f"clavicle_{_s}",
            f"forearm_{_s}": f"upper_arm_{_s}",
            f"hand_{_s}": f"forearm_{_s}",
            f"palm_{_s}": f"hand_{_s}",
            f"fingers_{_s}": f"palm_{_s}",
            f"thumb_{_s}": f"hand_{_s}",
            f"thumb2_{_s}": f"thumb_{_s}",
            f"thigh_{_s}": "hips",
            f"shin_{_s}": f"thigh_{_s}",
            f"foot_{_s}": f"shin_{_s}",
        }
    )
# Where each bone's tail goes: towards this child's head, or along a fixed direction (metres) when it
# has none to point at.
TAIL_TO = {"root": (0, 0, 0.15), "hips": "pelvis", "pelvis": "spine", "spine": "chest", "chest": "neck", "neck": "head", "head": (0, 0, 0.22)}
for _s, _x in (("L", 1), ("R", -1)):
    TAIL_TO.update(
        {
            f"clavicle_{_s}": f"upper_arm_{_s}",
            f"upper_arm_{_s}": f"forearm_{_s}",
            f"forearm_{_s}": f"hand_{_s}",
            f"hand_{_s}": f"palm_{_s}",
            f"palm_{_s}": f"fingers_{_s}",
            f"fingers_{_s}": (_x * 0.07, 0, 0),
            f"thumb_{_s}": f"thumb2_{_s}",
            f"thumb2_{_s}": (_x * 0.03, -0.03, 0),
            f"thigh_{_s}": f"shin_{_s}",
            f"shin_{_s}": f"foot_{_s}",
            f"foot_{_s}": (0, -0.13, 0),
        }
    )
# How high over the ground the ankle is, metres: the packs' foot bone sits at the ground, but a foot
# turns at the ankle.
ANKLE_Z = 0.075


# --------------------------------------------------------------------------------------------------
# Reading a pack's model
# --------------------------------------------------------------------------------------------------
class Model:
    """One imported pack model, read and then removed from the scene: its mesh in the bind pose
    (scaled to `HEIGHT`, feet at the origin), its faces' materials and colours, and its bones' rest
    places."""

    def __init__(self, path):
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=path)
        new = [o for o in bpy.data.objects if o not in before]
        arm = next(o for o in new if o.type == "ARMATURE")
        mesh_obj = next(o for o in new if o.type == "MESH" and o.parent == arm)
        me = mesh_obj.data
        world = mesh_obj.matrix_world.copy()
        points = [world @ v.co for v in me.vertices]
        lo = min(p.z for p in points)
        hi = max(p.z for p in points)
        self.scale = HEIGHT / (hi - lo)
        self.offset = Vector((0, 0, -lo))
        self.points = [self.place(p) for p in points]
        self.weights = [{arm.data.bones.get(mesh_obj.vertex_groups[g.group].name) and BONE_MAP.get(mesh_obj.vertex_groups[g.group].name): g.weight for g in v.groups if g.weight > 1e-4} for v in me.vertices]
        self.weights = [{k: w for k, w in d.items() if k} for d in self.weights]
        self.faces = [(tuple(p.vertices), p.material_index) for p in me.polygons]
        self.materials = []
        for m in me.materials:
            bsdf = m.node_tree.nodes.get("Principled BSDF") if m and m.use_nodes else None
            colour = tuple(bsdf.inputs["Base Color"].default_value) if bsdf else (0.5, 0.5, 0.5, 1.0)
            # A name seen in an earlier import comes back as "Pants.001": the paints know it as "Pants".
            self.materials.append(((m.name if m else "").split(".")[0], colour))
        self.bones = {}
        for b in arm.data.bones:
            name = BONE_MAP.get(b.name)
            if name:
                rest = arm.matrix_world @ b.matrix_local
                self.bones[name] = self.place(rest.to_translation())
        self.actions = [a for a in bpy.data.actions if a not in getattr(Model, "_seen_actions", set())]
        Model._seen_actions = set(bpy.data.actions)
        # Keep the armature (its clips are retargeted from it); remove the rest.
        self.armature = arm
        for o in new:
            if o is not arm:
                bpy.data.objects.remove(o, do_unlink=True)

    def place(self, p):
        """A point of the pack's world, in ours: scaled to `HEIGHT` with the feet at the origin."""
        return (Vector(p) + self.offset) * self.scale


def target_joints(model):
    """The target skeleton's joints from `model`'s bones, with each foot's pivot at the ankle."""
    joints = dict(model.bones)
    for s in ("L", "R"):
        foot = joints[f"foot_{s}"]
        joints[f"foot_{s}"] = Vector((foot.x, foot.y, ANKLE_Z))
    return joints


def build_armature(joints):
    """The target armature: the game's bone names, heads at `joints`, in the packs' T-pose."""
    data = bpy.data.armatures.new("Armature")
    obj = bpy.data.objects.new("Armature", data)
    bpy.context.scene.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    for name in PARENT:
        eb = data.edit_bones.new(name)
        eb.head = joints[name]
        to = TAIL_TO[name]
        eb.tail = joints[to] if isinstance(to, str) else joints[name] + Vector(to)
        if (eb.tail - eb.head).length < 0.01:
            eb.tail = eb.head + Vector((0, 0, 0.05))
        eb.roll = 0.0
    for name, parent in PARENT.items():
        if parent:
            data.edit_bones[name].parent = data.edit_bones[parent]
            data.edit_bones[name].use_connect = False
    bpy.ops.object.mode_set(mode="OBJECT")
    for pb in obj.pose.bones:
        pb.rotation_mode = "QUATERNION"
    return obj


# --------------------------------------------------------------------------------------------------
# The looks: which model each body is made from, and how its materials are painted
# --------------------------------------------------------------------------------------------------
OLIVE, OLIVE_DARK, OLIVE_GEAR, STRAP = srgb(0x56603F), srgb(0x464E34), srgb(0x3D4530), srgb(0x2E3426)
NAVY, NAVY_DARK = srgb(0x1F2B45), srgb(0x18213A)
HIVIS, HIVIS_GREEN, SHINE = srgb(0xDCDD3A), srgb(0xC8DE3C), srgb(0xBAC1C9)
GREEN = srgb(0x2E5A3A)
BLACK = srgb(0x17181B)

# A material's paint: ("outfit", shade), ("gear", colour), ("skin", shade), or None to keep the pack's
# colour in `gear`. Materials not named keep their colour.
SKIN = {"Skin": ("skin", 1.0)}
BODIES = {
    # Soldiers, in olive, with a helmet, a vest, a backpack and the player's colour on the patches.
    "soldier_m": (os.path.join(MEN, "Man_in_Long_Sleeves.glb"), {**SKIN, "Shirt": ("gear", OLIVE), "Pants": ("gear", OLIVE_DARK)}, "soldier"),
    "soldier_f": (os.path.join(WOMEN, "Woman.glb"), {**SKIN, "Jacket": ("gear", OLIVE), "LightJacket": ("gear", OLIVE_DARK), "Shirt": ("gear", OLIVE_DARK), "Pants": ("gear", OLIVE_DARK), "Shoes": ("gear", BLACK)}, "soldier"),
    # Police officers, in navy under a high-visibility vest, with a peaked cap.
    "officer_m": (os.path.join(MEN, "Man.glb"), {**SKIN, "Shirt": ("gear", NAVY), "Shirt2": ("gear", NAVY_DARK), "Pants": ("gear", NAVY_DARK), "Socks": ("gear", BLACK)}, "officer"),
    "officer_f": (os.path.join(WOMEN, "Woman.glb"), {**SKIN, "Jacket": ("gear", NAVY), "LightJacket": ("gear", NAVY_DARK), "Shirt": ("gear", NAVY_DARK), "Pants": ("gear", NAVY_DARK)}, "officer"),
    # Ambulance medics, in a yellow-green high-visibility top and green trousers.
    "medic_m": (os.path.join(MEN, "Man_in_Long_Sleeves.glb"), {**SKIN, "Shirt": ("gear", HIVIS_GREEN), "Pants": ("gear", GREEN)}, "medic"),
    "medic_f": (os.path.join(WOMEN, "Woman_Casual.glb"), {**SKIN, "Shirt": ("gear", HIVIS_GREEN), "Pants": ("gear", GREEN), "Socks": ("gear", GREEN)}, "medic"),
    # Civilians, as the packs dress them, the top tinted per person.
    "civilian_man": (os.path.join(MEN, "Man.glb"), {**SKIN, "Shirt": ("outfit", 1.0), "Shirt2": ("outfit", 0.7)}, None),
    "civilian_sleeves": (os.path.join(MEN, "Man_in_Long_Sleeves.glb"), {**SKIN, "Shirt": ("outfit", 1.0)}, None),
    "civilian_suit": (os.path.join(MEN, "Man_in_Suit.glb"), {**SKIN}, None),
    "civilian_woman": (os.path.join(WOMEN, "Woman.glb"), {**SKIN, "Jacket": ("outfit", 1.0), "LightJacket": ("outfit", 0.75)}, None),
    "civilian_casual": (os.path.join(WOMEN, "Woman_Casual.glb"), {**SKIN, "Shirt": ("outfit", 1.0)}, None),
    "civilian_dress": (os.path.join(WOMEN, "Woman_in_Dress.glb"), {**SKIN, "Dress": ("outfit", 1.0)}, None),
    "civilian_tanktop": (os.path.join(WOMEN, "Woman_in_Tank_Top.glb"), {**SKIN, "Shirt": ("outfit", 1.0)}, None),
}
# The men's skeleton is the shared one.
SKELETON = os.path.join(MEN, "Man.glb")


def paint_of(material, colour, paints):
    """The (material index, rgba) a face of pack material `material` is drawn in."""
    p = paints.get(material)
    if p is None:
        return MAT_GEAR, colour
    kind, value = p
    if kind == "outfit":
        return MAT_OUTFIT, grey(value)
    if kind == "skin":
        return MAT_SKIN, grey(value)
    return MAT_GEAR, value


def dominant(weights):
    return max(weights, key=weights.get) if weights else "root"


def rebind(model, joints):
    """`model`'s points moved onto the shared skeleton: each by its bones' offsets, weighted."""
    out = []
    for p, w in zip(model.points, model.weights):
        total = sum(w.values()) or 1.0
        shift = Vector()
        for bone, weight in w.items():
            if bone in joints and bone in model.bones:
                own = model.bones[bone] if not bone.startswith("foot_") else Vector((model.bones[bone].x, model.bones[bone].y, ANKLE_Z))
                shift += (joints[bone] - own) * (weight / total)
        out.append(p + shift)
    return out


def build_body(name, model, joints, paints, groups):
    """One body as a builder: the model's faces in the game's materials, skinned by its weights."""
    b = Builder(groups)
    bm = b.bm
    points = rebind(model, joints)
    verts = []
    for p, w in zip(points, model.weights):
        v = bm.verts.new(p)
        total = sum(w.values()) or 1.0
        # At most four influences, as the engine skins.
        for bone, weight in sorted(w.items(), key=lambda kv: -kv[1])[:4]:
            v[b.deform][b.group(bone)] = weight / total
        verts.append(v)
    bm.verts.ensure_lookup_table()
    for indices, mi in model.faces:
        try:
            f = bm.faces.new([verts[i] for i in indices])
        except ValueError:
            continue
        material, colour = paint_of(*model.materials[mi], paints)
        f.material_index = material
        f.smooth = True
        for loop in f.loops:
            loop[b.colors] = colour
    b.points = points
    b.weights = model.weights
    return b


# --------------------------------------------------------------------------------------------------
# Fitting accessories to a body
# --------------------------------------------------------------------------------------------------
def section(b, bones, z0, z1):
    """The extent of the body's vertices mostly on `bones` between heights `z0` and `z1`: centre x, y
    and half-widths, or None."""
    pts = [p for p, w in zip(b.points, b.weights) if dominant(w) in bones and z0 <= p.z <= z1]
    if not pts:
        return None
    xs, ys = [p.x for p in pts], [p.y for p in pts]
    return ((max(xs) + min(xs)) / 2, (max(ys) + min(ys)) / 2, (max(xs) - min(xs)) / 2, (max(ys) - min(ys)) / 2)


def shell(b, bones, levels, bone, material, color, grow=0.012, sides=10, square=0.35, slope=0.0, caps=(True, True)):
    """A loft round the body's own section at each height in `levels`, `grow` metres out from it:
    a vest round the chest, a band round an arm, a helmet over the head."""
    rings = []
    for z in levels:
        s = section(b, bones, z - 0.03, z + 0.03)
        if s is None:
            continue
        cx, cy, rx, ry = s
        rings.append((cx, cy, z, rx + grow, ry + grow, square, slope))
    if len(rings) >= 2:
        b.loft(rings, bone, material, color, sides=sides, caps=caps)


def head_box(b):
    """The head's extent above the neck: centre, half-widths and top."""
    pts = [p for p, w in zip(b.points, b.weights) if dominant(w) in ("head",)]
    xs, ys, zs = [p.x for p in pts], [p.y for p in pts], [p.z for p in pts]
    return Vector(((max(xs) + min(xs)) / 2, (max(ys) + min(ys)) / 2, (max(zs) + min(zs)) / 2)), Vector(((max(xs) - min(xs)) / 2, (max(ys) - min(ys)) / 2, (max(zs) - min(zs)) / 2)), max(zs)


def helmet(b, joints):
    c, r, top = head_box(b)
    brow = top - 0.13
    b.loft(
        [
            (c.x, c.y + 0.005, brow, r.x + 0.03, r.y + 0.03, 0.1, -0.3),
            (c.x, c.y + 0.005, brow + 0.015, r.x + 0.024, r.y + 0.024, 0.1, -0.3),
            (c.x, c.y + 0.01, top - 0.04, r.x + 0.012, r.y + 0.012),
            (c.x, c.y + 0.015, top + 0.01, r.x * 0.62, r.y * 0.65),
            (c.x, c.y + 0.015, top + 0.03, r.x * 0.22, r.y * 0.25),
        ],
        "head", MAT_GEAR, srgb(0x4B5437), sides=10,
    )
    # The band in the player's colour, and goggles pushed up on the front.
    b.loft([(c.x, c.y + 0.006, brow + 0.035, r.x + 0.022, r.y + 0.022, 0.0, -0.3), (c.x, c.y + 0.006, brow + 0.058, r.x + 0.019, r.y + 0.019, 0.0, -0.3)], "head", MAT_OUTFIT, grey(1.0), sides=10)
    b.box((c.x, c.y - r.y - 0.025, top - 0.03), (0.12, 0.025, 0.03), "head", MAT_GEAR, srgb(0x6FB8CC))


def cap(b, joints):
    c, r, top = head_box(b)
    band = top - 0.085
    b.loft([(c.x, c.y + 0.004, band, r.x + 0.012, r.y + 0.012, 0.0, -0.2), (c.x, c.y + 0.004, band + 0.016, r.x + 0.012, r.y + 0.012, 0.0, -0.2), (c.x, c.y + 0.004, band + 0.032, r.x + 0.012, r.y + 0.012, 0.0, -0.2)], "head", MAT_GEAR, chequered(BLACK, srgb(0xE8E8E8), band, 0.016, 10), sides=10)
    b.loft([(c.x, c.y + 0.004, band + 0.03, r.x + 0.01, r.y + 0.01, 0.0, -0.2), (c.x, c.y, top + 0.005, r.x + 0.022, r.y + 0.024, 0.2, -0.2), (c.x, c.y - 0.004, top + 0.03, r.x + 0.026, r.y + 0.028, 0.3)], "head", MAT_GEAR, NAVY, sides=10)
    b.box((c.x, c.y - r.y - 0.035, band + 0.02), (0.15, 0.07, 0.012), "head", MAT_GEAR, BLACK)


def chest_levels(joints):
    """Heights through the chest for a vest: from the waist to under the shoulders."""
    lo, hi = joints["spine"].z + 0.03, joints["clavicle_L"].z + 0.02
    return [lo + (hi - lo) * t for t in (0.0, 0.25, 0.5, 0.75, 1.0)]


def accessories(kind, b, joints):
    trunk = ("chest", "spine", "pelvis", "clavicle_L", "clavicle_R")
    if kind == "soldier":
        helmet(b, joints)
        shell(b, trunk, chest_levels(joints), "chest", MAT_GEAR, OLIVE_GEAR, grow=0.018, square=0.6)
        # Magazine pouches on the vest, the backpack over it.
        front = section(b, trunk, joints["spine"].z + 0.05, joints["spine"].z + 0.15)
        if front:
            cx, cy, rx, ry = front
            for dx in (-0.06, 0.0, 0.06):
                b.box((cx + dx, cy - ry - 0.035, joints["spine"].z + 0.1), (0.05, 0.03, 0.085), "chest", MAT_GEAR, STRAP)
            back = section(b, trunk, joints["chest"].z, joints["chest"].z + 0.2)
            if back:
                b.box((back[0], back[1] + back[3] + 0.08, joints["chest"].z + 0.1), (0.24, 0.12, 0.28), "chest", MAT_GEAR, OLIVE_GEAR, taper=(0.9, 0.9))
        # The player's colour on both upper arms.
        for s in ("L", "R"):
            arm = f"upper_arm_{s}"
            mid = (joints[arm] + joints[f"forearm_{s}"]) / 2
            pts = [p for p, w in zip(b.points, b.weights) if dominant(w) == arm and abs(p.x - mid.x) < 0.03]
            if pts:
                ys, zs = [p.y for p in pts], [p.z for p in pts]
                cy, cz, ry, rz = (max(ys) + min(ys)) / 2, (max(zs) + min(zs)) / 2, (max(ys) - min(ys)) / 2, (max(zs) - min(zs)) / 2
                # The arms lie along x in the T-pose: the band is a ring about x.
                b.loft([(mid.x - 0.02, cy, cz, ry + 0.008, rz + 0.008, 0.0), (mid.x + 0.02, cy, cz, ry + 0.008, rz + 0.008, 0.0)], arm, MAT_OUTFIT, grey(1.0), axis="x")
    elif kind == "officer":
        cap(b, joints)
        levels = chest_levels(joints)
        lo, hi = levels[0], levels[-1]
        hivis = bands((0, HIVIS), (lo + (hi - lo) * 0.3, SHINE), (lo + (hi - lo) * 0.38, HIVIS), (lo + (hi - lo) * 0.66, SHINE), (lo + (hi - lo) * 0.74, HIVIS))
        shell(b, trunk, [lo + (hi - lo) * t for t in (0.0, 0.3, 0.38, 0.66, 0.74, 1.0)], "chest", MAT_GEAR, hivis, grow=0.016, square=0.5)
    elif kind == "medic":
        levels = chest_levels(joints)
        lo, hi = levels[0], levels[-1]
        reflect = bands((0, HIVIS_GREEN), (lo + (hi - lo) * 0.3, SHINE), (lo + (hi - lo) * 0.38, HIVIS_GREEN), (lo + (hi - lo) * 0.85, GREEN))
        shell(b, trunk, [lo + (hi - lo) * t for t in (0.0, 0.3, 0.38, 0.85, 1.0)], "chest", MAT_GEAR, reflect, grow=0.008, square=0.4)


# --------------------------------------------------------------------------------------------------
# Building everything
# --------------------------------------------------------------------------------------------------
def available():
    """Whether the packs have been downloaded."""
    return os.path.exists(SKELETON)


def build(groups):
    """Imports the packs. Returns the target armature, {body name: Builder}, the joints, and the
    skeleton model (its armature carries the pack's own clips)."""
    skeleton = Model(SKELETON)
    joints = target_joints(skeleton)
    target = build_armature(joints)
    models = {SKELETON: skeleton}
    bodies = {}
    for name, (path, paints, kind) in BODIES.items():
        if path not in models:
            models[path] = Model(path)
        b = build_body(name, models[path], joints, paints, groups)
        accessories(kind, b, joints)
        bodies[name] = b
    # Only the skeleton's armature is kept; `build_character.py` removes it with the packs' own clips.
    for path, m in models.items():
        if m is not skeleton:
            bpy.data.objects.remove(m.armature, do_unlink=True)
    return target, bodies, joints, skeleton
