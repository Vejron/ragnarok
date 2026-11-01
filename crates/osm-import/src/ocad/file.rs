//! The `.ocd` file itself, versions 9 to 2018: symbols, objects and the scale parameters.
//!
//! A little-endian binary with three chains of index blocks, each of 256 entries and the position of
//! the next block. OCAD's own description stops at version 10; the later layouts here follow
//! OpenOrienteering Mapper's importer and the `ocd-dump` format descriptions. Every read is checked:
//! a map is someone's file from the internet, and a bad one must be an error and not a panic.

use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, BTreeSet};

const MAGIC: u16 = 0x0cad;
/// Entries in an index block.
const BLOCK: usize = 256;
/// Bytes of an object index entry.
const OBJECT_ENTRY: usize = 40;
const STRING_ENTRY: usize = 16;
/// Where a symbol's description starts: a Pascal string until version 10, UTF-16 after.
const SYMBOL_DESCRIPTION: usize = 56;
/// Bytes of the object header before its coordinates, from version 12 and before it.
const OBJECT_HEADER: usize = 56;
const OBJECT_HEADER_V9: usize = 40;

/// Parameter string that holds the scale and the georeferencing.
const STRING_SCALE: i32 = 1039;

const STATUS_NORMAL: u8 = 1;

// Flags in the low byte of a coordinate.
const X_FIRST_CONTROL: i32 = 0x01;
const X_SECOND_CONTROL: i32 = 0x02;
const Y_HOLE: i32 = 0x02;

/// Sample points in a flattened curve segment.
const CURVE_STEPS: usize = 6;

pub const OBJECT_POINT: u8 = 1;
pub const OBJECT_LINE: u8 = 2;
pub const OBJECT_AREA: u8 = 3;

/// A point on the paper, 1/100 mm, y up.
pub type Paper = [f64; 2];

#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    /// The symbol's number times 1000: boulder 204.0 is `204000`.
    pub number: i32,
    pub kind: u8,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub symbol: i32,
    pub kind: u8,
    /// Degrees, for the rotatable point symbols.
    pub angle: f64,
    /// The outline and, for an area, its holes. Curves are flattened; a point has one point.
    pub rings: Vec<Vec<Paper>>,
}

/// The scale parameters: how the paper sits on the ground.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scale {
    /// The map's scale, `15000` for 1:15 000.
    pub scale: f64,
    /// Real-world easting and northing of the paper's origin, metres.
    pub x: f64,
    pub y: f64,
    /// Rotation of the paper, degrees.
    pub angle: f64,
    /// OCAD's grid and zone, `grid * 1000 + zone`.
    pub grid: i32,
    /// Whether real-world coordinates are in use at all.
    pub real: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    pub version: u16,
    pub subversion: u8,
    pub scale: Scale,
    pub symbols: BTreeMap<i32, Symbol>,
    pub objects: Vec<Object>,
    /// Encrypted objects, which cannot be read and are left out.
    pub encrypted: usize,
}

/// Bounds-checked little-endian reads.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn bytes(&self, at: usize, len: usize) -> Result<&[u8]> {
        at.checked_add(len).and_then(|end| self.0.get(at..end)).with_context(|| format!("{len} bytes at {at} are past the end of the file"))
    }

    fn u8(&self, at: usize) -> Result<u8> {
        Ok(self.bytes(at, 1)?[0])
    }

    fn u16(&self, at: usize) -> Result<u16> {
        let b = self.bytes(at, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i16(&self, at: usize) -> Result<i16> {
        Ok(self.u16(at)? as i16)
    }

    fn u32(&self, at: usize) -> Result<u32> {
        let b = self.bytes(at, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32(&self, at: usize) -> Result<i32> {
        Ok(self.u32(at)? as i32)
    }

    /// The positions of a chain of index blocks. A chain that loops ends at the first repeat.
    fn blocks(&self, first: u32) -> Result<Vec<usize>> {
        let mut seen = BTreeSet::new();
        let mut at = first;
        while at != 0 && seen.insert(at) {
            at = self.u32(at as usize)?;
        }
        let mut chain = Vec::with_capacity(seen.len());
        let mut at = first;
        for _ in 0..seen.len() {
            chain.push(at as usize);
            at = self.u32(at as usize)?;
        }
        Ok(chain)
    }
}

pub fn parse(bytes: &[u8]) -> Result<Map> {
    let file = Reader(bytes);
    ensure!(file.u16(0).context("too short for a header")? == MAGIC, "not an OCAD file");
    let version = file.u16(4)?;
    let subversion = file.u8(6)?;
    ensure!(version >= 9, "OCAD {version} files are not supported, only 9 and later; save the map as a newer version");
    let symbols = symbols(&file, version).context("reading the symbols")?;
    let (objects, encrypted) = objects(&file, version).context("reading the objects")?;
    let scale = strings(&file).context("reading the parameter strings")?;
    Ok(Map { version, subversion, scale, symbols, objects, encrypted })
}

fn symbols(file: &Reader, version: u16) -> Result<BTreeMap<i32, Symbol>> {
    let mut symbols = BTreeMap::new();
    for block in file.blocks(file.u32(8)?)? {
        for entry in 0..BLOCK {
            let at = file.u32(block + 4 + entry * 4)? as usize;
            if at == 0 {
                continue;
            }
            let number = file.i32(at + 4)?;
            let kind = file.u8(at + 8)?;
            let description = if version >= 11 {
                let raw = file.bytes(at + SYMBOL_DESCRIPTION, 128)?;
                let units: Vec<u16> = raw.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|&unit| unit != 0).collect();
                String::from_utf16_lossy(&units)
            } else {
                let len = usize::from(file.u8(at + SYMBOL_DESCRIPTION)?).min(31);
                // Windows-1252, which for a symbol's name is near enough to Latin-1.
                file.bytes(at + SYMBOL_DESCRIPTION + 1, len)?.iter().map(|&b| char::from(b)).collect()
            };
            symbols.insert(number, Symbol { number, kind, description });
        }
    }
    Ok(symbols)
}

fn objects(file: &Reader, version: u16) -> Result<(Vec<Object>, usize)> {
    let mut objects = Vec::new();
    let mut encrypted = 0;
    for block in file.blocks(file.u32(12)?)? {
        for entry in 0..BLOCK {
            let index = block + 4 + entry * OBJECT_ENTRY;
            let at = file.u32(index + 16)? as usize;
            let (symbol, status) = (file.i32(index + 24)?, file.u8(index + 30)?);
            if at == 0 || symbol == 0 || status != STATUS_NORMAL {
                continue;
            }
            if file.u8(index + 29)? != 0 {
                encrypted += 1;
                continue;
            }
            objects.push(object(file, version, at).with_context(|| format!("object at {at}"))?);
        }
    }
    Ok((objects, encrypted))
}

fn object(file: &Reader, version: u16, at: usize) -> Result<Object> {
    let (items, header) = if version >= 12 { (file.u32(at + 44)?, OBJECT_HEADER) } else { (file.u32(at + 8)?, OBJECT_HEADER_V9) };
    let items = items as usize;
    let raw = file.bytes(at + header, items.checked_mul(8).context("too many coordinates")?)?;
    let kind = file.u8(at + 4)?;
    let coords: Vec<[i32; 2]> = raw.chunks_exact(8).map(|c| [i32::from_le_bytes([c[0], c[1], c[2], c[3]]), i32::from_le_bytes([c[4], c[5], c[6], c[7]])]).collect();
    // Tenths of a degree.
    let angle = f64::from(file.i16(at + 6)?) / 10.0;
    Ok(Object { symbol: file.i32(at)?, kind, angle, rings: rings(&coords, kind) })
}

/// Splits coordinates into rings at the hole flags and flattens the curves. The low byte of each
/// value is flags, and the rest 1/100 mm.
fn rings(coords: &[[i32; 2]], kind: u8) -> Vec<Vec<Paper>> {
    let at = |c: [i32; 2]| [f64::from(c[0] >> 8), f64::from(c[1] >> 8)];
    if kind == OBJECT_POINT {
        return coords.first().map(|&c| vec![vec![at(c)]]).unwrap_or_default();
    }
    let mut rings: Vec<Vec<Paper>> = Vec::new();
    let mut i = 0;
    while i < coords.len() {
        let c = coords[i];
        if rings.is_empty() || (kind == OBJECT_AREA && c[1] & Y_HOLE != 0) {
            rings.push(Vec::new());
        }
        let ring = rings.last_mut().expect("just pushed");
        // A curve is its two control points and the end point, flagged on the first two.
        let curve = c[0] & X_FIRST_CONTROL != 0 && coords.get(i + 1).is_some_and(|next| next[0] & X_SECOND_CONTROL != 0) && i + 2 < coords.len();
        if let (true, Some(&start)) = (curve, ring.last()) {
            let (one, two, end) = (at(c), at(coords[i + 1]), at(coords[i + 2]));
            ring.extend((1..=CURVE_STEPS).map(|step| bezier(start, one, two, end, step as f64 / CURVE_STEPS as f64)));
            i += 3;
        } else {
            ring.push(at(c));
            i += 1;
        }
    }
    rings
}

fn bezier(a: Paper, b: Paper, c: Paper, d: Paper, t: f64) -> Paper {
    let u = 1.0 - t;
    let mix = |axis: usize| u * u * u * a[axis] + 3.0 * u * u * t * b[axis] + 3.0 * u * t * t * c[axis] + t * t * t * d[axis];
    [mix(0), mix(1)]
}

fn strings(file: &Reader) -> Result<Scale> {
    let mut scale = Scale::default();
    for block in file.blocks(file.u32(32)?)? {
        for entry in 0..BLOCK {
            let index = block + 4 + entry * STRING_ENTRY;
            let (at, size, kind) = (file.u32(index)? as usize, file.u32(index + 4)? as usize, file.i32(index + 8)?);
            if at != 0 && kind == STRING_SCALE {
                scale = scale_parameters(&String::from_utf8_lossy(file.bytes(at, size)?));
            }
        }
    }
    if scale.scale <= 0.0 {
        bail!("the file does not say its scale");
    }
    Ok(scale)
}

/// Reads `first\tmvalue\txvalue...`: a tab before each one-letter key.
fn scale_parameters(text: &str) -> Scale {
    let mut scale = Scale::default();
    let text = text.split('\0').next().unwrap_or_default();
    for field in text.split('\t').skip(1) {
        let mut chars = field.chars();
        let Some(key) = chars.next() else { continue };
        let value = chars.as_str();
        let number = value.parse::<f64>().ok();
        match (key, number) {
            ('m', Some(value)) => scale.scale = value,
            ('x', Some(value)) => scale.x = value,
            ('y', Some(value)) => scale.y = value,
            ('a', Some(value)) => scale.angle = value,
            ('i', Some(value)) => scale.grid = value as i32,
            ('r', Some(value)) => scale.real = value != 0.0,
            _ => {}
        }
    }
    scale
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// One file as OCAD lays it out: header, then one block of each index.
    pub struct Builder {
        pub version: u16,
        pub symbols: Vec<(i32, u8, String)>,
        /// `(symbol, kind, status, encrypted, coordinates)`.
        pub objects: Vec<(i32, u8, u8, u8, Vec<[i32; 2]>)>,
        pub scale: String,
    }

    pub fn point(x: i32, y: i32) -> [i32; 2] {
        [x << 8, y << 8]
    }

    fn put_u32(file: &mut [u8], at: usize, value: u32) {
        file[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    impl Builder {
        pub fn new(version: u16) -> Builder {
            Builder { version, symbols: Vec::new(), objects: Vec::new(), scale: "\tm10000\tx1000\ty2000\ta0\ti13002\tr1".into() }
        }

        pub fn build(&self) -> Vec<u8> {
            let mut file = vec![0u8; 64];
            file[0..2].copy_from_slice(&MAGIC.to_le_bytes());
            file[4..6].copy_from_slice(&self.version.to_le_bytes());

            let index = file.len();
            file.resize(index + 4 + BLOCK * 4, 0);
            put_u32(&mut file, 8, index as u32);
            for (i, (number, kind, description)) in self.symbols.iter().enumerate() {
                let at = file.len();
                file.resize(at + 1024, 0);
                put_u32(&mut file, at + 4, *number as u32);
                file[at + 8] = *kind;
                if self.version >= 11 {
                    for (unit, value) in description.encode_utf16().enumerate() {
                        file[at + SYMBOL_DESCRIPTION + unit * 2..][..2].copy_from_slice(&value.to_le_bytes());
                    }
                } else {
                    file[at + SYMBOL_DESCRIPTION] = description.len() as u8;
                    file[at + SYMBOL_DESCRIPTION + 1..][..description.len()].copy_from_slice(description.as_bytes());
                }
                put_u32(&mut file, index + 4 + i * 4, at as u32);
            }

            let index = file.len();
            file.resize(index + 4 + BLOCK * OBJECT_ENTRY, 0);
            put_u32(&mut file, 12, index as u32);
            for (i, (symbol, kind, status, encrypted, coords)) in self.objects.iter().enumerate() {
                let at = file.len();
                let header = if self.version >= 12 { OBJECT_HEADER } else { OBJECT_HEADER_V9 };
                file.resize(at + header + coords.len() * 8, 0);
                put_u32(&mut file, at, *symbol as u32);
                file[at + 4] = *kind;
                put_u32(&mut file, at + if self.version >= 12 { 44 } else { 8 }, coords.len() as u32);
                for (j, c) in coords.iter().enumerate() {
                    put_u32(&mut file, at + header + j * 8, c[0] as u32);
                    put_u32(&mut file, at + header + j * 8 + 4, c[1] as u32);
                }
                let entry = index + 4 + i * OBJECT_ENTRY;
                put_u32(&mut file, entry + 16, at as u32);
                put_u32(&mut file, entry + 24, *symbol as u32);
                file[entry + 29] = *encrypted;
                file[entry + 30] = *status;
            }

            let index = file.len();
            file.resize(index + 4 + BLOCK * STRING_ENTRY, 0);
            put_u32(&mut file, 32, index as u32);
            let at = file.len();
            file.extend_from_slice(self.scale.as_bytes());
            put_u32(&mut file, index + 4, at as u32);
            put_u32(&mut file, index + 8, self.scale.len() as u32);
            put_u32(&mut file, index + 12, STRING_SCALE as u32);
            file
        }
    }

    fn sample(version: u16) -> Builder {
        let mut builder = Builder::new(version);
        builder.symbols.push((204_000, 1, "Boulder".into()));
        builder.symbols.push((505_000, 2, "Vehicle track".into()));
        builder.objects.push((204_000, OBJECT_POINT, 1, 0, vec![point(100, -200)]));
        builder.objects.push((505_000, OBJECT_LINE, 1, 0, vec![point(0, 0), point(10, 0), point(10, 10)]));
        builder
    }

    #[test]
    fn reads_every_layout() {
        for version in [9, 10, 11, 12, 2018] {
            let map = parse(&sample(version).build()).unwrap();
            assert_eq!(map.version, version);
            assert_eq!(map.symbols[&204_000].description, "Boulder");
            assert_eq!(map.objects.len(), 2);
            assert_eq!(map.objects[0].rings, vec![vec![[100.0, -200.0]]]);
            assert_eq!(map.objects[1].rings[0].len(), 3);
        }
    }

    #[test]
    fn reads_the_scale() {
        let scale = parse(&sample(12).build()).unwrap().scale;
        assert_eq!(scale, Scale { scale: 10000.0, x: 1000.0, y: 2000.0, angle: 0.0, grid: 13002, real: true });
    }

    #[test]
    fn leaves_out_deleted_hidden_and_encrypted_objects() {
        let mut builder = sample(12);
        builder.objects.push((204_000, OBJECT_POINT, 0, 0, vec![point(1, 1)]));
        builder.objects.push((204_000, OBJECT_POINT, 2, 0, vec![point(1, 1)]));
        builder.objects.push((204_000, OBJECT_POINT, 3, 0, vec![point(1, 1)]));
        builder.objects.push((204_000, OBJECT_POINT, 1, 3, vec![point(1, 1)]));
        let map = parse(&builder.build()).unwrap();
        assert_eq!((map.objects.len(), map.encrypted), (2, 1));
    }

    #[test]
    fn splits_holes_and_flattens_curves() {
        let hole = [point(5, 5)[0], point(5, 5)[1] | Y_HOLE];
        let area = vec![point(0, 0), point(9, 0), point(9, 9), hole, point(6, 5), point(6, 6)];
        assert_eq!(rings(&area, OBJECT_AREA).iter().map(Vec::len).collect::<Vec<_>>(), [3, 3]);
        let curve = vec![point(0, 0), [point(0, 10)[0] | X_FIRST_CONTROL, point(0, 10)[1]], [point(10, 10)[0] | X_SECOND_CONTROL, point(10, 10)[1]], point(10, 0)];
        let ring = &rings(&curve, OBJECT_LINE)[0];
        assert_eq!(ring.len(), 1 + CURVE_STEPS);
        assert_eq!(ring[ring.len() - 1], [10.0, 0.0]);
        assert_eq!(ring[CURVE_STEPS / 2], [5.0, 7.5]);
    }

    #[test]
    fn bad_files_are_errors() {
        let good = sample(12).build();
        assert!(parse(&[]).is_err());
        assert!(parse(&[0u8; 64]).is_err());
        let mut old = good.clone();
        old[4] = 8;
        old[5] = 0;
        assert!(parse(&old).is_err());
        // Any cut must fail cleanly: this is the whole point of the checked reads.
        for len in (0..good.len()).step_by(61) {
            assert!(parse(&good[..len]).is_err(), "{len} bytes");
        }
    }

    #[test]
    fn a_looping_index_ends() {
        let mut file = sample(12).build();
        let first = u32::from_le_bytes(file[12..16].try_into().unwrap()) as usize;
        file[first..first + 4].copy_from_slice(&(first as u32).to_le_bytes());
        assert_eq!(parse(&file).unwrap().objects.len(), 2);
    }

    #[test]
    fn an_impossible_coordinate_count_is_an_error() {
        let mut file = sample(12).build();
        let first = u32::from_le_bytes(file[12..16].try_into().unwrap()) as usize;
        let object = u32::from_le_bytes(file[first + 20..first + 24].try_into().unwrap()) as usize;
        file[object + 44..object + 48].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&file).is_err());
    }
}
