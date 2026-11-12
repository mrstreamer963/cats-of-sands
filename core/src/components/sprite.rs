use super::{PreparedComponent, deserialize, fixed, prepare, serialize_fixed, validate_color};
use crate::Fixed;
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

/// Visual component of an entity.
#[derive(Clone, Component, Serialize, Deserialize)]
pub struct Sprite {
    #[serde(serialize_with = "serialize_fixed")]
    pub radius: Fixed,
    pub color: String,
}

impl Sprite {
    pub(super) fn load(value: Value, path: &str) -> Result<PreparedComponent, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            radius: f64,
            color: String,
        }

        let fields: Fields = deserialize(value, path)?;
        let radius = fixed(&format!("{path}.radius"), fields.radius)?;
        if radius.raw() <= 0 {
            return Err(format!("{path}.radius: ожидается число больше нуля"));
        }
        validate_color(&format!("{path}.color"), &fields.color)?;
        Ok(prepare(Self {
            radius,
            color: fields.color,
        }))
    }
}
