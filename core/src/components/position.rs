use super::{PreparedComponent, deserialize, fixed, prepare, serialize_fixed};
use crate::Fixed;
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

/// Location of an entity in the simulation.
#[derive(Clone, Component, Serialize)]
pub struct Position {
    #[serde(serialize_with = "serialize_fixed")]
    pub x: Fixed,
    #[serde(serialize_with = "serialize_fixed")]
    pub y: Fixed,
}

impl Position {
    pub(super) fn load(value: Value, path: &str) -> Result<PreparedComponent, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            x: f64,
            y: f64,
        }

        let fields: Fields = deserialize(value, path)?;
        Ok(prepare(Self {
            x: fixed(&format!("{path}.x"), fields.x)?,
            y: fixed(&format!("{path}.y"), fields.y)?,
        }))
    }
}
