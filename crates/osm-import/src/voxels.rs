//! Bakes every building's cubes into its tile, once, so the server and the clients never cut a
//! building themselves: the cells go into `Building::cells` in place of the walls and roof in the
//! collision mesh, and both sides make the faces from them (see `shared::carve`).

use shared::carve::{CELL, Shell, Tri, pack, unpack};
use shared::world::{Bits, CollisionMesh, Tile};
use std::collections::{HashMap, HashSet};

/// Cuts each building of `tile` into cubes and rewrites the collision mesh to match: a building's
/// walls and roof leave it (`walls` and `roof` become 0) and only its floor stays. A building too
/// big to cut keeps its own triangles and no cells. Bridges and whatever else is in the mesh are copied as they are, and
/// every `first` is moved to where its triangles now start. Baking a baked tile changes nothing.
pub fn bake(tile: &mut Tile) {
    let old = std::mem::take(&mut tile.collision);
    let end = old.triangles.len() as u32;
    let tri = |t: u32| -> Tri {
        let [a, b, c] = old.triangles[t as usize].map(|v| old.vertices[v as usize]);
        [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]]
    };
    // The mesh is cut where a building or a bridge starts (as `WorldFile::assemble` reads it).
    let mut starts: Vec<u32> = tile.buildings.iter().map(|b| b.first).chain(tile.bridges.iter().map(|b| b.first)).chain([0, end]).collect();
    starts.sort_unstable();
    starts.dedup();
    let mut out = Mesh::default();
    let mut moved: HashMap<u32, u32> = HashMap::new();
    for pair in starts.windows(2) {
        let (from, to) = (pair[0].min(end), pair[1].min(end));
        moved.insert(from, out.triangles.len() as u32);
        // A building baked already (its cells are there) is copied as it is.
        let building = tile.buildings.iter_mut().find(|b| b.first == from && b.walls + b.roof > 0 && b.cells.dims == [0; 3]);
        let mut copy_from = from;
        if let Some(building) = building {
            let roof = (from + building.walls).min(to);
            let floor = (roof + building.roof).min(to);
            let walls: Vec<Tri> = (from..roof).map(tri).collect();
            let roofs: Vec<Tri> = (roof..floor).map(tri).collect();
            if let Some(shell) = Shell::new(&walls, &roofs) {
                building.walls = 0;
                building.roof = 0;
                building.cells = shell.cells();
                copy_from = floor;
            }
        }
        for t in copy_from..to {
            let triangle = old.triangles[t as usize].map(|v| out.vertex_of(&old, v));
            out.triangles.push(triangle);
        }
    }
    for building in &mut tile.buildings {
        building.first = moved.get(&building.first).copied().unwrap_or(out.triangles.len() as u32);
    }
    for bridge in &mut tile.bridges {
        bridge.first = moved.get(&bridge.first).copied().unwrap_or(out.triangles.len() as u32);
    }
    tile.collision = CollisionMesh { vertices: out.vertices, triangles: out.triangles };
}

/// Gives each cube claimed by more than one building to the first of them, in the order `tiles`
/// and their buildings come: houses built wall to wall would otherwise both draw the cubes where
/// they meet, in the same place. The cubes go on drawing and colliding as one of them. Run it over
/// the whole world after `bake`, since neighbours can be in different tiles. Returns how many cubes
/// it took away; running it again takes none.
pub fn share(tiles: &mut [&mut Tile]) -> usize {
    let mut claimed: HashSet<[i32; 3]> = HashSet::new();
    let mut taken = 0;
    for building in tiles.iter_mut().flat_map(|tile| tile.buildings.iter_mut()) {
        let cells = &mut building.cells;
        let dims = cells.dims.map(|d| d as usize);
        let (n, layer) = (dims[0] * dims[1] * dims[2], dims[0] * dims[2]);
        if n == 0 {
            continue;
        }
        let mut solid = unpack(&cells.solid.0, n, layer);
        let mut roof = unpack(&cells.roof.0, n, layer);
        let base = cells.origin.map(|v| v.div_euclid(CELL));
        let before = taken;
        for id in 0..n {
            let at = [base[0] + (id % dims[0]) as i32, base[1] + (id / layer) as i32, base[2] + ((id / dims[0]) % dims[2]) as i32];
            if solid[id] && !claimed.insert(at) {
                (solid[id], roof[id]) = (false, false);
                taken += 1;
            }
        }
        if taken > before {
            cells.solid = Bits(pack(&solid, layer));
            cells.roof = Bits(pack(&roof, layer));
        }
    }
    taken
}

/// The mesh being written, with the old mesh's vertices that are still used, shared as they were.
#[derive(Default)]
struct Mesh {
    vertices: Vec<[i32; 3]>,
    triangles: Vec<[u32; 3]>,
    /// Old vertex number to new.
    copied: HashMap<u32, u32>,
}

impl Mesh {
    fn vertex_of(&mut self, old: &CollisionMesh, v: u32) -> u32 {
        *self.copied.entry(v).or_insert_with(|| {
            self.vertices.push(old.vertices[v as usize]);
            self.vertices.len() as u32 - 1
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{Bridge, Building};

    /// A closed box 5 × 4 × 3 m at x0: walls (8), roof (2), floor (2), as `roof::build` writes one.
    fn house(mesh: &mut CollisionMesh, x0: i32) -> Building {
        let base = mesh.vertices.len() as u32;
        let v = |x: i32, y: i32, z: i32| [x0 + x, y, z];
        mesh.vertices.extend([v(0, 0, 0), v(500, 0, 0), v(500, 0, 400), v(0, 0, 400), v(0, 300, 0), v(500, 300, 0), v(500, 300, 400), v(0, 300, 400)]);
        let first = mesh.triangles.len() as u32;
        let triangles = [[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]];
        mesh.triangles.extend(triangles.map(|t: [u32; 3]| t.map(|i| i + base)));
        Building { height: 300, base: 0, first, walls: 8, roof: 2, ..Default::default() }
    }

    #[test]
    fn a_tile_is_baked_into_cubes_and_everything_else_keeps_its_place() {
        let mut mesh = CollisionMesh::default();
        let a = house(&mut mesh, 0);
        // A bridge between the houses: one triangle.
        let bridge_first = mesh.triangles.len() as u32;
        let at = mesh.vertices.len() as u32;
        mesh.vertices.extend([[900, 100, 0], [1000, 100, 0], [900, 100, 100]]);
        mesh.triangles.push([at, at + 1, at + 2]);
        let b = house(&mut mesh, 2000);
        let mut tile = Tile { buildings: vec![a, b], bridges: vec![Bridge { first: bridge_first, deck: 1, steel: 0 }], collision: mesh, ..Default::default() };
        bake(&mut tile);
        let shell = Shell::from_cells(&tile.buildings[0].cells).unwrap();
        assert_eq!(shell.dims, [10, 6, 8]);
        let [a, b] = [&tile.buildings[0], &tile.buildings[1]];
        // Each house is its floor alone, then the bridge, then the other house the same way.
        assert_eq!((a.first, a.walls, a.roof), (0, 0, 0));
        assert_eq!(tile.bridges[0].first, 2);
        assert_eq!((b.first, b.walls, b.roof), (3, 0, 0));
        assert_eq!(tile.collision.triangles.len(), 5);
        let corner = |t: usize, k: usize| tile.collision.vertices[tile.collision.triangles[t][k] as usize];
        assert_eq!([0, 1, 2].map(|k| corner(2, k)), [[900, 100, 0], [1000, 100, 0], [900, 100, 100]]);
        assert_eq!([0, 1, 2].map(|k| corner(3, k)), [[2000, 0, 0], [2500, 0, 400], [2500, 0, 0]], "the floor");
        // The vertices nothing uses any more are gone: two floors and a bridge.
        assert_eq!(tile.collision.vertices.len(), 4 + 3 + 4);
        let again = tile.clone();
        bake(&mut tile);
        assert_eq!(tile, again, "baking twice");
    }

    #[test]
    fn houses_wall_to_wall_share_the_cubes_where_they_meet() {
        // Two 5 × 4 × 3 m houses, in different tiles, whose walls at x = 5 and 4.8 m are in the same
        // cubes (x 4.5..5 m); a third far off.
        let (mut one, mut two) = (CollisionMesh::default(), CollisionMesh::default());
        let a = house(&mut one, 0);
        let far = house(&mut one, 3000);
        let b = house(&mut two, 480);
        let mut first = Tile { buildings: vec![a, far], collision: one, ..Default::default() };
        let mut second = Tile { buildings: vec![b], collision: two, ..Default::default() };
        bake(&mut first);
        bake(&mut second);
        let count = |tile: &Tile, i: usize| Shell::from_cells(&tile.buildings[i].cells).unwrap().solid_count();
        let whole = [count(&first, 0), count(&first, 1), count(&second, 0)];
        // The wall at x = 4.5..5 m: 3 m high and 4 m deep, in half-metre cubes.
        assert_eq!(share(&mut [&mut first, &mut second]), 6 * 8);
        assert_eq!([count(&first, 0), count(&first, 1), count(&second, 0)], [whole[0], whole[1], whole[2] - 48]);
        let shell = Shell::from_cells(&second.buildings[0].cells).unwrap();
        assert!(!shell.is_solid(shell.index(0, 1, 1)), "the shared wall is the first house's");
        assert_eq!(shell.origin[0], 450);
        assert!(shell.is_solid(shell.index(10, 1, 1)), "its own far wall stays");
        assert_eq!(share(&mut [&mut first, &mut second]), 0, "sharing twice");
    }
}
