"""
Quaternius' Universal Animation Library 1 and 2, the free "Standard" tier (CC0): hand-keyed clips on
a mannequin, which `build_character.py` retargets onto the game's people (docs/CHARACTER-LOOKS.md).

itch.io will not hand the zips to a script, so they are downloaded by hand into
`.cache/characters/ual/` as itch.io names them:

    Universal Animation Library[Standard].zip
    Universal Animation Library 2[Standard].zip

The clips without root motion (the mannequin runs on the spot) are read straight out of the zips, and
the ones with it only for how far each cycle goes. Both libraries share one rig, so every clip plays
on the one armature kept from the first.
"""

import os
import zipfile

import bpy
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
UAL = os.path.normpath(os.path.join(HERE, "../../.cache/characters/ual"))
# (zip, its in-place glTF file, the same clips with root motion).
ZIPS = [
    ("Universal Animation Library[Standard].zip", "UAL1_Standard.glb", "UAL1_Standard_RM.glb"),
    ("Universal Animation Library 2[Standard].zip", "UAL2_Standard.glb", "UAL2_Standard_RM.glb"),
]

# The mannequin's bones by the game's names. Its spine has one bone more than ours; `palm_*` has none
# of its own and follows the hand, and the middle finger's first bone curls ours.
BONE_MAP = {
    "pelvis": "hips",
    "spine_01": "pelvis",
    "spine_02": "spine",
    "spine_03": "chest",
    "neck_01": "neck",
    "Head": "head",
}
for _s in ("L", "R"):
    _m = _s.lower()
    BONE_MAP.update(
        {
            f"clavicle_{_m}": f"clavicle_{_s}",
            f"upperarm_{_m}": f"upper_arm_{_s}",
            f"lowerarm_{_m}": f"forearm_{_s}",
            f"hand_{_m}": f"hand_{_s}",
            f"middle_01_{_m}": f"fingers_{_s}",
            f"thumb_01_{_m}": f"thumb_{_s}",
            f"thumb_02_{_m}": f"thumb2_{_s}",
            f"thigh_{_m}": f"thigh_{_s}",
            f"calf_{_m}": f"shin_{_s}",
            f"foot_{_m}": f"foot_{_s}",
        }
    )


def available():
    """Whether both zips have been downloaded."""
    return all(os.path.exists(os.path.join(UAL, z)) for z, _, _ in ZIPS)


def unpacked(zip_name, member):
    """The path of `member` (a file name) taken out of `zip_name`, unpacked next to it once."""
    out = os.path.join(UAL, member)
    if not os.path.exists(out):
        with zipfile.ZipFile(os.path.join(UAL, zip_name)) as z:
            inside = next(n for n in z.namelist() if n.endswith("/" + member) and "Unreal-Godot" in n)
            with open(out, "wb") as f:
                f.write(z.read(inside))
    return out


class Library:
    """Both libraries imported: one armature (`armature`), every clip by its library name (`clips`),
    and how high the mannequin's hips stand at rest (`hips`), for scaling its hips' travel."""

    def __init__(self):
        self.clips = {}
        self.imported = []
        self.armature = None
        for zip_name, member, _ in ZIPS:
            before_objects, before_actions = set(bpy.data.objects), set(bpy.data.actions)
            bpy.ops.import_scene.gltf(filepath=unpacked(zip_name, member))
            new = [o for o in bpy.data.objects if o not in before_objects]
            for act in bpy.data.actions:
                if act not in before_actions:
                    self.imported.append(act)
                    # A clip both libraries have (the T-pose) keeps the first one's.
                    self.clips.setdefault(act.name.split(".")[0], act)
            for o in new:
                if o.type == "ARMATURE" and self.armature is None:
                    self.armature = o
                else:
                    bpy.data.objects.remove(o, do_unlink=True)
        self.armature.animation_data_create()
        self.armature.animation_data.action = None
        world = self.armature.matrix_world
        self.hips = world @ self.armature.data.bones["pelvis"].head_local
        self.thigh = (world @ self.armature.data.bones["thigh_l"].head_local).z

    def travel(self, names):
        """How far the mannequin goes over one cycle of each of clips `names`, metres, read from the
        clips with root motion."""
        out = {}
        for zip_name, _, member in ZIPS:
            before_objects, before_actions = set(bpy.data.objects), set(bpy.data.actions)
            bpy.ops.import_scene.gltf(filepath=unpacked(zip_name, member))
            arm = next(o for o in bpy.data.objects if o not in before_objects and o.type == "ARMATURE")
            arm.animation_data_create()
            scene = bpy.context.scene
            for act in [a for a in bpy.data.actions if a not in before_actions]:
                name = act.name.split(".")[0]
                if name in names and name not in out:
                    arm.animation_data.action = act
                    ends = []
                    for frame in act.frame_range:
                        scene.frame_set(int(frame))
                        ends.append((arm.matrix_world @ arm.pose.bones["root"].matrix).to_translation())
                    out[name] = (ends[1] - ends[0]).length
                bpy.data.actions.remove(act)
            for o in [o for o in bpy.data.objects if o not in before_objects]:
                bpy.data.objects.remove(o, do_unlink=True)
        return out

    def remove(self):
        """Takes the library out of the scene, so none of it is exported."""
        for act in self.imported:
            bpy.data.actions.remove(act)
        bpy.data.objects.remove(self.armature, do_unlink=True)
        self.clips, self.imported = {}, []
        self.armature = None
