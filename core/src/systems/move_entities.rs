use crate::components::{Position, Speed};
use crate::{Fixed, TICKS_PER_SECOND};
use bevy_ecs::prelude::Query;

const SCENE_WIDTH: Fixed = Fixed::from_int(640);

pub(crate) fn move_entities(mut entities: Query<(&mut Position, &Speed)>) {
    for (mut position, speed) in &mut entities {
        let displacement = speed.0.raw() / TICKS_PER_SECOND;
        let width = SCENE_WIDTH.raw();
        // Reduce the initial position before adding to avoid overflowing at the Q32.32 limits.
        let x = position.x.raw().rem_euclid(width) + displacement;
        position.x = Fixed::from_raw(x.rem_euclid(width));
    }
}
