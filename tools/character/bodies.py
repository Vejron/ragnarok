"""
The source rig's body (`build_character.py`): a lofted low-poly soldier on the rig the game's own
clips are posed on. The game does not draw it: its vertices settle the lying clips on the ground
before the clips are retargeted onto the packs' people (`characters.py`, CHARACTER-LOOKS.md).
"""

from parts import MAT_GEAR, MAT_OUTFIT, MAT_SKIN, grey, mottled, soles, srgb

SKIN = grey(1.0)
LIPS = grey(0.78)
EYE = srgb(0x1A1A1F)
BLACK = srgb(0x17181B)
DARK = srgb(0x2A2C31)



class Dress:
    """What a look wears on the body itself: a colour (or painter) and material for each segment."""

    def __init__(self, pelvis, waist, chest, upper_arm, forearm, thigh, shin, hands=None, shoes=(BLACK, BLACK), hair=None, hair_style="short"):
        # Each is (material, colour or painter, or a function of a seed returning a painter).
        self.pelvis, self.waist, self.chest = pelvis, waist, chest
        self.upper_arm, self.forearm = upper_arm, forearm
        self.thigh, self.shin = thigh, shin
        # None for bare hands, else the gloves' colour.
        self.hands = hands
        # The shoes' upper and sole.
        self.shoes = shoes
        # The hair's colour, None under a helmet, and its cut ("short", "long").
        self.hair, self.hair_style = hair, hair_style


def paint(part, seed):
    """A segment's material and painter; a mottled dress makes a new seeded painter per segment."""
    material, color = part
    if isinstance(color, Mottle):
        return material, mottled(color.shades, seed)
    return material, color


class Mottle:
    def __init__(self, *shades):
        self.shades = shades


# --------------------------------------------------------------------------------------------------
# The body
# --------------------------------------------------------------------------------------------------
def head(b, d, hair, style):
    """Head, face and hair on the `head` bone, the neck on `neck`."""
    b.prism((0, 0.006, d.NECK_Z - 0.05), (0, 0.006, d.HEAD_Z + 0.04), 0.05, 0.047, "neck", MAT_SKIN, SKIN, sides=8)
    # Ten sides, so the face has a ridge down its middle and a face either side of it for the eyes.
    b.loft(
        [
            (0, -0.045, 1.525, 0.042, 0.035),
            (0, -0.02, 1.555, 0.072, 0.075),
            (0, -0.006, 1.61, 0.083, 0.092),
            (0, 0.0, 1.67, 0.087, 0.097),
            (0, 0.008, 1.73, 0.079, 0.088),
            (0, 0.012, 1.772, 0.052, 0.058),
        ],
        "head", MAT_SKIN, SKIN, sides=10,
    )
    b.box((0, -0.103, 1.622), (0.026, 0.03, 0.046), "head", MAT_SKIN, grey(0.94), taper=(0.6, 0.5))
    b.box((0, -0.093, 1.577), (0.032, 0.012, 0.007), "head", MAT_SKIN, LIPS)
    for sx in (1, -1):
        b.box((sx * 0.033, -0.086, 1.655), (0.02, 0.012, 0.014), "head", MAT_GEAR, EYE)
        b.box((sx * 0.086, 0.006, 1.635), (0.018, 0.04, 0.055), "head", MAT_SKIN, grey(0.92))
        if hair is not None:
            b.box((sx * 0.035, -0.089, 1.679), (0.034, 0.012, 0.01), "head", MAT_GEAR, hair)
    if hair is None:
        return
    # The crown over the skull, its edge higher at the front than at the back; the back and sides of
    # the cut below it.
    b.loft(
        [
            (0, 0.004, 1.672, 0.092, 0.102, 0.0, -0.42),
            (0, 0.008, 1.73, 0.085, 0.095),
            (0, 0.012, 1.775, 0.062, 0.068),
            (0, 0.014, 1.792, 0.03, 0.034),
        ],
        "head", MAT_GEAR, hair, sides=10,
    )
    low = 1.555 if style == "long" else 1.6
    b.box((0, 0.052, (low + 1.7) / 2), (0.175, 0.085, 1.7 - low), "head", MAT_GEAR, hair, taper=(1.05, 1.1))
    for sx in (1, -1):
        b.box((sx * 0.083, -0.012, 1.66), (0.016, 0.05, 0.06 if style == "short" else 0.1), "head", MAT_GEAR, hair)


def torso(b, d, dress):
    """Pelvis on `hips`, waist on `spine`, chest and shoulders on `chest`: each overlaps the next.

    A tube's end is left open where it is buried in another part of the same limb, which comes off
    with it (`gibs.ts`), so it is never seen: a cap there would only cost vertices."""
    m, c = paint(dress.pelvis, 11)
    b.loft([(0, 0.006, 0.80, 0.12, 0.084, 0.3), (0, 0, 0.88, 0.154, 0.097, 0.3), (0, 0, 0.97, 0.151, 0.095, 0.3), (0, 0, 1.03, 0.148, 0.093, 0.3)], "hips", m, c, caps=(True, False))
    m, c = paint(dress.waist, 12)
    b.loft([(0, 0, 0.99, 0.148, 0.093, 0.3), (0, 0, 1.08, 0.15, 0.095, 0.3), (0, -0.003, 1.19, 0.165, 0.105, 0.3)], "spine", m, c, caps=(False, False))
    m, c = paint(dress.chest, 13)
    b.loft(
        [
            (0, -0.003, 1.15, 0.163, 0.104, 0.3),
            (0, -0.008, 1.27, 0.18, 0.115, 0.35),
            (0, -0.004, 1.37, 0.19, 0.11, 0.4),
            (0, 0.005, 1.44, 0.16, 0.09, 0.4),
            (0, 0.01, 1.48, 0.075, 0.06),
        ],
        "chest", m, c, caps=(False, True),
    )


def arm(b, d, dress, side, sx):
    """Shoulder, upper arm and elbow on the upper arm, the forearm on its own bone, then the hand: a
    palm on `hand`, the four fingers together on `fingers` and the thumb on `thumb`."""
    x = sx * d.SHOULDER_X
    elbow = d.SHOULDER_Z - d.UPPER_ARM
    wrist = elbow - d.FOREARM
    m, c = paint(dress.upper_arm, 20 + (side == "R"))
    b.ball((x, 0, d.SHOULDER_Z - 0.025), 0.072, f"upper_arm_{side}", m, c)
    b.loft([(x, 0, 1.40, 0.068, 0.07), (x, 0.002, 1.30, 0.064, 0.066), (x, 0, 1.13, 0.053, 0.055), (x, 0, elbow + 0.005, 0.05, 0.052)], f"upper_arm_{side}", m, c, caps=(False, False))
    b.ball((x, 0.004, elbow), 0.05, f"upper_arm_{side}", m, c)
    m, c = paint(dress.forearm, 30 + (side == "R"))
    b.loft([(x, 0, elbow + 0.015, 0.049, 0.052), (x, -0.004, 1.0, 0.053, 0.056), (x, 0, 0.82, 0.039, 0.036), (x, 0, wrist + 0.015, 0.034, 0.031)], f"forearm_{side}", m, c)
    m, c = (MAT_SKIN, SKIN) if dress.hands is None else (MAT_GEAR, dress.hands)
    b.ball((x, 0, wrist), 0.026, f"hand_{side}", m, c)
    b.loft([(x, -0.004, wrist + 0.012, 0.019, 0.031, 0.4), (x, -0.006, wrist - 0.012, 0.022, 0.042, 0.5), (x, -0.008, wrist - 0.072, 0.019, 0.045, 0.5)], f"hand_{side}", m, c, caps=(False, True))
    b.loft([(x, -0.008, wrist - 0.068, 0.017, 0.043, 0.5), (x, -0.006, wrist - 0.115, 0.015, 0.04, 0.4), (x, -0.004, wrist - 0.152, 0.011, 0.032, 0.3)], f"fingers_{side}", m, c)
    b.prism((x - sx * 0.004, -0.03, wrist - 0.012), (x - sx * 0.012, -0.058, wrist - 0.068), 0.014, 0.011, f"thumb_{side}", m, c)


def leg(b, d, dress, side, sx, boots):
    """Thigh and knee on the thigh, the shin on its own bone, and the shoe: heel and instep on `foot`,
    the toe cap on `toes`, the soles flat on the ground."""
    x = sx * d.LEG_X
    m, c = paint(dress.thigh, 40 + (side == "R"))
    b.loft([(x, 0.0, 0.93, 0.094, 0.1, 0.2), (x + sx * 0.005, -0.004, 0.84, 0.097, 0.103, 0.2), (x, -0.004, 0.70, 0.085, 0.09, 0.2), (x, 0.0, 0.54, 0.068, 0.07, 0.2), (x, 0.002, d.KNEE_Z - 0.01, 0.062, 0.064, 0.2)], f"thigh_{side}", m, c)
    b.ball((x, -0.004, d.KNEE_Z), 0.063, f"thigh_{side}", m, c)
    m, c = paint(dress.shin, 50 + (side == "R"))
    b.loft([(x, 0.004, d.KNEE_Z + 0.02, 0.061, 0.063), (x, 0.012, 0.40, 0.062, 0.071), (x, 0.006, 0.24, 0.048, 0.054), (x, 0.0, 0.13, 0.045, 0.048), (x, 0.0, 0.085, 0.043, 0.046)], f"shin_{side}", m, c, caps=(True, False))
    upper, sole = dress.shoes
    if boots:
        b.loft([(x, 0.006, 0.25, 0.053, 0.058, 0.2), (x, 0.0, 0.09, 0.05, 0.054, 0.2)], f"shin_{side}", MAT_GEAR, upper)
    shoe = soles(upper, sole)
    b.loft(
        [
            (x, 0.07, 0.05, 0.042, 0.05, 0.5),
            (x, 0.03, 0.075, 0.05, 0.075, 0.5),
            (x, -0.04, 0.06, 0.054, 0.06, 0.5),
            (x, -0.112, 0.04, 0.055, 0.04, 0.5),
        ],
        f"foot_{side}", MAT_GEAR, shoe, axis="y",
    )
    b.loft([(x, -0.104, 0.038, 0.054, 0.038, 0.5), (x, -0.18, 0.03, 0.05, 0.03, 0.5), (x, -0.215, 0.026, 0.035, 0.023, 0.4)], f"toes_{side}", MAT_GEAR, shoe, axis="y")


def body(b, d, dress, boots=False):
    torso(b, d, dress)
    head(b, d, dress.hair, dress.hair_style)
    for side, sx in (("L", 1), ("R", -1)):
        arm(b, d, dress, side, sx)
        leg(b, d, dress, side, sx, boots)


def ring(b, bone, x, y, z0, z1, rx, ry, material, color, sides=8, square=0.3):
    """A band round a segment, as a belt, a cuff or an armband."""
    b.loft([(x, y, z0, rx, ry, square), (x, y, z1, rx, ry, square)], bone, material, color, sides=sides)


# --------------------------------------------------------------------------------------------------
# The looks
# --------------------------------------------------------------------------------------------------
def soldier(b, d):
    camo = Mottle(srgb(0x56603F), srgb(0x6B7550), srgb(0x454E34), srgb(0x77775A))
    pants = Mottle(srgb(0x4F5839), srgb(0x626B48), srgb(0x3F4730), srgb(0x6E6E52))
    olive, strap, helmet = srgb(0x3D4530), srgb(0x2E3426), srgb(0x4B5437)
    boot, sole = srgb(0x1F1D1B), srgb(0x121212)
    g = MAT_GEAR
    dress = Dress((g, pants), (g, camo), (g, camo), (g, camo), (g, camo), (g, pants), (g, pants), hands=DARK, shoes=(boot, sole), hair=None)
    body(b, d, dress, boots=True)
    # The plate carrier, its shoulder straps, and magazine pouches on the front.
    b.loft([(0, -0.006, 1.14, 0.178, 0.12, 0.6), (0, -0.01, 1.27, 0.193, 0.128, 0.6), (0, -0.004, 1.37, 0.194, 0.122, 0.6)], "chest", g, olive)
    for sx in (1, -1):
        b.box((sx * 0.11, 0.0, 1.405), (0.065, 0.215, 0.07), "chest", g, olive)
    for x in (-0.068, 0.0, 0.068):
        b.box((x, -0.138, 1.2), (0.056, 0.036, 0.095), "chest", g, strap)
    b.box((0.1, -0.132, 1.31), (0.05, 0.03, 0.06), "chest", g, strap)
    # The belt and its pouches.
    ring(b, "spine", 0, 0, 0.982, 1.035, 0.158, 0.101, g, strap)
    for sx in (1, -1):
        b.box((sx * 0.152, 0.035, 0.995), (0.04, 0.075, 0.075), "spine", g, olive)
    # The backpack on the carrier's back, its flap on top.
    b.box((0, 0.192, 1.24), (0.25, 0.13, 0.30), "chest", g, olive, taper=(0.9, 0.9))
    b.box((0, 0.19, 1.39), (0.22, 0.12, 0.04), "chest", g, strap)
    # Knee pads.
    for side, sx in (("L", 1), ("R", -1)):
        b.box((sx * d.LEG_X, -0.06, d.KNEE_Z - 0.005), (0.085, 0.035, 0.085), f"shin_{side}", g, strap)
    # The helmet: brow higher than the nape, a cover, and goggles pushed up on its front.
    b.loft(
        [
            (0, 0.004, 1.672, 0.106, 0.116, 0.1, -0.3),
            (0, 0.004, 1.686, 0.101, 0.111, 0.1, -0.3),
            (0, 0.008, 1.745, 0.093, 0.103),
            (0, 0.012, 1.792, 0.064, 0.072),
            (0, 0.014, 1.815, 0.026, 0.03),
        ],
        "head", g, helmet, sides=10,
    )
    b.box((0, -0.088, 1.762), (0.13, 0.03, 0.032), "head", g, srgb(0x6FB8CC))
    for sx in (1, -1):
        b.box((sx * 0.086, -0.022, 1.6), (0.008, 0.012, 0.13), "head", g, strap)
    # The player's colour: a band round the helmet, an armband on each arm and a patch on each shoulder.
    team = grey(1.0)
    ring(b, "head", 0, 0.007, 1.712, 1.734, 0.098, 0.108, MAT_OUTFIT, team, sides=10, square=0.0)
    for side, sx in (("L", 1), ("R", -1)):
        ring(b, f"upper_arm_{side}", sx * d.SHOULDER_X, 0.001, 1.2, 1.25, 0.063, 0.065, MAT_OUTFIT, team, square=0.0)
        b.box((sx * (d.SHOULDER_X + 0.066), 0.0, 1.315), (0.012, 0.05, 0.05), f"upper_arm_{side}", MAT_OUTFIT, team)


def build(look, b, d):
    """Builds the source rig's body into builder `b` (`look` is "soldier", the one there is)."""
    if look != "soldier":
        raise ValueError(look)
    soldier(b, d)
