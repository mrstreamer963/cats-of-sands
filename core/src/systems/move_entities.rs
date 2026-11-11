use crate::components::{MoveTarget, Position, Speed, Velocity};
use crate::{Fixed, SCENE_WIDTH, TICKS_PER_SECOND};
use bevy_ecs::prelude::{Commands, Entity, Query};

pub(crate) fn move_entities(
    mut commands: Commands,
    mut entities: Query<(
        Entity,
        &mut Position,
        &Speed,
        Option<&MoveTarget>,
        &mut Velocity,
    )>,
) {
    for (entity, mut position, speed, target, mut velocity) in &mut entities {
        let Some(MoveTarget(target)) = target else {
            if velocity.x.raw() != 0 {
                position.x = position.x.add_wrapped(velocity.x, SCENE_WIDTH);
            }
            continue;
        };

        let (x, y) = Fixed::step_towards_2d(
            (position.x, position.y),
            (target.x, target.y),
            speed.0.div_int(TICKS_PER_SECOND),
        );
        velocity.x = Fixed::from_raw(x.raw() - position.x.raw());
        velocity.y = Fixed::from_raw(y.raw() - position.y.raw());
        position.x = Fixed::from_raw(position.x.raw() + velocity.x.raw());
        position.y = Fixed::from_raw(position.y.raw() + velocity.y.raw());

        if position.x == target.x && position.y == target.y {
            *velocity = Velocity::ZERO;
            commands.entity(entity).remove::<MoveTarget>();
        }
    }
}
