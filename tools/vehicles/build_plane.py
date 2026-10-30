"""
Builds the triplane (a Fokker Dr.I in silhouette, stretched into a two-seater) and exports it to
client/public/models/plane.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_plane.py -- \
        [--out client/public/models/plane.glb] [--preview DIR]

Everything is procedural and flat-shaded, a couple of thousand triangles in all, like the other vehicles.

Conventions
  * Blender: Z up, the nose points along +Y, the plane's right hand side is +X. The glTF exporter turns that
    into Y up, nose along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward). A
    game position [x, y, z] is Blender (x, -z, y).
  * Origin is the centre of the fuselage collider (a 1.0 x 1.2 x 5.6 m box, PLANE_HALF_EXTENTS). The wings
    are a second box (PLANE_WING_*). The wheels touch the ground 1.05 m below the origin, where the
    suspension settles. The real Dr.I sat nose-high on its tail skid; this one sits level, as its box does.
  * Nodes the game looks up by name:
      `hull`        everything fixed to the airframe: fuselage, wings, struts, undercarriage, tail
      `prop`        origin on the propeller hub: the game spins it about Z (the nose axis)
      `gun_L`, `gun_R`  the forward guns, origin on each muzzle (PLANE_GUN_PIVOTS): the game slides them back in recoil
      `turret`      the gunner's ring, origin on its pivot (PLANE_REAR_PIVOT): the game yaws it
      `gun`         child of `turret`, origin on the pivot too; the game pitches it and slides it back in recoil
      `elevator`, `aileron_L`, `aileron_R`  origin on their hinge lines: the game turns them about X
      `rudder`      origin on its hinge: the game turns it about Y
      `bomb_1`..`bomb_4`  on the rack under the fuselage: the game hides them as they drop
  * Materials: `paint` is tinted per plane at runtime; `glass` is drawn translucent; `dark` (tyres, guns,
    openings, crosses), `metal` (cowling, fittings), `steel` (struts, hubs), `wood` (propeller, cockpit rim)
    and `white` (the crosses' fields) keep their colours.
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
from common import MATERIALS, about_x, about_y, about_z, arg, export

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/plane.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with PLANE_* in crates/shared/src/vehicles/.
# --------------------------------------------------------------------------------------------------
GROUND = -1.05
GUN_PIVOTS = [Vector((-0.16, 2.0, 0.72)), Vector((0.16, 2.0, 0.72))]  # game [∓0.16, 0.72, -2.0]
REAR_PIVOT = Vector((0.0, -0.7, 0.85))  # game [0, 0.85, 0.7]
REAR_LENGTH = 0.9
BOMB_PIVOT = Vector((0.0, 0.3, -0.85))  # game [0, -0.85, -0.3]
AXLE_Y = 1.5  # game z -1.5
TAIL_Y = -2.4  # game z 2.4
TRACK = 0.8
WHEEL = 0.35
PROP_HUB = Vector((0.0, 2.88, 0.15))
PROP_RADIUS = 1.08
# The three wings: (z of the chord line, y of its leading edge, chord, half span). Staggered so the top one
# is furthest forward, all inside the wing box (y 0.55..1.65, z -0.4..1.3, x ±3.6).
WINGS = [(-0.38, 1.55, 0.95, 2.85), (0.45, 1.6, 0.95, 3.1), (1.22, 1.65, 1.0, 3.55)]
WING_THICK = 0.07

# --------------------------------------------------------------------------------------------------
# Scene and materials
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


def material(name, rgb, alpha=1.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, alpha)
    bsdf.inputs["Roughness"].default_value = 1.0
    bsdf.inputs["Alpha"].default_value = alpha
    m.diffuse_color = (*rgb, alpha)
    return m


MATERIALS[:] = [
    material("paint", (0.55, 0.06, 0.05)),
    material("dark", (0.07, 0.07, 0.07)),
    material("metal", (0.36, 0.36, 0.34)),
    material("steel", (0.55, 0.56, 0.55)),
    material("glass", (0.62, 0.84, 0.92), 0.5),
    material("wood", (0.42, 0.26, 0.12)),
    material("white", (0.92, 0.91, 0.86)),
]
PAINT, DARK, METAL, STEEL, GLASS, WOOD, WHITE = range(7)


class Part(common.Part):

    def box(self, centre, size, mat, rx=0.0, rot=None):
        """A box. `rot` is a 4x4 rotation about its own centre; `rx` is the shorthand for one about X."""
        turn = rot if rot is not None else Matrix.Rotation(rx, 4, "X")
        m = Matrix.Translation(centre) @ turn @ Matrix.Diagonal((*size, 1.0))
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

    def strut(self, a, b, width, mat):
        """A thin square bar from `a` to `b`."""
        a, b = Vector(a), Vector(b)
        along = b - a
        turn = along.to_track_quat("Z", "Y").to_matrix().to_4x4()
        m = Matrix.Translation((a + b) / 2) @ turn @ Matrix.Diagonal((width, width, along.length, 1.0))
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

    def cylinder(self, centre, radius, depth, mat, axis="Z", segments=10, radius2=None, squash=(1.0, 1.0)):
        """A cylinder or frustum (`radius2` is the far end's, towards +axis). `squash` stretches the cross-section."""
        turn = {"Z": Matrix.Identity(4), "X": Matrix.Rotation(math.pi / 2, 4, "Y"), "Y": Matrix.Rotation(-math.pi / 2, 4, "X")}[axis]
        flat = Matrix.Diagonal((squash[0], squash[1], 1.0, 1.0))
        m = Matrix.Translation(centre) @ turn @ flat
        verts = bmesh.ops.create_cone(
            self.bm,
            cap_ends=True,
            cap_tris=False,
            segments=segments,
            radius1=radius,
            radius2=radius if radius2 is None else radius2,
            depth=depth,
            matrix=m,
        )["verts"]
        self._tag(verts, mat)

    def dome(self, centre, scale, mat, floor=0.0, u=12, v=7):
        """The part of an ellipsoid above the horizontal plane `floor` (relative to its centre)."""
        m = Matrix.Translation(centre) @ Matrix.Diagonal((*scale, 1.0))
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=u, v_segments=v, radius=1.0, matrix=m)["verts"]
        below = [vert for vert in verts if vert.co.z < centre[2] + floor * scale[2] - 1e-4]
        bmesh.ops.delete(self.bm, geom=below, context="VERTS")
        self._tag([vert for vert in verts if vert.is_valid], mat)

    def slab(self, outline, z0, z1, mat):
        """Extrudes a closed (x, y) outline along Z, from z0 to z1: a flat plate seen from above."""
        a = [self.bm.verts.new((x, y, z0)) for x, y in outline]
        b = [self.bm.verts.new((x, y, z1)) for x, y in outline]
        faces = [self.bm.faces.new(a[::-1]), self.bm.faces.new(b)]
        for i in range(len(outline)):
            j = (i + 1) % len(outline)
            faces.append(self.bm.faces.new((a[i], a[j], b[j], b[i])))
        for face in faces:
            face.material_index = mat

    def cross(self, centre, size, normal, mat_field=WHITE, mat_cross=DARK):
        """An Iron Cross of WW1: a black cross pattée on a white square, a hair proud of a surface facing
        `normal` ("+Z", "-Z", "+X" or "-X")."""
        axis, sign = normal[1], 1 if normal[0] == "+" else -1
        lift = 0.012 * sign
        bar, thin = size * 0.78, size * 0.22
        if axis == "Z":
            c = Vector(centre) + Vector((0, 0, lift))
            self.box(c, (size, size, 0.008), mat_field)
            c2 = c + Vector((0, 0, 0.006 * sign))
            self.box(c2, (bar, thin, 0.008), mat_cross)
            self.box(c2, (thin, bar, 0.008), mat_cross)
        else:
            c = Vector(centre) + Vector((lift, 0, 0))
            self.box(c, (0.008, size, size), mat_field)
            c2 = c + Vector((0.006 * sign, 0, 0))
            self.box(c2, (0.008, bar, thin), mat_cross)
            self.box(c2, (0.008, thin, bar), mat_cross)


# --------------------------------------------------------------------------------------------------
# Hull: the fuselage behind the round cowling, the three wings and their struts, the undercarriage with
# its little axle wing, the tailplane and the tail skid.
# --------------------------------------------------------------------------------------------------
hull = Part("hull")

# The fuselage: a slab-sided box that tapers to a knife edge at the tail, deep at the front where it meets
# the cowling, its top rising a little to the cockpits. (y, centre x, centre z, radius x, radius z)
hull.loft(
    [
        (2.38, 0.0, 0.12, 0.44, 0.46),
        (1.7, 0.0, 0.12, 0.46, 0.55),
        (0.5, 0.0, 0.12, 0.43, 0.55),
        (-0.8, 0.0, 0.17, 0.34, 0.45),
        (-1.9, 0.0, 0.25, 0.2, 0.32),
        (-2.8, 0.0, 0.32, 0.04, 0.14),
    ],
    PAINT,
    segments=12,
    power=4.0,
)
# Cockpits: openings in the decking with a wooden rim, the pilot's with a little windscreen and a headrest
# fairing behind; the gunner's rim carries the ring (see `turret`).
for y, rim in ((0.45, 0.62), (-0.7, 0.66)):
    hull.cylinder((0.0, y, 0.66), 0.27, 0.04, WOOD, segments=12, squash=(1.0, rim / 0.54))
    hull.cylinder((0.0, y, 0.67), 0.23, 0.04, DARK, segments=12, squash=(1.0, rim / 0.54))
hull.box((0.0, 0.83, 0.75), (0.34, 0.03, 0.16), GLASS, rx=-0.35)
hull.box((0.0, 0.0, 0.7), (0.16, 0.5, 0.12), PAINT)  # decking hump between the cockpits

# The cowling: a round, open-bottomed shroud over the rotary engine, with the cylinders showing through the
# opening, and the engine's crankcase in front behind the propeller.
hull.cylinder((0.0, 2.55, PROP_HUB.z), 0.5, 0.4, METAL, axis="Y", segments=14)
hull.cylinder((0.0, 2.76, PROP_HUB.z), 0.36, 0.04, DARK, axis="Y", segments=14)
for k in range(9):
    a = 2 * math.pi * k / 9
    if math.sin(a) < -0.3:
        hull.cylinder((0.47 * math.cos(a), 2.62, PROP_HUB.z + 0.47 * math.sin(a)), 0.07, 0.16, STEEL, axis="Y", segments=6)
hull.cylinder((0.0, 2.79, PROP_HUB.z), 0.12, 0.08, STEEL, axis="Y", segments=8)

# The wings, with the top one ahead of the other two. Each is a thin plate with a rounded tip; the ailerons
# on the top wing are separate (below). The middle wing passes through the fuselage, the lower one under it.
for z, lead, chord, half in WINGS:
    trail = lead - chord
    top = z == WINGS[2][0]
    span = half - (0.75 if top else 0.0)  # the top wing's ailerons hang past this
    outline = [(-span, lead - 0.08), (-span + 0.12, lead), (span - 0.12, lead), (span, lead - 0.08), (span, trail), (-span, trail)]
    if top:
        hull.slab(outline, z - WING_THICK / 2, z + WING_THICK / 2, PAINT)
        # The centre of the top wing and the root of the aileron bays, between the ailerons.
        for side in (-1, 1):
            hull.slab([(side * span, lead), (side * half, lead - 0.12), (side * half, lead - 0.65), (side * span, lead - 0.65)], z - WING_THICK / 2, z + WING_THICK / 2, PAINT)
    else:
        hull.slab(outline, z - WING_THICK / 2, z + WING_THICK / 2, PAINT)
    # Iron crosses near the tips: on the top of the top wing and the underside of the lowest.
    if top:
        for side in (-1, 1):
            hull.cross((side * (half - 1.05), lead - chord / 2, z + WING_THICK / 2), 0.62, "+Z")
    if z == WINGS[0][0]:
        for side in (-1, 1):
            hull.cross((side * (half - 0.65), lead - chord / 2, z - WING_THICK / 2), 0.58, "-Z")

# Struts: a single broad I-strut out on each side through all three wings, and the cabane struts from the
# fuselage up to the top wing.
for side in (-1, 1):
    x = side * 2.3
    hull.box((x, 1.12, (WINGS[0][0] + WINGS[2][0]) / 2), (0.06, 0.32, WINGS[2][0] - WINGS[0][0]), WOOD)
    for dy in (0.15, -0.15):
        hull.strut((side * 0.32, 1.15 + dy, 0.62), (side * 0.42, 1.2 + dy, WINGS[2][0]), 0.04, STEEL)

# Undercarriage: two V-legs from the fuselage's belly to the axle, the small aerofoil around the axle, and
# the tail skid under the tail.
axle = (AXLE_Y, GROUND + WHEEL)
for side in (-1, 1):
    hull.strut((side * 0.3, 1.9, -0.4), (side * 0.62, axle[0], axle[1]), 0.05, STEEL)
    hull.strut((side * 0.3, 1.0, -0.38), (side * 0.62, axle[0], axle[1]), 0.05, STEEL)
hull.box((0.0, axle[0], axle[1]), (1.3, 0.42, 0.08), PAINT)
hull.box((0.0, axle[0] + 0.12, axle[1] + 0.02), (1.3, 0.16, 0.1), PAINT)
hull.strut((0.0, TAIL_Y + 0.15, 0.05), (0.0, TAIL_Y, GROUND + 0.04), 0.06, WOOD)
hull.box((0.0, TAIL_Y - 0.08, GROUND + 0.04), (0.08, 0.3, 0.06), STEEL)

# The tailplane: a broad flat triangle on the fuselage's tail, its trailing edge the elevator's hinge.
TAIL_HINGE = -2.6
hull.slab([(0.0, -1.75), (1.05, TAIL_HINGE), (-1.05, TAIL_HINGE)], 0.28, 0.32, PAINT)

# Iron crosses on the fuselage's sides, behind the gunner.
for side in (-1, 1):
    hull.cross((side * 0.27, -1.55, 0.3), 0.5, "+X" if side > 0 else "-X")

# Rack for the bombs under the fuselage, between the undercarriage legs and the tail.
hull.box((0.0, BOMB_PIVOT.y, -0.55), (0.5, 0.75, 0.05), METAL)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: wire-spoked, with canvas covers, on the ends of the axle.
# --------------------------------------------------------------------------------------------------
gear = Part("gear", parent=hull_obj)
for side in (-1, 1):
    gear.cylinder((side * TRACK, AXLE_Y, GROUND + WHEEL), WHEEL, 0.12, DARK, axis="X", segments=14)
    gear.cylinder((side * TRACK, AXLE_Y, GROUND + WHEEL), WHEEL * 0.8, 0.14, WHITE, axis="X", segments=12)
    gear.cylinder((side * TRACK, AXLE_Y, GROUND + WHEEL), 0.06, 0.18, STEEL, axis="X", segments=6)
gear_obj = gear.build()

# --------------------------------------------------------------------------------------------------
# Propeller: two laminated wooden blades with a spinner-less hub, turning about the nose axis.
# --------------------------------------------------------------------------------------------------
prop = Part("prop", location=PROP_HUB)
prop.cylinder((0, 0, 0), 0.1, 0.16, STEEL, axis="Y", segments=8)
for k in range(2):
    a = math.pi * k
    turn = about_y(a)
    for start, end, width in ((0.08, 0.5, 0.2), (0.5, PROP_RADIUS - 0.15, 0.17), (PROP_RADIUS - 0.15, PROP_RADIUS, 0.11)):
        c = Vector(((start + end) / 2, 0, 0))
        # A little pitch on the blades, so they read as a propeller and not a plank.
        prop.box(turn @ c, (end - start, 0.05, width), WOOD, rot=turn @ about_x(0.35))
prop_obj = prop.build()

# --------------------------------------------------------------------------------------------------
# The forward guns: two Spandaus on the decking ahead of the pilot, their perforated cooling jackets
# reaching forward to the muzzles over the cowling. Each node's origin is on its muzzle.
# --------------------------------------------------------------------------------------------------
fixed_guns = []
for name, pivot in (("gun_L", GUN_PIVOTS[0]), ("gun_R", GUN_PIVOTS[1])):
    g = Part(name, location=pivot)
    g.box((0.0, -1.15, -0.03), (0.1, 0.42, 0.14), DARK)  # receiver and breech
    g.cylinder((0.0, -0.5, 0.0), 0.05, 0.9, DARK, axis="Y", segments=8)  # cooling jacket
    for y in (-0.8, -0.55, -0.3):
        g.cylinder((0.0, y, 0.0), 0.054, 0.05, METAL, axis="Y", segments=8)  # its ribs
    g.cylinder((0.0, -0.02, 0.0), 0.025, 0.06, METAL, axis="Y", segments=6)  # muzzle
    g.box((0.0, -0.95, -0.12), (0.08, 0.3, 0.08), METAL)  # mount on the decking
    fixed_guns.append(g.build())
    fixed_guns[-1].parent = hull_obj

# --------------------------------------------------------------------------------------------------
# The gunner's ring and Parabellum: the ring (`turret`) yaws all the way round, the gun (`gun`) nods and
# recoils along its own axis. At yaw 0 it points along the nose.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=REAR_PIVOT)
turret.cylinder((0, 0, -0.16), 0.34, 0.05, STEEL, segments=14)  # the ring on the cockpit rim
turret.strut((0.0, -0.1, -0.16), (0.0, 0.0, 0.0), 0.05, STEEL)  # the post up to the cradle
turret.box((0.0, 0.0, 0.0), (0.12, 0.12, 0.08), METAL)  # cradle
turret_obj = turret.build()

ring_gun = Part("gun", parent=turret_obj)
ring_gun.box((0.0, -0.15, 0.0), (0.09, 0.45, 0.12), DARK)  # receiver
ring_gun.cylinder((0.0, 0.25, 0.0), 0.04, 0.5, DARK, axis="Y", segments=8)  # jacket
ring_gun.cylinder((0.0, REAR_LENGTH - 0.07, 0.0), 0.018, 0.14, METAL, axis="Y", segments=6)  # muzzle
ring_gun.cylinder((0.1, -0.05, -0.02), 0.08, 0.06, METAL, axis="X", segments=8)  # ammunition drum
ring_gun.box((0.0, -0.4, -0.04), (0.04, 0.16, 0.1), WOOD)  # grip
ring_gun_obj = ring_gun.build()

# --------------------------------------------------------------------------------------------------
# Control surfaces. The ailerons hang off the top wing's outer trailing edges past its tips (the Dr.I's
# balanced "ears"), the elevator spans the tailplane's trailing edge, and the comma rudder stands on the
# tail with no fin in front of it.
# --------------------------------------------------------------------------------------------------
z_top, lead_top, chord_top, half_top = WINGS[2]
hinge_y = lead_top - 0.65
ailerons = []
for name, side in (("aileron_L", -1), ("aileron_R", 1)):
    root = side * (half_top - 0.75)
    hinge = Vector((root + side * 0.4, hinge_y, z_top))
    a = Part(name, location=hinge)
    # Outline relative to the hinge, from the root out to the overhanging tip.
    xs = [root - hinge.x, side * (half_top + 0.05) - hinge.x]
    a.slab(
        [(xs[0], 0.0), (xs[1], 0.0), (xs[1], -0.3), (xs[1] - side * 0.1, -0.38), (xs[0], -0.38)],
        -WING_THICK / 2.5,
        WING_THICK / 2.5,
        PAINT,
    )
    ailerons.append(a.build())
    ailerons[-1].parent = hull_obj

elevator = Part("elevator", location=(0.0, TAIL_HINGE, 0.3))
elevator.slab([(-1.1, 0.0), (1.1, 0.0), (1.0, -0.32), (0.25, -0.4), (-0.25, -0.4), (-1.0, -0.32)], -0.02, 0.02, PAINT)
elevator_obj = elevator.build()
elevator_obj.parent = hull_obj

# The rudder: a comma, broad at the top and sweeping down to the tail, white with a black cross.
rudder = Part("rudder", location=(0.0, -2.78, 0.3))
comma = [(0.0, -0.12), (0.0, 0.9), (-0.2, 1.0), (-0.45, 0.95), (-0.6, 0.75), (-0.6, 0.4), (-0.42, 0.12), (-0.25, 0.0)]
rudder.prism(comma, -0.025, 0.025, WHITE)
for side in (-1, 1):
    rudder.cross((side * 0.025, -0.32, 0.55), 0.42, "+X" if side > 0 else "-X", mat_field=WHITE)
rudder_obj = rudder.build()
rudder_obj.parent = hull_obj

# --------------------------------------------------------------------------------------------------
# Bombs: four small ones hung nose forward on the rack, two by two.
# --------------------------------------------------------------------------------------------------
bombs = []
for k, (dx, dy) in enumerate(((-0.14, 0.22), (0.14, 0.22), (-0.14, -0.22), (0.14, -0.22))):
    b = Part(f"bomb_{k + 1}", location=BOMB_PIVOT + Vector((dx, dy, 0.18)))
    b.cylinder((0, 0, 0), 0.075, 0.34, DARK, axis="Y", segments=8)
    b.cylinder((0, 0.22, 0), 0.075, 0.1, DARK, axis="Y", segments=8, radius2=0.02)
    b.cylinder((0, -0.22, 0), 0.075, 0.1, DARK, axis="Y", segments=8, radius2=0.03)
    b.box((0, -0.3, 0), (0.18, 0.08, 0.012), METAL)
    b.box((0, -0.3, 0), (0.012, 0.08, 0.18), METAL)
    bombs.append(b.build())
    bombs[-1].parent = hull_obj


# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, gear_obj, prop_obj, turret_obj, ring_gun_obj, elevator_obj, rudder_obj, *fixed_guns, *ailerons, *bombs]
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
    mid = Vector((0, 0.0, 0.1))
    views = {
        "side": ("ORTHO", Vector((14, 0.0, 0.1)), mid, 7.5),
        "top": ("ORTHO", Vector((0, 0.0, 14)), mid, 8.5),
        "front": ("ORTHO", Vector((0, 14, 0.1)), mid, 8.5),
        "three": ("PERSP", Vector((7.5, 8.0, 3.5)), mid, 0),
        "rear": ("PERSP", Vector((-6.5, -9.0, 3.5)), mid, 0),
        "below": ("PERSP", Vector((5.0, 4.0, -3.0)), mid, 0),
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
