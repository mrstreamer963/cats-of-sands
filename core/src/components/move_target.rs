use super::Position;
use bevy_ecs::prelude::Component;

/// Destination of the current movement order.
#[derive(Component)]
pub(crate) struct MoveTarget(pub(crate) Position);
