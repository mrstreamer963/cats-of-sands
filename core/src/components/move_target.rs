use super::Position;
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

/// Destination of the current movement order.
#[derive(Clone, Component, Deserialize, Serialize)]
pub(crate) struct MoveTarget(pub(crate) Position);
