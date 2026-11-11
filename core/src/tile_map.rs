use crate::{Fixed, Position};
use bevy_ecs::prelude::{Component, Entity, Resource, World};
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

#[derive(Clone, Component, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ground {
    pub passable: bool,
    pub cost: u32,
}

#[derive(Clone, Component, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Wall {
    pub blocking: bool,
    pub durability: u32,
}

#[derive(Deserialize)]
pub(crate) struct Grid {
    width: usize,
    height: usize,
    tilewidth: i32,
    tileheight: i32,
    ground: Vec<Tile>,
    walls: Vec<Option<Tile>>,
}

#[derive(Deserialize)]
struct Tile {
    terrain: String,
    gid: u32,
}

struct Cell {
    ground: Entity,
    wall: Option<(Entity, u32)>,
}

#[derive(Resource)]
pub(crate) struct TileMap {
    width: usize,
    height: usize,
    tilewidth: i32,
    tileheight: i32,
    cells: Vec<Cell>,
}

impl TileMap {
    pub fn load(
        world: &mut World,
        grid: Grid,
        ground: &BTreeMap<String, Ground>,
        walls: &BTreeMap<String, Wall>,
    ) -> Result<Self, String> {
        let size = grid.width.checked_mul(grid.height);
        if size != Some(grid.ground.len())
            || size != Some(grid.walls.len())
            || grid.width == 0
            || grid.height == 0
            || !valid_extent(grid.width, grid.tilewidth)
            || !valid_extent(grid.height, grid.tileheight)
        {
            return Err("Некорректный размер тайловой сетки".into());
        }

        let mut cells = Vec::with_capacity(grid.ground.len());
        for (index, (tile, wall)) in grid.ground.into_iter().zip(grid.walls).enumerate() {
            let surface = ground
                .get(&tile.terrain)
                .ok_or_else(|| format!("ground[{index}]: неизвестный тип {}", tile.terrain))?;
            let wall = wall
                .map(|tile| {
                    let definition = walls.get(&tile.terrain).ok_or_else(|| {
                        format!("walls[{index}]: неизвестный тип {}", tile.terrain)
                    })?;
                    Ok::<_, String>((world.spawn(definition.clone()).id(), tile.gid))
                })
                .transpose()?;
            cells.push(Cell {
                ground: world.spawn(surface.clone()).id(),
                wall,
            });
        }
        Ok(Self {
            width: grid.width,
            height: grid.height,
            tilewidth: grid.tilewidth,
            tileheight: grid.tileheight,
            cells,
        })
    }

    pub fn cell(&self, position: &Position) -> Option<usize> {
        let x = position
            .x
            .raw()
            .div_euclid(Fixed::from_int(self.tilewidth).raw());
        let y = position
            .y
            .raw()
            .div_euclid(Fixed::from_int(self.tileheight).raw());
        self.index(usize::try_from(x).ok()?, usize::try_from(y).ok()?)
    }

    fn index(&self, column: usize, row: usize) -> Option<usize> {
        (column < self.width && row < self.height).then(|| row * self.width + column)
    }

    pub fn cost(&self, world: &World, cell: usize) -> Option<u32> {
        let cell = &self.cells[cell];
        let ground = world.get::<Ground>(cell.ground)?;
        let blocked = cell
            .wall
            .is_some_and(|(entity, _)| world.get::<Wall>(entity).is_some_and(|wall| wall.blocking));
        (ground.passable && !blocked).then_some(ground.cost)
    }

    pub fn dimensions(&self) -> (Fixed, Fixed) {
        (
            Fixed::from_int(self.width as i32 * self.tilewidth),
            Fixed::from_int(self.height as i32 * self.tileheight),
        )
    }

    pub fn center(&self, cell: usize) -> Position {
        Position {
            x: Fixed::from_raw(
                Fixed::from_int((cell % self.width) as i32 * self.tilewidth).raw()
                    + Fixed::from_int(self.tilewidth).raw() / 2,
            ),
            y: Fixed::from_raw(
                Fixed::from_int((cell / self.width) as i32 * self.tileheight).raw()
                    + Fixed::from_int(self.tileheight).raw() / 2,
            ),
        }
    }

    pub fn drift_target(&self, world: &World, position: &Position) -> Option<Position> {
        let start = self.cell(position)?;
        let row = start / self.width;
        let end = (start % self.width + 1..self.width)
            .take_while(|column| self.cost(world, row * self.width + column).is_some())
            .last()
            .unwrap_or(start % self.width);
        let edge = Fixed::from_int((end + 1) as i32 * self.tilewidth);
        let x = edge.raw() - (Fixed::from_int(1).raw() / 1024);
        (x > position.x.raw()).then(|| Position {
            x: Fixed::from_raw(x),
            y: position.y,
        })
    }

    pub fn route(
        &self,
        world: &World,
        start: &Position,
        goal: &Position,
    ) -> Option<VecDeque<Position>> {
        let start = self.cell(start)?;
        let target = self.cell(goal)?;
        self.cost(world, target)?;
        if start == target {
            return Some(VecDeque::from([goal.clone()]));
        }

        let mut queue = BinaryHeap::new();
        let mut best = vec![u64::MAX; self.cells.len()];
        let mut parent = vec![start; self.cells.len()];
        best[start] = 0;
        queue.push(Reverse((self.distance(start, target), 0, start)));
        while let Some(Reverse((_, cost, cell))) = queue.pop() {
            if cost != best[cell] {
                continue;
            }
            if cell == target {
                let mut route = VecDeque::from([goal.clone()]);
                // The endpoints can connect directly across their shared edge
                // with the neighboring cell, without detouring to their centers.
                let mut step = parent[cell];
                while step != start {
                    route.push_front(self.center(step));
                    step = parent[step];
                }
                return Some(route);
            }
            for next in self.neighbors(cell) {
                let Some(step_cost) = self.cost(world, next) else {
                    continue;
                };
                let next_cost = cost + u64::from(step_cost);
                if next_cost < best[next] {
                    best[next] = next_cost;
                    parent[next] = cell;
                    queue.push(Reverse((
                        next_cost + self.distance(next, target),
                        next_cost,
                        next,
                    )));
                }
            }
        }
        None
    }

    fn distance(&self, from: usize, to: usize) -> u64 {
        ((from % self.width).abs_diff(to % self.width)
            + (from / self.width).abs_diff(to / self.width)) as u64
    }

    fn neighbors(&self, cell: usize) -> impl Iterator<Item = usize> {
        let column = cell % self.width;
        let row = cell / self.width;
        [
            column.checked_sub(1).and_then(|x| self.index(x, row)),
            row.checked_sub(1).and_then(|y| self.index(column, y)),
            self.index(column + 1, row),
            self.index(column, row + 1),
        ]
        .into_iter()
        .flatten()
    }

    pub fn regions(&self, world: &World) -> Vec<Option<usize>> {
        let mut regions = vec![None; self.cells.len()];
        for start in 0..self.cells.len() {
            if regions[start].is_some() || self.cost(world, start).is_none() {
                continue;
            }
            regions[start] = Some(start);
            let mut pending = VecDeque::from([start]);
            while let Some(cell) = pending.pop_front() {
                for next in self.neighbors(cell) {
                    if regions[next].is_none() && self.cost(world, next).is_some() {
                        regions[next] = Some(start);
                        pending.push_back(next);
                    }
                }
            }
        }
        regions
    }

    pub fn closest_free_center(
        &self,
        regions: &[Option<usize>],
        region: usize,
        position: &Position,
        assigned: &BTreeSet<(Fixed, Fixed)>,
    ) -> Option<Position> {
        regions
            .iter()
            .enumerate()
            .filter(|(_, entry)| **entry == Some(region))
            .map(|(cell, _)| self.center(cell))
            .filter(|center| !assigned.contains(&(center.x, center.y)))
            .min_by_key(|center| {
                let dx = i128::from(center.x.raw()) - i128::from(position.x.raw());
                let dy = i128::from(center.y.raw()) - i128::from(position.y.raw());
                (
                    dx.unsigned_abs().pow(2) + dy.unsigned_abs().pow(2),
                    center.y,
                    center.x,
                )
            })
    }

    pub fn wall_tiles(&self) -> impl Iterator<Item = (usize, u32)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(cell, entry)| entry.wall.map(|(_, gid)| (cell, gid)))
    }

    pub fn damage_wall(&mut self, world: &mut World, column: usize, row: usize, damage: u32) {
        let Some(cell) = self.index(column, row) else {
            return;
        };
        let Some((entity, _)) = self.cells[cell].wall else {
            return;
        };
        let mut wall = world
            .get_mut::<Wall>(entity)
            .expect("indexed wall must exist");
        wall.durability = wall.durability.saturating_sub(damage);
        if wall.durability == 0 {
            self.cells[cell].wall = None;
            world.despawn(entity);
        }
    }
}

fn valid_extent(cells: usize, tile: i32) -> bool {
    tile > 0
        && i32::try_from(cells)
            .ok()
            .and_then(|cells| cells.checked_mul(tile))
            .is_some()
}

#[derive(Component)]
pub(crate) struct Route(pub VecDeque<Position>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, SimId, Simulation, config};

    #[test]
    fn blocked_waypoint_replans_an_active_route() {
        let layer = config::tests::layer(
            "test.yaml",
            "tiled: test.tmj
ground:
  sand: { passable: true, cost: 1 }
  rock: { passable: false, cost: 1 }
units:
  - type: CAT
    Speed: 60
placements:
  - type: CAT
    position: { x: 5, y: 15 }",
        );
        let mut grid = Grid {
            width: 5,
            height: 3,
            tilewidth: 10,
            tileheight: 10,
            ground: (0..15)
                .map(|_| Tile {
                    terrain: "sand".into(),
                    gid: 1,
                })
                .collect(),
            walls: (0..15).map(|_| None).collect(),
        };
        grid.ground[7].terrain = "rock".into();
        let mut simulation =
            Simulation::from_config(config::load(&[layer]).unwrap(), Some(grid)).unwrap();
        let goal = Position {
            x: Fixed::from_int(45),
            y: Fixed::from_int(15),
        };
        simulation
            .submit_command(
                0,
                Command::MoveTo {
                    id: SimId(1),
                    point: goal.clone(),
                },
            )
            .unwrap();
        simulation.step(1);
        let top_gap = simulation.world.resource::<TileMap>().cells[2].ground;
        simulation
            .world
            .get_mut::<Ground>(top_gap)
            .unwrap()
            .passable = false;

        let mut crossed = false;
        for _ in 0..100 {
            simulation.step(1);
            let snapshot = simulation.snapshot();
            let position = snapshot[0].position.unwrap();
            if (Fixed::from_int(20)..Fixed::from_int(30)).contains(&position.x) {
                assert!(position.y >= Fixed::from_int(20));
                crossed = true;
            }
        }
        assert!(crossed);
        let snapshot = simulation.snapshot();
        let position = snapshot[0].position.unwrap();
        assert_eq!((position.x, position.y), (goal.x, goal.y));
    }
}
