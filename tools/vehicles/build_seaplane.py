"""
Builds the seaplane (a two-seat biplane of the 1920s on twin pontoons, with small wheels beside them)
and exports it to client/public/models/seaplane.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_seaplane.py -- \
        [--out client/public/models/seaplane.glb] [--preview DIR]

Everything is procedural and flat-shaded, a couple of thousand triangles in all, like the other vehicles.

Conventions
  * Blender: Z up, the nose points along +Y, the plane's right hand side is +X. The glTF exporter turns that
    into Y up, nose along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward). A
    game position [x, y, z] is Blender (x, -z, y).
  * Origin is the centre of the fuselage collider (a 1.0 x 1.2 x 6.0 m box, SEAPLANE_HALF_EXTENTS). The
    wings are a second box (SEAPLANE_WING_*) and each pontoon a third (SEAPLANE_FLOAT_*), their undersides
    SEAPLANE_KEEL (1.55 m) below the origin. On a road its wheels touch the ground SEAPLANE_RIDE_HEIGHT
    (1.71 m) below the origin; afloat the water is SEAPLANE_DRAFT (0.25 m) up the pontoons.
  * Nodes the game looks up by name are the triplane's (`client/src/render/plane.ts` draws both):
      `hull`        everything fixed to the airframe: fuselage, wings, struts, pontoons, tail
      `prop`        origin on the propeller hub: the game spins it about Z (the nose axis)
      `gun_L`, `gun_R`  the forward guns, origin on each muzzle (SEAPLANE_GUN_PIVOTS): the game slides them back in recoil
      `turret`      the gunner's ring, origin on its pivot (SEAPLANE_REAR_PIVOT): the game yaws it
      `gun`         child of `turret`, origin on the pivot too; the game pitches it and slides it back in recoil
      `elevator`, `aileron_L`, `aileron_R`  origin on their hinge lines: the game turns them about X
      `rudder`      origin on its hinge: the game turns it about Y
      `bomb_1`..`bomb_4`  on the rack under the fuselage, between the pontoons: the game hides them as they drop
  * Materials: `paint` is tinted per plane at runtime; `glass` is drawn translucent; `dark` (tyres, guns,
    openings), `metal` (cowling, fittings), `steel` (struts, the pontoons' aluminium), `wood` (propeller,
    cockpit rims) and `white` (the trim) keep their colours.
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
from common import MATERIALS, about_x, about_y, arg, export

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/seaplane.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with SEAPLANE_* in crates/shared/src/vehicles/seaplane.rs.
# --------------------------------------------------------------------------------------------------
GROUND = -1.71
KEEL = -1.55
GUN_PIVOTS = [Vector((-0.16, 2.2, 0.72)), Vector((0.16, 2.2, 0.72))]  # game [∓0.16, 0.72, -2.2]
REAR_PIVOT = Vector((0.0, -0.9, 0.85))  # game [0, 0.85, 0.9]
REAR_LENGTH = 0.9
BOMB_PIVOT = Vector((0.0, 0.3, -0.85))  # game [0, -0.85, -0.3]
PILOT_Y = 0.3  # game z -0.3
AXLES_Y = (1.4, -1.6)  # game z -1.4 and 1.6
TRACK = 1.1
WHEEL = 0.25
PROP_HUB = Vector((0.0, 3.1, 0.12))
PROP_RADIUS = 1.2
# The two wings: (z of the chord line, y of its leading edge, chord, half span). The upper one is the
# longer and sits a little ahead, both inside the wing box (y 0.4..1.6, z -0.4..1.3, x ±4.4).
LOWER = (-0.33, 1.5, 1.1, 3.85)
UPPER = (1.22, 1.6, 1.2, 4.35)
WING_THICK = 0.08
# Each pontoon's box: y -1.9..2.7, z -1.55..-0.95, 0.6 m wide about x = ±TRACK.
FLOAT_TOP = -0.95

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
    material("paint", (0.37, 0.47, 0.53)),
    material("dark", (0.07, 0.07, 0.07)),
    material("metal", (0.36, 0.36, 0.34)),
    material("steel", (0.62, 0.64, 0.64)),
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

    def strut(self, a, b, width, mat, chord=None):
        """A bar from `a` to `b`, `width` across and `chord` (or `width`) fore and aft: a streamlined strut."""
        a, b = Vector(a), Vector(b)
        along = b - a
        turn = along.to_track_quat("Z", "Y").to_matrix().to_4x4()
        m = Matrix.Translation((a + b) / 2) @ turn @ Matrix.Diagonal((width, chord or width, along.length, 1.0))
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

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


# --------------------------------------------------------------------------------------------------
# Hull: the fuselage behind the radial engine, the two wings and their struts, the pontoons on their
# struts, the fin and the tailplane.
# --------------------------------------------------------------------------------------------------
hull = Part("hull")

# The fuselage: round at the engine, slab-sided through the cockpits and tapering to the sternpost.
# (y, centre x, centre z, radius x, radius z)
hull.loft(
    [
        (2.6, 0.0, 0.12, 0.44, 0.46),
        (1.8, 0.0, 0.12, 0.47, 0.56),
        (0.4, 0.0, 0.12, 0.45, 0.56),
        (-1.1, 0.0, 0.16, 0.38, 0.48),
        (-2.2, 0.0, 0.24, 0.22, 0.34),
        (-3.0, 0.0, 0.3, 0.05, 0.2),
    ],
    PAINT,
    segments=12,
    power=3.5,
)
# A white band round the fuselage behind the gunner, and a white cheat line along each side.
hull.box((0.0, -1.75, 0.2), (0.6, 0.3, 0.86), WHITE)
for side in (-1, 1):
    hull.box((side * 0.455, 0.9, 0.2), (0.012, 2.6, 0.07), WHITE)

# Cockpits: openings in the decking with a wooden rim, the pilot's with a windscreen; the gunner's rim
# carries the ring (see `turret`).
for y, rim in ((PILOT_Y, 0.62), (REAR_PIVOT.y, 0.66)):
    hull.cylinder((0.0, y, 0.67), 0.27, 0.04, WOOD, segments=12, squash=(1.0, rim / 0.54))
    hull.cylinder((0.0, y, 0.68), 0.23, 0.04, DARK, segments=12, squash=(1.0, rim / 0.54))
hull.box((0.0, PILOT_Y + 0.4, 0.78), (0.4, 0.03, 0.2), GLASS, rx=-0.4)
hull.box((0.0, (PILOT_Y + REAR_PIVOT.y) / 2, 0.7), (0.16, 0.5, 0.1), PAINT)  # decking hump between the cockpits

# The engine: a nine-cylinder radial in the open, its cylinders standing out past a narrow cowling ring,
# the crankcase in front behind the propeller, and an exhaust pipe down the right side.
hull.cylinder((0.0, 2.72, PROP_HUB.z), 0.4, 0.26, METAL, axis="Y", segments=14)
for k in range(9):
    a = 2 * math.pi * k / 9 + math.pi / 2
    hull.cylinder((0.47 * math.cos(a), 2.78, PROP_HUB.z + 0.47 * math.sin(a)), 0.085, 0.2, DARK, axis="Y", segments=6)
hull.cylinder((0.0, 2.92, PROP_HUB.z), 0.2, 0.18, STEEL, axis="Y", segments=10)
hull.strut((0.5, 2.6, -0.1), (0.52, 1.3, -0.3), 0.07, DARK)

# The wings: thin plates with rounded tips. The upper one's ailerons are separate (below).
for z, lead, chord, half in (LOWER, UPPER):
    trail = lead - chord
    upper = z == UPPER[0]
    span = half - (1.3 if upper else 0.0)  # the upper wing's ailerons hang outboard of this, behind the spar
    # Rounded tips on the lower wing; the upper one's centre section ends square, where its outer panels join it.
    round_ = 0.0 if upper else 0.15
    outline = [(-span, lead - round_ * 0.8), (-span + round_, lead), (span - round_, lead), (span, lead - round_ * 0.8), (span, trail), (-span, trail)]
    hull.slab(outline, z - WING_THICK / 2, z + WING_THICK / 2, PAINT)
    if upper:
        # Outboard, the wing ahead of the ailerons, out to the rounded tip.
        for side in (-1, 1):
            hull.slab(
                [(side * span, lead), (side * (half - 0.15), lead), (side * half, lead - 0.15), (side * half, lead - 0.8), (side * span, lead - 0.8)][:: side],
                z - WING_THICK / 2,
                z + WING_THICK / 2,
                PAINT,
            )
        # White tips on the upper wing, seen from above.
        for side in (-1, 1):
            hull.box((side * (half - 0.5), lead - 0.42, z + WING_THICK / 2 + 0.006), (0.5, 0.7, 0.01), WHITE)

# Struts. Two bays of interplane struts on each side, each a pair leaning with the stagger, the cabane
# struts from the fuselage up to the upper wing, and a pair of bracing wires' worth of steel across each bay.
for side in (-1, 1):
    for x in (1.9, 3.3):
        for dy in (0.3, -0.5):
            hull.strut((side * x, LOWER[1] - 0.55 + dy, LOWER[0]), (side * x, UPPER[1] - 0.6 + dy, UPPER[0]), 0.045, WOOD, chord=0.12)
    for dy in (0.3, -0.4):
        hull.strut((side * 0.36, 1.0 + dy, 0.6), (side * 0.5, 1.05 + dy, UPPER[0]), 0.04, STEEL, chord=0.09)

# The pontoons: long aluminium floats with an upswept bow, a flat planing bottom with a step, and a
# tapering tail. Each stands on four struts from the fuselage and the lower wing, with spreader bars across.
for side in (-1, 1):
    x = side * TRACK
    hull.loft(
        [
            (2.7, x, -1.02, 0.05, 0.05),
            (2.35, x, -1.15, 0.22, 0.18),
            (1.5, x, -1.25, 0.3, 0.3),
            (-0.1, x, -1.25, 0.3, 0.3),
            (-0.12, x, -1.2, 0.29, 0.25),
            (-1.2, x, -1.14, 0.2, 0.18),
            (-1.9, x, -1.06, 0.06, 0.08),
        ],
        STEEL,
        segments=8,
        power=5.0,
    )
    hull.box((x, 0.7, FLOAT_TOP + 0.005), (0.2, 2.6, 0.02), PAINT)  # walkway down the deck
    hull.box((x, 0.7, KEEL + 0.01), (0.05, 1.6, 0.05), DARK)  # keel strip under the planing bottom
    for y, up in ((1.5, (side * 0.38, 1.7, -0.4)), (0.0, (side * 0.38, 0.3, -0.4)), (1.5, (side * 1.5, LOWER[1] - 0.3, LOWER[0])), (0.0, (side * 1.5, LOWER[1] - 0.9, LOWER[0]))):
        hull.strut((x, y, FLOAT_TOP), up, 0.045, STEEL, chord=0.11)
for y in (1.5, 0.0):
    hull.strut((-TRACK, y, FLOAT_TOP + 0.03), (TRACK, y, FLOAT_TOP + 0.03), 0.05, STEEL, chord=0.1)

# The tail: a fixed fin ahead of the rudder, and the tailplane, its trailing edge the elevator's hinge.
TAIL_HINGE = -2.75
hull.prism([(-1.7, 0.5), (-2.4, 1.25), (TAIL_HINGE, 1.3), (TAIL_HINGE, 0.45)], -0.025, 0.025, PAINT)
hull.slab([(0.0, -1.8), (0.9, -2.15), (1.3, TAIL_HINGE), (-1.3, TAIL_HINGE), (-0.9, -2.15)], 0.36, 0.41, PAINT)
for side in (-1, 1):
    hull.strut((side * 0.9, -2.5, 0.37), (side * 0.12, -2.5, -0.02), 0.03, STEEL)

# Rack for the bombs under the fuselage, between the pontoons.
hull.box((0.0, BOMB_PIVOT.y, -0.55), (0.5, 0.75, 0.05), METAL)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: a small one on a stub axle outboard of each pontoon ahead of the step and another near its tail,
# their tyres reaching a little below the keel.
# --------------------------------------------------------------------------------------------------
gear = Part("gear", parent=hull_obj)
for side in (-1, 1):
    for y in AXLES_Y:
        gear.cylinder((side * (TRACK + 0.33), y, GROUND + WHEEL), WHEEL, 0.1, DARK, axis="X", segments=12)
        gear.cylinder((side * (TRACK + 0.34), y, GROUND + WHEEL), WHEEL * 0.55, 0.11, WHITE, axis="X", segments=8)
        gear.strut((side * TRACK, y, GROUND + WHEEL), (side * (TRACK + 0.36), y, GROUND + WHEEL), 0.06, STEEL)
        gear.strut((side * (TRACK + 0.2), y, FLOAT_TOP - 0.1), (side * (TRACK + 0.27), y, GROUND + WHEEL), 0.05, STEEL)
gear_obj = gear.build()

# --------------------------------------------------------------------------------------------------
# Propeller: two laminated wooden blades on a steel hub, turning about the nose axis.
# --------------------------------------------------------------------------------------------------
prop = Part("prop", location=PROP_HUB)
prop.cylinder((0, 0, 0), 0.1, 0.18, STEEL, axis="Y", segments=8)
for k in range(2):
    turn = about_y(math.pi * k)
    for start, end, width in ((0.08, 0.55, 0.21), (0.55, PROP_RADIUS - 0.18, 0.18), (PROP_RADIUS - 0.18, PROP_RADIUS, 0.11)):
        c = Vector(((start + end) / 2, 0, 0))
        # A little pitch on the blades, so they read as a propeller and not a plank.
        prop.box(turn @ c, (end - start, 0.05, width), WOOD, rot=turn @ about_x(0.35))
prop_obj = prop.build()

# --------------------------------------------------------------------------------------------------
# The forward guns: two on the decking ahead of the pilot, their cooling jackets reaching forward to the
# muzzles behind the engine. Each node's origin is on its muzzle.
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
# The gunner's ring and its gun: the ring (`turret`) yaws all the way round, the gun (`gun`) nods and
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
ring_gun.cylinder((0.0, -0.05, 0.1), 0.11, 0.05, METAL, segments=8)  # pan magazine on top
ring_gun.box((0.0, -0.4, -0.04), (0.04, 0.16, 0.1), WOOD)  # grip
ring_gun_obj = ring_gun.build()

# --------------------------------------------------------------------------------------------------
# Control surfaces. The ailerons are the outer trailing edges of the upper wing, the elevator spans the
# tailplane's trailing edge, and the rudder hangs behind the fin.
# --------------------------------------------------------------------------------------------------
z_top, lead_top, chord_top, half_top = UPPER
hinge_y = lead_top - 0.8
ailerons = []
for name, side in (("aileron_L", -1), ("aileron_R", 1)):
    root = side * (half_top - 1.3)
    hinge = Vector((root + side * 0.65, hinge_y, z_top))
    a = Part(name, location=hinge)
    # Outline relative to the hinge, from the root out to the tip.
    xs = [root - hinge.x, side * half_top - hinge.x]
    depth = chord_top - 0.8
    a.slab([(xs[0], 0.0), (xs[1], 0.0), (xs[1] - side * 0.12, -depth), (xs[0], -depth)][:: side], -WING_THICK / 2.5, WING_THICK / 2.5, PAINT)
    ailerons.append(a.build())
    ailerons[-1].parent = hull_obj

elevator = Part("elevator", location=(0.0, TAIL_HINGE, 0.385))
elevator.slab([(-1.3, 0.0), (-0.12, 0.0), (-0.12, -0.3), (-1.15, -0.4)], -0.02, 0.02, PAINT)
elevator.slab([(0.12, 0.0), (1.3, 0.0), (1.15, -0.4), (0.12, -0.3)], -0.02, 0.02, PAINT)
elevator_obj = elevator.build()
elevator_obj.parent = hull_obj

# The rudder: rounded at the top, running down past the sternpost, white with a painted stripe.
rudder = Part("rudder", location=(0.0, TAIL_HINGE, 0.3))
rudder.prism([(0.0, -0.2), (0.0, 1.0), (-0.2, 1.12), (-0.45, 1.05), (-0.58, 0.8), (-0.58, 0.3), (-0.4, 0.0), (-0.2, -0.2)], -0.025, 0.025, WHITE)
rudder.box((0.0, -0.3, 0.5), (0.056, 0.14, 1.0), PAINT)
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
    mid = Vector((0, 0.0, -0.1))
    views = {
        "side": ("ORTHO", Vector((14, 0.0, -0.1)), mid, 8.0),
        "top": ("ORTHO", Vector((0, 0.0, 14)), mid, 10.0),
        "front": ("ORTHO", Vector((0, 14, -0.1)), mid, 10.0),
        "three": ("PERSP", Vector((8.5, 9.0, 3.5)), mid, 0),
        "rear": ("PERSP", Vector((-7.5, -10.0, 3.5)), mid, 0),
        "below": ("PERSP", Vector((6.0, 5.0, -4.0)), mid, 0),
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
