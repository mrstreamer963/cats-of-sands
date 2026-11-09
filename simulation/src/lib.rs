mod config;

use bevy_ecs::prelude::*;
use serde::Serialize;
use wasm_bindgen::prelude::*;

const STEP: f64 = 1.0 / 60.0;
const MAX_FRAME_SECONDS: f64 = 0.25;
const SCENE_WIDTH: f64 = 640.0;

#[derive(Component, Serialize)]
struct Sprite {
    id: String,
    x: f64,
    y: f64,
    radius: f64,
    color: String,
}

#[derive(Component)]
struct Speed(f64);

fn move_sprites(mut sprites: Query<(&mut Sprite, &Speed)>) {
    for (mut sprite, speed) in &mut sprites {
        sprite.x = (sprite.x + speed.0 * STEP).rem_euclid(SCENE_WIDTH);
    }
}

#[wasm_bindgen]
pub struct Session {
    world: World,
    schedule: Schedule,
    accumulator: f64,
}

#[wasm_bindgen]
impl Session {
    #[wasm_bindgen(constructor)]
    pub fn new(layers: JsValue) -> Result<Session, JsError> {
        let layers: Vec<config::Layer> = serde_wasm_bindgen::from_value(layers)?;
        let config = config::load(&layers).map_err(|error| JsError::new(&error))?;
        Ok(Self::from_config(config))
    }

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

    pub fn snapshot(&mut self) -> Result<JsValue, JsError> {
        let mut query = self.world.query::<&Sprite>();
        let sprites: Vec<_> = query.iter(&self.world).collect();
        Ok(serde_wasm_bindgen::to_value(&sprites)?)
    }
}

impl Session {
    fn from_config(config: config::Config) -> Self {
        let mut world = World::new();
        for (id, entity) in config.entities {
            world.spawn((
                Sprite {
                    id,
                    x: entity.position.x,
                    y: entity.position.y,
                    radius: entity.radius,
                    color: entity.color,
                },
                Speed(entity.speed),
            ));
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(move_sprites);
        Self {
            world,
            schedule,
            accumulator: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
