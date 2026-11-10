#![doc = include_str!("../README.md")]

mod components;
mod config;
mod observers;
mod systems;

use bevy_ecs::prelude::*;
use components::{PreparedComponent, Sprite};
use observers::{Clicked, apply_click_color};
use serde::Serialize;
use std::collections::BTreeMap;
use systems::move_sprites;
use wasm_bindgen::prelude::*;

const STEP: f64 = 1.0 / 60.0;
const MAX_FRAME_SECONDS: f64 = 0.25;

#[derive(Serialize)]
struct SpriteSnapshot<'a> {
    id: &'a str,
    x: f64,
    y: f64,
    radius: f64,
    color: &'a str,
}

#[wasm_bindgen]
/// Игровая сессия с собственным миром Bevy и фиксированным шагом симуляции.
pub struct Session {
    world: World,
    entities: BTreeMap<String, Entity>,
    schedule: Schedule,
    accumulator: f64,
}

#[wasm_bindgen]
impl Session {
    /// Загружает упорядоченные YAML-слои в виде массива объектов с полями `name` и `text`.
    ///
    /// Возвращает ошибку загрузки или проверки конфигурации, если сессию создать нельзя.
    #[wasm_bindgen(constructor)]
    pub fn new(layers: JsValue) -> Result<Session, JsError> {
        let layers: Vec<config::Layer> = serde_wasm_bindgen::from_value(layers)?;
        let config = config::load(&layers).map_err(|error| JsError::new(&error))?;
        Ok(Self::from_config(config))
    }

    /// Накапливает прошедшее время в секундах и выполняет доступные шаги симуляции.
    ///
    /// Принимает до 0,25 секунды за вызов. Неположительные и неконечные значения пропускаются.
    pub fn advance(&mut self, seconds: f64) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }

        self.accumulator += seconds.min(MAX_FRAME_SECONDS);
        while self.accumulator + f64::EPSILON >= STEP {
            self.schedule.run(&mut self.world);
            self.accumulator = (self.accumulator - STEP).max(0.0);
        }
    }

    /// Возвращает массив объектов с полями `id`, `x`, `y`, `radius` и `color`.
    pub fn snapshot(&mut self) -> Result<JsValue, JsError> {
        let sprites: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(id, &entity)| {
                let sprite = self.world.get::<Sprite>(entity)?;
                Some(SpriteSnapshot {
                    id,
                    x: sprite.x,
                    y: sprite.y,
                    radius: sprite.radius,
                    color: &sprite.color,
                })
            })
            .collect();
        Ok(serde_wasm_bindgen::to_value(&sprites)?)
    }

    /// Отправляет событие клика сущности по её ID. Неизвестный ID пропускается.
    pub fn click(&mut self, id: &str) {
        if let Some(&entity) = self.entities.get(id) {
            self.world.trigger(Clicked { entity });
        }
    }
}

impl Session {
    fn from_config(config: BTreeMap<String, Vec<PreparedComponent>>) -> Self {
        let mut world = World::new();
        world.add_observer(apply_click_color);
        let mut entities = BTreeMap::new();
        for (id, components) in config {
            let mut entity = world.spawn_empty();
            for insert in components {
                insert(&mut entity);
            }
            entities.insert(id, entity.id());
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(move_sprites);
        Self {
            world,
            entities,
            schedule,
            accumulator: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ClickColor;

    #[test]
    fn equal_steps_give_equal_positions_across_frame_rates() {
        let mut one_step_per_frame =
            Session::from_config(config::load(&[config::tests::base()]).unwrap());
        let mut two_steps_per_frame =
            Session::from_config(config::load(&[config::tests::base()]).unwrap());

        for _ in 0..60 {
            one_step_per_frame.advance(STEP);
        }
        for _ in 0..30 {
            two_steps_per_frame.advance(2.0 * STEP);
        }
        for session in [&mut one_step_per_frame, &mut two_steps_per_frame] {
            let mut query = session.world.query::<&Sprite>();
            assert_eq!(query.single(&session.world).unwrap().x, 140.0);
        }
    }

    #[test]
    fn clicks_change_only_the_target_with_a_click_color() {
        let addon = config::tests::layer(
            "desert-companion.yaml",
            include_str!("../../public/config/addons/desert-companion.yaml"),
        );
        let mut session =
            Session::from_config(config::load(&[config::tests::base(), addon]).unwrap());
        let cat = session.entities["cat"];
        let companion = session.entities["companion"];
        session
            .world
            .entity_mut(cat)
            .insert(ClickColor("#00ff00".into()));

        session.click("companion");
        session.advance(STEP);
        assert_eq!(session.world.get::<Sprite>(cat).unwrap().color, "#f9c86d");
        let companion = session.world.get::<Sprite>(companion).unwrap();
        assert_eq!(companion.color, "#f1a2ca");
        assert_eq!(companion.x, 41.5);
    }
}
