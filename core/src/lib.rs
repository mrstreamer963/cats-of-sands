#![doc = include_str!("../README.md")]

mod components;
mod config;
mod fixed;
mod formation;
mod observers;
mod sim_id;
mod systems;

pub use components::{Position, Sprite};
pub use config::Layer;
pub use fixed::Fixed;
pub use sim_id::SimId;

use bevy_ecs::prelude::*;
use components::{MoveTarget, PreparedComponent, Speed, Velocity};
use observers::{Clicked, apply_click_color};
use serde::Serialize;
use std::collections::BTreeMap;
use systems::move_entities;
use wasm_bindgen::prelude::*;

const TICKS_PER_SECOND: i64 = 60;
const SCENE_WIDTH: Fixed = Fixed::from_int(640);
const SCENE_HEIGHT: Fixed = Fixed::from_int(360);
const STEP: f64 = 1.0 / TICKS_PER_SECOND as f64;
const MAX_FRAME_SECONDS: f64 = 0.25;

#[derive(Serialize)]
pub struct EntitySnapshot<'a> {
    pub id: SimId,
    #[serde(rename = "type")]
    pub type_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<&'a Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<&'a Sprite>,
}

pub enum Command {
    Click { id: SimId },
    MoveTo { id: SimId, point: Position },
    MoveGroupTo { ids: Vec<SimId>, point: Position },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SimEvent {
    pub tick: u64,
    pub id: SimId,
    pub color: String,
}

#[derive(Default, Resource)]
struct SimEvents(Vec<SimEvent>);

/// ECS world with an explicit clock. Tick 0 is the first tick to execute.
pub struct Simulation {
    world: World,
    entities: BTreeMap<SimId, (Entity, String)>,
    types: BTreeMap<String, Vec<PreparedComponent>>,
    last_id: u32,
    schedule: Schedule,
    tick: u64,
    commands: BTreeMap<u64, Vec<Command>>,
}

impl Simulation {
    pub fn from_layers(layers: &[Layer]) -> Result<Self, String> {
        config::load(layers).map(Self::from_config)
    }

    fn from_config(config: config::LoadedConfig) -> Self {
        let mut world = World::new();
        world.init_resource::<SimEvents>();
        world.add_observer(apply_click_color);
        let mut schedule = Schedule::default();
        schedule.add_systems(move_entities);
        let mut simulation = Self {
            world,
            entities: BTreeMap::new(),
            types: config.types,
            last_id: 0,
            schedule,
            tick: 0,
            commands: BTreeMap::new(),
        };
        for (type_name, position) in config.placements {
            simulation
                .spawn_with_position(&type_name, position.as_ref())
                .expect("initial type must be spawnable");
        }
        simulation
    }

    /// Creates an independent instance from a YAML type.
    pub fn spawn(&mut self, type_name: &str) -> Result<SimId, String> {
        self.spawn_with_position(type_name, None)
    }

    fn spawn_with_position(
        &mut self,
        type_name: &str,
        position: Option<&PreparedComponent>,
    ) -> Result<SimId, String> {
        let components = self
            .types
            .get(type_name)
            .ok_or_else(|| format!("Неизвестный тип: {type_name}"))?;
        let next = self
            .last_id
            .checked_add(1)
            .ok_or_else(|| "Закончились идентификаторы сущностей".to_owned())?;
        let id = SimId(next);
        let mut entity = self.world.spawn_empty();
        for insert in components {
            insert(&mut entity);
        }
        if let Some(insert) = position {
            insert(&mut entity);
        }
        if let Some(speed) = entity.get::<Speed>() {
            let x = speed.0.div_int(TICKS_PER_SECOND);
            entity.insert(Velocity {
                x,
                ..Velocity::ZERO
            });
        }
        self.entities
            .insert(id, (entity.id(), type_name.to_owned()));
        self.last_id = next;
        Ok(id)
    }

    /// Removes the instance. Its ID is never assigned again.
    pub fn despawn(&mut self, id: SimId) -> bool {
        self.entities
            .remove(&id)
            .is_some_and(|(entity, _)| self.world.despawn(entity))
    }

    /// Tick that will execute next.
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Commands for the same tick execute in submission order.
    pub fn submit_command(&mut self, tick: u64, command: Command) -> Result<(), String> {
        if tick < self.tick {
            return Err(format!("Тик {tick} уже выполнен"));
        }
        self.commands.entry(tick).or_default().push(command);
        Ok(())
    }

    pub fn step(&mut self, ticks: u32) {
        for _ in 0..ticks {
            let next_tick = self.tick.checked_add(1).expect("simulation tick overflow");
            for command in self.commands.remove(&self.tick).into_iter().flatten() {
                match command {
                    Command::Click { id } => {
                        if let Some(&(entity, _)) = self.entities.get(&id) {
                            self.world.trigger(Clicked {
                                entity,
                                id,
                                tick: self.tick,
                            });
                        }
                    }
                    Command::MoveTo { id, point } => {
                        if let Some(entity) = self.movable_entity(id) {
                            self.set_move_target(entity, point);
                        }
                    }
                    Command::MoveGroupTo { ids, point } => self.move_group_to(ids, point),
                }
            }
            self.schedule.run(&mut self.world);
            self.tick = next_tick;
        }
    }

    fn movable_entity(&self, id: SimId) -> Option<Entity> {
        let &(entity, _) = self.entities.get(&id)?;
        self.world.get::<Position>(entity)?;
        let speed = self.world.get::<Speed>(entity)?;
        (speed.0.raw() >= TICKS_PER_SECOND).then_some(entity)
    }

    fn set_move_target(&mut self, entity: Entity, point: Position) {
        self.world.entity_mut(entity).insert(MoveTarget(point));
    }

    fn move_group_to(&mut self, mut ids: Vec<SimId>, point: Position) {
        ids.sort_unstable();
        ids.dedup();
        let members: Vec<_> = ids
            .into_iter()
            .filter_map(|id| self.movable_entity(id))
            .collect();
        match members.as_slice() {
            [] => {}
            &[entity] => self.set_move_target(entity, point),
            _ => {
                let radius = members
                    .iter()
                    .filter_map(|&entity| {
                        self.world.get::<Sprite>(entity).map(|sprite| sprite.radius)
                    })
                    .max()
                    .unwrap_or(Fixed::from_int(0));
                let destinations = formation::destinations(point, members.len(), radius);
                for (entity, destination) in members.into_iter().zip(destinations) {
                    self.set_move_target(entity, destination);
                }
            }
        }
    }

    pub fn snapshot(&self) -> Vec<EntitySnapshot<'_>> {
        self.entities
            .iter()
            .map(|(&id, (entity, type_name))| EntitySnapshot {
                id,
                type_name,
                position: self.world.get::<Position>(*entity),
                sprite: self.world.get::<Sprite>(*entity),
            })
            .collect()
    }

    pub fn drain_events(&mut self) -> Vec<SimEvent> {
        std::mem::take(&mut self.world.resource_mut::<SimEvents>().0)
    }
}

/// Browser adapter with a fixed-step frame accumulator.
#[wasm_bindgen]
pub struct Session {
    simulation: Simulation,
    accumulator: f64,
    previous_positions: BTreeMap<SimId, Position>,
}

#[wasm_bindgen]
impl Session {
    #[wasm_bindgen(constructor)]
    pub fn new(layers: JsValue) -> Result<Session, JsError> {
        let layers: Vec<Layer> = serde_wasm_bindgen::from_value(layers)?;
        let simulation = Simulation::from_layers(&layers).map_err(|error| JsError::new(&error))?;
        Ok(Self {
            simulation,
            accumulator: 0.0,
            previous_positions: BTreeMap::new(),
        })
    }

    /// Accepts up to 0.25 seconds per call.
    pub fn advance(&mut self, seconds: f64) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        self.accumulator += seconds.min(MAX_FRAME_SECONDS);
        let mut ticks = 0;
        while self.accumulator + f64::EPSILON >= STEP {
            ticks += 1;
            self.accumulator = (self.accumulator - STEP).max(0.0);
        }
        self.step(ticks);
    }

    pub fn current_tick(&self) -> u64 {
        self.simulation.current_tick()
    }

    pub fn step(&mut self, ticks: u32) {
        if ticks == 0 {
            return;
        }
        self.simulation.step(ticks - 1);
        self.previous_positions = self
            .simulation
            .snapshot()
            .into_iter()
            .filter_map(|entity| {
                entity
                    .position
                    .cloned()
                    .map(|position| (entity.id, position))
            })
            .collect();
        self.simulation.step(1);
    }

    pub fn snapshot(&self) -> Result<JsValue, JsError> {
        Ok(serde_wasm_bindgen::to_value(&self.simulation.snapshot())?)
    }

    /// Fraction of the current tick interval available for rendering.
    pub fn interpolation_alpha(&self) -> f64 {
        self.accumulator / STEP
    }

    /// Drawable entities with positions from the last two completed ticks.
    pub fn render_snapshot(&self) -> Result<JsValue, JsError> {
        #[derive(Serialize)]
        struct RenderEntity<'a> {
            id: SimId,
            position: &'a Position,
            previous_position: &'a Position,
            sprite: &'a Sprite,
        }

        let entities: Vec<_> = self
            .simulation
            .snapshot()
            .into_iter()
            .filter_map(|entity| {
                let position = entity.position?;
                Some(RenderEntity {
                    id: entity.id,
                    position,
                    previous_position: self.previous_positions.get(&entity.id).unwrap_or(position),
                    sprite: entity.sprite?,
                })
            })
            .collect();
        Ok(serde_wasm_bindgen::to_value(&entities)?)
    }

    /// Queues a click for the next tick to execute. Unknown IDs are ignored.
    pub fn click(&mut self, id: u32) {
        let tick = self.simulation.current_tick();
        self.submit_click(tick, id)
            .expect("the next tick accepts commands");
    }

    pub fn submit_click(&mut self, tick: u64, id: u32) -> Result<(), JsError> {
        self.simulation
            .submit_command(tick, Command::Click { id: SimId(id) })
            .map_err(|error| JsError::new(&error))
    }

    /// Queues a movement order for the next tick.
    pub fn move_to(&mut self, id: u32, x: f64, y: f64) -> Result<(), JsError> {
        self.simulation
            .submit_command(
                self.current_tick(),
                Command::MoveTo {
                    id: SimId(id),
                    point: movement_point(x, y)?,
                },
            )
            .map_err(|error| JsError::new(&error))
    }

    /// Queues one movement order for all selected instances.
    pub fn move_group_to(&mut self, ids: Vec<u32>, x: f64, y: f64) -> Result<(), JsError> {
        self.simulation
            .submit_command(
                self.current_tick(),
                Command::MoveGroupTo {
                    ids: ids.into_iter().map(SimId).collect(),
                    point: movement_point(x, y)?,
                },
            )
            .map_err(|error| JsError::new(&error))
    }

    pub fn spawn(&mut self, type_name: &str) -> Result<u32, JsError> {
        self.simulation
            .spawn(type_name)
            .map(|id| id.0)
            .map_err(|error| JsError::new(&error))
    }

    pub fn despawn(&mut self, id: u32) -> bool {
        self.simulation.despawn(SimId(id))
    }

    pub fn drain_events(&mut self) -> Result<JsValue, JsError> {
        let serializer =
            serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
        Ok(self.simulation.drain_events().serialize(&serializer)?)
    }
}

fn movement_point(x: f64, y: f64) -> Result<Position, JsError> {
    Ok(Position {
        x: Fixed::try_from_f64(x).ok_or_else(|| JsError::new("Некорректная координата x"))?,
        y: Fixed::try_from_f64(y).ok_or_else(|| JsError::new("Некорректная координата y"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ClickColor;

    #[test]
    fn group_order_assigns_distinct_places_inside_the_scene_in_id_order() {
        let layer = config::tests::layer(
            "group.yaml",
            "units:
  - type: CAT
    Speed: 60
    Sprite: { radius: 18, color: '#ffffff' }
  - type: FAST
    Speed: 120
    Sprite: { radius: 10, color: '#ffffff' }
placements:
  - { type: CAT, position: { x: 100, y: 18 } }
  - { type: FAST, position: { x: 100, y: 18 } }",
        );
        let mut simulation = Simulation::from_layers(&[layer]).unwrap();
        let [first, second] = [SimId(1), SimId(2)];
        let point = |x, y| Position {
            x: Fixed::from_int(x),
            y: Fixed::from_int(y),
        };
        let assert_positions = |simulation: &Simulation, expected: [(i32, i32); 2]| {
            for (entity, (x, y)) in simulation.snapshot().iter().zip(expected) {
                let position = entity.position.unwrap();
                assert_eq!(
                    (position.x, position.y),
                    (Fixed::from_int(x), Fixed::from_int(y))
                );
            }
        };

        simulation
            .submit_command(
                0,
                Command::MoveGroupTo {
                    ids: vec![second, SimId(99), first, second],
                    point: point(1, 1),
                },
            )
            .unwrap();
        simulation.step(1);
        assert_positions(&simulation, [(99, 18), (98, 18)]);
        simulation.step(100);
        assert_positions(&simulation, [(18, 18), (62, 18)]);

        simulation
            .submit_command(
                simulation.current_tick(),
                Command::MoveGroupTo {
                    ids: vec![second],
                    point: point(70, 18),
                },
            )
            .unwrap();
        simulation.step(10);
        assert_positions(&simulation, [(18, 18), (70, 18)]);
    }

    #[test]
    fn move_order_replaces_the_target_and_stops_on_arrival() {
        let mut simulation = Simulation::from_layers(&config::tests::shipped_scene()).unwrap();
        let id = simulation.snapshot()[0].id;
        let point = |x, y| Position {
            x: Fixed::from_int(x),
            y: Fixed::from_int(y),
        };
        simulation
            .submit_command(
                0,
                Command::MoveTo {
                    id,
                    point: point(80, 155),
                },
            )
            .unwrap();
        simulation.step(1);
        let snapshot = simulation.snapshot();
        let position = snapshot[0].position.unwrap();
        assert_eq!(
            (position.x, position.y),
            (Fixed::from_int(80), Fixed::from_int(151))
        );

        simulation
            .submit_command(
                1,
                Command::MoveTo {
                    id,
                    point: point(83, 155),
                },
            )
            .unwrap();
        simulation.step(10);
        let snapshot = simulation.snapshot();
        let position = snapshot[0].position.unwrap();
        assert_eq!(
            (position.x, position.y),
            (Fixed::from_int(83), Fixed::from_int(155))
        );
        assert!(
            simulation
                .world
                .get::<MoveTarget>(simulation.entities[&id].0)
                .is_none()
        );
        let velocity = simulation
            .world
            .get::<Velocity>(simulation.entities[&id].0)
            .unwrap();
        assert_eq!(
            (velocity.x, velocity.y),
            (Velocity::ZERO.x, Velocity::ZERO.y)
        );
    }

    #[test]
    fn equal_steps_give_equal_positions_across_frame_rates() {
        let mut one_step_per_frame = Session {
            simulation: Simulation::from_layers(&config::tests::shipped_scene()).unwrap(),
            accumulator: 0.0,
            previous_positions: BTreeMap::new(),
        };
        let mut two_steps_per_frame = Session {
            simulation: Simulation::from_layers(&config::tests::shipped_scene()).unwrap(),
            accumulator: 0.0,
            previous_positions: BTreeMap::new(),
        };

        for _ in 0..60 {
            one_step_per_frame.advance(STEP);
        }
        for _ in 0..30 {
            two_steps_per_frame.advance(2.0 * STEP);
        }
        for session in [&mut one_step_per_frame, &mut two_steps_per_frame] {
            let mut query = session.simulation.world.query::<&Position>();
            assert_eq!(
                query.single(&session.simulation.world).unwrap().x,
                Fixed::from_int(140)
            );
            let entity = session.simulation.snapshot().pop().unwrap();
            assert_eq!(
                session.previous_positions[&entity.id].x,
                Fixed::from_int(139)
            );
            session.advance(STEP / 2.0);
            assert!((session.interpolation_alpha() - 0.5).abs() < 1e-12);
        }
    }

    #[test]
    fn instances_of_one_type_have_independent_ids_and_state() {
        let [units, scene] = config::tests::shipped_scene();
        let addon = config::tests::shipped_addon();
        let mut simulation = Simulation::from_layers(&[units, scene, addon]).unwrap();
        let first = simulation.spawn("COMPANION").unwrap();
        let second = simulation.spawn("COMPANION").unwrap();
        assert_ne!(first, second);
        simulation
            .submit_command(0, Command::Click { id: first })
            .unwrap();
        simulation.step(1);

        let snapshot = simulation.snapshot();
        let first_view = snapshot.iter().find(|entity| entity.id == first).unwrap();
        let second_view = snapshot.iter().find(|entity| entity.id == second).unwrap();
        assert_eq!(first_view.type_name, second_view.type_name);
        assert_eq!(first_view.sprite.unwrap().color, "#f1a2ca");
        assert_eq!(second_view.sprite.unwrap().color, "#80d8c4");
        assert_eq!(simulation.drain_events()[0].id, first);

        simulation
            .submit_command(1, Command::Click { id: second })
            .unwrap();
        assert!(simulation.despawn(second));
        let replacement = simulation.spawn("COMPANION").unwrap();
        assert!(replacement > second);
        simulation.step(1);
        assert!(simulation.drain_events().is_empty());
        assert_eq!(
            simulation
                .snapshot()
                .iter()
                .find(|entity| entity.id == replacement)
                .unwrap()
                .sprite
                .unwrap()
                .color,
            "#80d8c4",
        );
    }

    #[test]
    fn fractional_negative_speed_wraps_with_integer_arithmetic() {
        let scene = config::tests::layer(
            "fractional.yaml",
            "entities: {cat: {Position: {x: 0.25, y: 0}, Speed: {value: -22.5}}}",
        );
        let mut simulation = Simulation::from_layers(&[scene]).unwrap();
        simulation.step(1);
        assert_eq!(
            simulation.snapshot()[0].position.unwrap().x.to_f64(),
            639.875
        );
    }

    #[test]
    fn clicks_change_only_the_target_with_a_click_color() {
        let [units, scene] = config::tests::shipped_scene();
        let addon = config::tests::shipped_addon();
        let mut session = Simulation::from_layers(&[units, scene, addon]).unwrap();
        let cat = session.entities[&SimId(1)].0;
        let companion = session.entities[&SimId(2)].0;
        session
            .world
            .entity_mut(cat)
            .insert(ClickColor("#00ff00".into()));

        session
            .submit_command(1, Command::Click { id: SimId(2) })
            .unwrap();
        session.step(1);
        assert!(session.drain_events().is_empty());
        assert_eq!(
            session.world.get::<Sprite>(companion).unwrap().color,
            "#80d8c4"
        );
        session.step(1);
        assert_eq!(session.current_tick(), 2);
        assert_eq!(
            session.drain_events(),
            vec![SimEvent {
                tick: 1,
                id: SimId(2),
                color: "#f1a2ca".into(),
            }]
        );
        assert!(session.drain_events().is_empty());
        assert!(
            session
                .submit_command(1, Command::Click { id: SimId(2) })
                .is_err()
        );
        assert_eq!(session.world.get::<Sprite>(cat).unwrap().color, "#f9c86d");
        let companion = session.world.get::<Sprite>(companion).unwrap();
        assert_eq!(companion.color, "#f1a2ca");
        assert_eq!(
            session.snapshot()[1].position.unwrap().x,
            Fixed::from_int(43)
        );
    }
}
