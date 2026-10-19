//! Bakes every building's cubes into its tile, once, so the server and the clients never cut a
//! building themselves: the cells go into `Building::cells` in place of the walls and roof in the
//! collision mesh, and both sides make the faces from them (see `shared::carve`).

use shared::carve::{Shell, Tri};
use shared::world::{CollisionMesh, Tile};
use std::collections::HashMap;

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
        assert_eq!(shell.dims, [5, 3, 4]);
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
}
