"""
Moves clips from one armature to another in Blender (docs/CHARACTER-LOOKS.md): the game's own
procedural clips from the rig `build_character.py` poses, and the Universal Animation Library's
hand-keyed clips (`library.py`), both onto the target skeleton from `characters.py`.

Each target bone takes its source bone's turn away from a reference pose the two rigs share (the
T-pose: the target stands in it at rest, and the source is posed into it), so rigs whose bones lie
differently at rest still move alike. The hips move as the source's do, scaled by how much longer or
shorter the target's legs are. Arms that hold something are then solved onto where the source's
hands are, scaled to the target's arms, so a gun stays in both hands. Bones can also be given a world
rotation outright (`fixed`): the library's legs carrying the upper body of one of our clips.
"""

import math

import bpy
from mathutils import Matrix, Quaternion, Vector


def rot3(m):
    """The rotation of a 4x4 or 3x3 matrix, scale taken out."""
    m3 = m.to_3x3()
    return m3.normalized()


class Rig:
    """A posed armature read in world space: rest and posed matrices per bone."""

    def __init__(self, obj):
        self.obj = obj
        self.world = obj.matrix_world.copy()
        self.rest = {b.name: self.world @ b.matrix_local for b in obj.data.bones}

    def posed(self, name):
        return self.world @ self.obj.pose.bones[name].matrix

    def head(self, name):
        return self.posed(name).to_translation()


class Retarget:
    """Poses `target` (an armature with quaternion bones) from `source`.

    - `mapping` is {target bone: source bone}.
    - `reference` is {source bone: world rotation} in the pose that matches the target's rest (by
      default the source's rest).
    - `place(name, point)` maps target bone `name`'s source head, a world point, to the target's
      world (a scale and an offset).
    - `moved` names the target bones whose location follows their source bone's (the root and the
      hips)."""

    def __init__(self, source, target, mapping, reference=None, place=None, moved=("hips",)):
        self.src = Rig(source)
        self.tgt = Rig(target)
        self.target = target
        self.mapping = mapping
        self.reference = reference or {s: rot3(self.src.rest[s]) for s in mapping.values()}
        self.place = place or (lambda name, p: p)
        self.moved = moved
        # Target bones, parents first, with their rest relative to their parent.
        self.order = []

        def walk(bone):
            self.order.append(bone.name)
            for child in bone.children:
                walk(child)

        for b in target.data.bones:
            if b.parent is None:
                walk(b)
        self.local_rest = {}
        for name in self.order:
            b = target.data.bones[name]
            self.local_rest[name] = (b.parent.matrix_local.inverted() @ b.matrix_local) if b.parent else b.matrix_local.copy()
        self.length = {n: (target.data.bones[n].tail_local - target.data.bones[n].head_local).length for n in self.order}

    def wanted(self, name):
        """The world rotation the source asks of target bone `name`, or None."""
        s = self.mapping.get(name)
        if s is None:
            return None
        return rot3(self.src.posed(s)) @ self.reference[s].inverted() @ rot3(self.tgt.rest[name])

    def pose(self, arms=None, fixed=None):
        """Poses the target from the source as it is posed now, and returns the target's world
        matrices. `arms` is {side: (wrist from the shoulder, elbow pole, hand rotation or None for the
        retargeted one)} in world axes, for arms solved onto a grip, or None. `fixed` is {target bone:
        world rotation} for bones that turn as given whatever the source does (an upper body that
        holds a gun level over another clip's legs), or None."""
        pbs = self.target.pose.bones
        mats = {}
        world = self.tgt.world
        for name in self.order:
            b = self.target.data.bones[name]
            parent = mats[b.parent.name] if b.parent else world
            frame = parent @ self.local_rest[name]
            loc = Vector()
            if name in self.moved and self.mapping.get(name):
                want = self.place(name, self.src.head(self.mapping[name]))
                loc = frame.inverted() @ want
            at = frame @ Matrix.Translation(loc)
            want_rot = fixed[name] if fixed and name in fixed else self.wanted(name)
            side = name[-1] if name[-2:] in ("_L", "_R") else None
            if arms and side in arms and name.startswith(("upper_arm", "forearm", "hand_")):
                want_rot = self.solve_arm(name, side, arms[side], at, mats, want_rot)
            if want_rot is None:
                q = Quaternion()
            else:
                q = (rot3(at).inverted() @ want_rot).to_quaternion()
            pbs[name].location = loc
            pbs[name].rotation_quaternion = q
            mats[name] = at @ q.to_matrix().to_4x4()
        return mats

    def solve_arm(self, name, side, grip, at, mats, retargeted):
        """The world rotation of an arm bone solved so the wrist reaches `grip`'s wrist."""
        reach_to, pole, hand_rot = grip
        if name.startswith("hand_"):
            return hand_rot if hand_rot is not None else retargeted
        upper, fore = f"upper_arm_{side}", f"forearm_{side}"
        a, b = self.length[upper], self.length[fore]
        if name == upper:
            shoulder = at.to_translation()
            wrist = shoulder + reach_to
            self._wrist = wrist
            to = wrist - shoulder
            reach = min(to.length, (a + b) * 0.999)
            d = to.normalized()
            along = (a * a - b * b + reach * reach) / (2 * reach)
            h = math.sqrt(max(a * a - along * along, 0.0))
            out = pole - d * pole.dot(d)
            out = out.normalized() if out.length > 1e-6 else Vector((0, 0, -1))
            elbow = shoulder + d * along + out * h
            self._elbow = elbow
            return self.aim(at, retargeted, elbow - shoulder)
        # The forearm, from the elbow just placed to the wrist.
        return self.aim(at, retargeted, self._wrist - self._elbow)

    @staticmethod
    def aim(at, retargeted, direction):
        """`retargeted` (or the rest, if None) turned the least so the bone's length lies along `direction`."""
        base = retargeted if retargeted is not None else rot3(at)
        y = base.col[1].normalized()
        return y.rotation_difference(direction.normalized()).to_matrix() @ base

    def key(self, frame):
        pbs = self.target.pose.bones
        for name in self.order:
            pbs[name].keyframe_insert("rotation_quaternion", frame=frame, group=name)
            if name in self.moved:
                pbs[name].keyframe_insert("location", frame=frame, group=name)


def play(obj, act):
    """Gives `obj` action `act`, made for another armature or not: an action plays only through a slot
    of its own, and one imported with another armature names that armature's."""
    obj.animation_data.action = act
    if act is not None and act.slots:
        obj.animation_data.action_slot = act.slots[0]


def new_action(target, name):
    act = bpy.data.actions.new(name)
    act.use_fake_user = True
    target.animation_data_create()
    target.animation_data.action = act
    return act
