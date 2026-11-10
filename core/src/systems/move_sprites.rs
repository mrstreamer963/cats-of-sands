use crate::components::{Speed, Sprite};
use crate::{Fixed, TICKS_PER_SECOND};
use bevy_ecs::prelude::Query;

const SCENE_WIDTH: Fixed = Fixed::from_int(640);

pub(crate) fn move_sprites(mut sprites: Query<(&mut Sprite, &Speed)>) {
    for (mut sprite, speed) in &mut sprites {
        let displacement = speed.0.raw() / TICKS_PER_SECOND;
        let width = SCENE_WIDTH.raw();
        // Reduce the initial position before adding to avoid overflowing at the Q32.32 limits.
        let position = sprite.x.raw().rem_euclid(width) + displacement;
        sprite.x = Fixed::from_raw(position.rem_euclid(width));
    }
}
