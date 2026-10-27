//! Armour and penetration, in millimetres of steel: what each vehicle is made of, plate by plate, and
//! what each round can go through.
//!
//! A vehicle's `Armor` is a list of plates. A shot that meets the vehicle is matched to the first plate
//! whose solid, face and region it struck (so the small, special plates — a driver's visor slit, a
//! mantlet, an engine grille — come before the plain faces). The plate's thickness over the cosine of
//! the angle the shot meets it at (its slope included) is what the round has to get through. The server
//! judges it (`crates/server/src/mechanics/armor.rs`).

/// The faces of a solid, in the vehicle's frame (+X right, +Y up, -Z forward).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Front,
    Side,
    Back,
    Top,
    Belly,
}

impl Zone {
    /// The face a hull-frame outward `normal` is on: the axis it is most along.
    pub fn of(normal: [f32; 3]) -> Zone {
        let [x, y, z] = normal;
        if y.abs() >= x.abs() && y.abs() >= z.abs() {
            if y >= 0.0 { Zone::Top } else { Zone::Belly }
        } else if z.abs() >= x.abs() {
            if z < 0.0 { Zone::Front } else { Zone::Back }
        } else {
            Zone::Side
        }
    }
}

/// Which of a vehicle's solids a plate is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    /// The hull box.
    Hull,
    /// `spec.solids[i]`: a turret, a casemate, a shield.
    Solid(u8),
    /// Any of them.
    Any,
}

/// An axis-aligned box in the vehicle's frame: a centre and half extents, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box3 {
    pub centre: [f32; 3],
    pub half: [f32; 3],
}

impl Box3 {
    pub const fn new(centre: [f32; 3], half: [f32; 3]) -> Box3 {
        Box3 { centre, half }
    }

    /// Whether `point` is inside it, or within `margin` of it.
    pub fn holds(&self, point: [f32; 3], margin: f32) -> bool {
        (0..3).all(|i| (point[i] - self.centre[i]).abs() <= self.half[i] + margin)
    }
}

/// One plate of a vehicle's armour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plate {
    pub name: &'static str,
    pub on: On,
    /// The face it covers; `None` for every face (a turret drum, a shield).
    pub face: Option<Zone>,
    /// Only where the shot lands in this box (a slit, a mantlet); `None` for the whole face.
    pub region: Option<Box3>,
    /// Thickness, millimetres.
    pub mm: f32,
    /// How far it leans back from square to its face, degrees: a glacis's 55. It turns the plate's
    /// normal up, so a level shot meets it at that angle.
    pub slope: f32,
    /// Incidence beyond which a round glances off, degrees (unless it overmatches the plate).
    pub ricochet: f32,
    /// A gap rather than a plate: a visor or a vision slit. A round goes straight through it, and no
    /// fragments come with it.
    pub slit: bool,
}

impl Plate {
    /// A plain plate of `mm` on a whole `face` of the hull.
    pub const fn hull(name: &'static str, face: Zone, mm: f32) -> Plate {
        Plate { name, on: On::Hull, face: Some(face), region: None, mm, slope: 0.0, ricochet: 75.0, slit: false }
    }

    /// The same plate on solid `index` instead.
    pub const fn on(self, index: u8) -> Plate {
        Plate { on: On::Solid(index), ..self }
    }

    /// The same plate on every face.
    pub const fn all_round(self) -> Plate {
        Plate { face: None, ..self }
    }

    pub const fn sloped(self, slope: f32) -> Plate {
        Plate { slope, ..self }
    }

    pub const fn glances(self, ricochet: f32) -> Plate {
        Plate { ricochet, ..self }
    }

    pub const fn within(self, region: Box3) -> Plate {
        Plate { region: Some(region), ..self }
    }

    /// A slit in `face` of the hull at `region`.
    pub const fn slit(name: &'static str, face: Zone, region: Box3) -> Plate {
        Plate { name, on: On::Hull, face: Some(face), region: Some(region), mm: 0.0, slope: 0.0, ricochet: 91.0, slit: true }
    }

    /// Whether a shot that struck solid `solid` (`None` the hull) on `face` at `point` meets this plate.
    pub fn covers(&self, solid: Option<u8>, face: Zone, point: [f32; 3]) -> bool {
        let on = match self.on {
            On::Any => true,
            On::Hull => solid.is_none(),
            On::Solid(i) => solid == Some(i),
        };
        // A little slack: the point is on the solid's surface, which a region's box is drawn around.
        on && self.face.is_none_or(|f| f == face) && self.region.is_none_or(|region| region.holds(point, 0.05))
    }
}

/// What a round does once it is through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenKind {
    /// A plain bullet: it stops in what it meets.
    Ball,
    /// Armour-piercing: a solid shot or an AP bullet.
    Ap,
    /// High explosive: bursts on the plate. Even when it does not go through, the blast does a little.
    He,
}

/// How much steel a round goes through, millimetres: `near` at the muzzle, falling to `far` at `far_m`
/// metres and staying there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    pub near: f32,
    pub far: f32,
    pub far_m: f32,
    pub kind: PenKind,
    /// The round's calibre, millimetres: how big a hole it makes, and how much flies off the inside of it.
    pub calibre: f32,
}

impl Pen {
    /// Nothing at all: a bandage, a grenade (which hurts by its blast).
    pub const NONE: Pen = Pen { near: 0.0, far: 0.0, far_m: 1.0, kind: PenKind::Ball, calibre: 0.0 };

    pub const fn ball(near: f32, far: f32, far_m: f32, calibre: f32) -> Pen {
        Pen { near, far, far_m, kind: PenKind::Ball, calibre }
    }

    pub const fn ap(near: f32, far: f32, far_m: f32, calibre: f32) -> Pen {
        Pen { near, far, far_m, kind: PenKind::Ap, calibre }
    }

    pub const fn he(mm: f32, calibre: f32) -> Pen {
        Pen { near: mm, far: mm, far_m: 1.0, kind: PenKind::He, calibre }
    }

    /// Millimetres it goes through after `m` metres of flight.
    pub fn at(&self, m: f32) -> f32 {
        let along = (m / self.far_m.max(1e-3)).clamp(0.0, 1.0);
        self.near + (self.far - self.near) * along
    }
}

/// What is inside a vehicle, that a round which gets through can hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// Hurt, it gives less power and speed; gone, none.
    Engine,
    /// Gone, the tracks are not driven.
    Transmission,
    /// Outside the hull, on its flank: a shell that bursts on it or beside it breaks it, armour or not.
    /// Broken, that side is not driven and the vehicle can hardly turn.
    TrackLeft,
    TrackRight,
    /// Gone, the gun cannot fire.
    Breech,
    /// Gone, it cooks off and the vehicle is wrecked.
    Ammo,
    /// Gone, it burns: the vehicle loses health until it is a wreck.
    Fuel,
    /// The occupant of seat `n`, who is hurt themselves.
    Crew(u8),
}

/// How many parts have a state (`Part::Crew` is a person's own health).
pub const PARTS: usize = 7;

impl Part {
    /// Its slot in a vehicle's part states; `None` for crew.
    pub const fn slot(self) -> Option<usize> {
        Some(match self {
            Part::Engine => 0,
            Part::Transmission => 1,
            Part::TrackLeft => 2,
            Part::TrackRight => 3,
            Part::Breech => 4,
            Part::Ammo => 5,
            Part::Fuel => 6,
            Part::Crew(_) => return None,
        })
    }

    /// Outside the armour: hit where the shot lands, whether or not it gets through.
    pub const fn outside(self) -> bool {
        matches!(self, Part::TrackLeft | Part::TrackRight)
    }
}

/// A part, where it is (the vehicle's frame, the turret's turned to face ahead for one in the turret)
/// and how much it takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Module {
    pub part: Part,
    pub at: Box3,
    pub hp: u8,
    /// It turns with the turret, and goes with it.
    pub turret: bool,
}

impl Module {
    pub const fn new(part: Part, centre: [f32; 3], half: [f32; 3], hp: u8) -> Module {
        Module { part, at: Box3::new(centre, half), hp, turret: false }
    }

    pub const fn crew(seat: u8, centre: [f32; 3], half: [f32; 3]) -> Module {
        Module::new(Part::Crew(seat), centre, half, 0)
    }

    pub const fn in_turret(self) -> Module {
        Module { turret: true, ..self }
    }
}

/// A part's state as the snapshot carries it: sound, hurt (half or less of its health left) or gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Sound = 0,
    Hurt = 1,
    Gone = 2,
}

impl State {
    /// The state of a part with `hp` of its `full` left.
    pub fn of(hp: u8, full: u8) -> State {
        if hp == 0 {
            State::Gone
        } else if u16::from(hp) * 2 <= u16::from(full) {
            State::Hurt
        } else {
            State::Sound
        }
    }
}

/// Part states two bits each, in slot order, in the two bytes the snapshot carries them in
/// (`DAMAGE_PARTS`).
pub fn pack(states: [State; PARTS]) -> [u8; 2] {
    let bits = states.iter().enumerate().fold(0u16, |bits, (slot, &state)| bits | (state as u16) << (2 * slot));
    bits.to_le_bytes()
}

pub fn unpack(bytes: [u8; 2]) -> [State; PARTS] {
    let bits = u16::from_le_bytes(bytes);
    std::array::from_fn(|slot| match (bits >> (2 * slot)) & 3 {
        0 => State::Sound,
        1 => State::Hurt,
        _ => State::Gone,
    })
}

/// A vehicle's armour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Armor {
    /// Matched in order: the first that covers where a shot landed is the one it meets.
    pub plates: &'static [Plate],
    /// What is inside, which a round that gets through goes on to hit.
    pub modules: &'static [Module],
    /// Canvas and wood, which a machine gun's rounds tear through: they deal their `plane` damage.
    pub canvas: bool,
}

/// Unarmoured: a sheet of something everywhere, which any round goes through.
const fn skin(name: &'static str, mm: f32, ricochet: f32) -> Plate {
    Plate { name, on: On::Any, face: None, region: None, mm, slope: 0.0, ricochet, slit: false }
}

impl Armor {
    /// The plate a shot that struck solid `solid` (`None` the hull) on `face` at `point` meets, if any.
    pub fn plate(&self, solid: Option<u8>, face: Zone, point: [f32; 3]) -> Option<&'static Plate> {
        self.plates.iter().find(|plate| plate.covers(solid, face, point))
    }

    /// The part in `slot`, if it has one.
    pub fn module(&self, slot: usize) -> Option<&'static Module> {
        self.modules.iter().find(|module| module.part.slot() == Some(slot))
    }

    /// Whether it is proof against small arms and splash: its thinnest plate (slits aside) is at least
    /// `ARMOURED_MM` thick.
    pub fn armoured(&self) -> bool {
        self.plates.iter().filter(|plate| !plate.slit).all(|plate| plate.mm >= ARMOURED_MM) && !self.plates.is_empty()
    }
}

/// A vehicle whose every plate is at least this thick shrugs off splash.
pub const ARMOURED_MM: f32 = 8.0;

/// A car's pressed steel: any bullet goes through, and only the most grazing glances off.
pub const SHEET: Armor = Armor { plates: &[skin("body", 1.5, 82.0)], modules: &[], canvas: false };
/// A helicopter's or a walker's light alloy skin: a little more than a car's.
pub const ALLOY: Armor = Armor { plates: &[skin("skin", 3.0, 82.0)], modules: &[], canvas: false };
/// Canvas and wood: nothing glances off.
pub const SOFT: Armor = Armor { plates: &[skin("canvas", 0.0, 91.0)], modules: &[], canvas: true };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_states_pack_into_two_bytes_and_back() {
        let states = [State::Gone, State::Sound, State::Hurt, State::Sound, State::Sound, State::Hurt, State::Gone];
        assert_eq!(unpack(pack(states)), states);
        assert_eq!(pack([State::Sound; PARTS]), [0, 0]);
        assert_eq!(State::of(50, 100), State::Hurt);
        assert_eq!(State::of(51, 100), State::Sound);
    }

    #[test]
    fn faces_are_found_by_the_normals_main_axis() {
        assert_eq!(Zone::of([0.0, 0.0, -1.0]), Zone::Front);
        assert_eq!(Zone::of([0.0, 0.0, 1.0]), Zone::Back);
        assert_eq!(Zone::of([-0.9, 0.1, 0.2]), Zone::Side);
        assert_eq!(Zone::of([0.1, 0.9, 0.2]), Zone::Top);
        assert_eq!(Zone::of([0.1, -0.9, 0.2]), Zone::Belly);
    }

    #[test]
    fn penetration_falls_off_with_range_and_then_holds() {
        let pen = Pen::ap(90.0, 70.0, 500.0, 75.0);
        assert_eq!(pen.at(0.0), 90.0);
        assert_eq!(pen.at(250.0), 80.0);
        assert_eq!(pen.at(2000.0), 70.0);
    }

    #[test]
    fn the_first_plate_that_covers_a_point_is_the_one_met() {
        const SLIT: Box3 = Box3::new([0.4, 0.3, -2.6], [0.15, 0.04, 0.1]);
        const ARMOR: Armor = Armor { plates: &[Plate::slit("visor", Zone::Front, SLIT), Plate::hull("front", Zone::Front, 50.0)], modules: &[], canvas: false };
        assert!(ARMOR.plate(None, Zone::Front, [0.4, 0.3, -2.6]).unwrap().slit);
        assert_eq!(ARMOR.plate(None, Zone::Front, [0.0, 0.0, -2.6]).unwrap().name, "front");
        assert!(ARMOR.plate(None, Zone::Side, [1.0, 0.0, 0.0]).is_none());
        assert!(ARMOR.plate(Some(0), Zone::Front, [0.4, 0.3, -2.6]).is_none(), "the hull's plates are not the turret's");
    }
}
