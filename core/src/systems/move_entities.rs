use crate::components::{MoveTarget, Position, Speed, Velocity};
use crate::tile_map::Route;
use crate::{Fixed, SCENE_WIDTH, TICKS_PER_SECOND};
use bevy_ecs::prelude::{Commands, Entity, Query};

pub(crate) fn move_entities(
    mut commands: Commands,
    mut entities: Query<(
        Entity,
        &mut Position,
        &Speed,
        Option<&mut MoveTarget>,
        Option<&mut Route>,
        &mut Velocity,
    )>,
) {
    for (entity, mut position, speed, target, route, mut velocity) in &mut entities {
        let Some(mut target) = target else {
            if velocity.x.raw() != 0 {
                position.x = position.x.add_wrapped(velocity.x, SCENE_WIDTH);
            }
            continue;
        };

        let initial = (position.x, position.y);
        let mut remaining = speed.0.div_int(TICKS_PER_SECOND);
        let mut route = route;
        loop {
            let from = (position.x, position.y);
            let to = (target.0.x, target.0.y);
            (position.x, position.y) = Fixed::step_towards_2d(from, to, remaining);

            if (position.x, position.y) != to {
                velocity.x = Fixed::from_raw(position.x.raw() - initial.0.raw());
                velocity.y = Fixed::from_raw(position.y.raw() - initial.1.raw());
                break;
            }
            let Some(next) = route.as_mut().and_then(|route| route.0.pop_front()) else {
                *velocity = Velocity::ZERO;
                commands.entity(entity).remove::<(MoveTarget, Route)>();
                break;
            };
            target.0 = next;
            let distance = Fixed::distance_2d(from, to);
            remaining = Fixed::from_raw((i128::from(remaining.raw()) - distance) as i64);
        }
    }
}
