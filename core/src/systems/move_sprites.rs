use crate::STEP;
use crate::components::{Speed, Sprite};
use bevy_ecs::prelude::Query;

const SCENE_WIDTH: f64 = 640.0;

pub(crate) fn move_sprites(mut sprites: Query<(&mut Sprite, &Speed)>) {
    for (mut sprite, speed) in &mut sprites {
        sprite.x = (sprite.x + speed.0 * STEP).rem_euclid(SCENE_WIDTH);
    }
}
