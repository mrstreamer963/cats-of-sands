use crate::Fixed;
use bevy_ecs::prelude::Component;

/// Displacement per simulation tick.
#[derive(Component)]
pub(crate) struct Velocity {
    pub(crate) x: Fixed,
    pub(crate) y: Fixed,
}

impl Velocity {
    pub(crate) const ZERO: Self = Self {
        x: Fixed::from_int(0),
        y: Fixed::from_int(0),
    };
}
