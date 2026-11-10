use super::{PreparedComponent, deserialize, fixed, prepare, validate_color};
use crate::Fixed;
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize, Serializer};
use serde_yaml_ng::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Position {
    x: f64,
    y: f64,
}

/// Visual component of an entity.
#[derive(Clone, Component, Serialize)]
pub struct Sprite {
    #[serde(serialize_with = "serialize_fixed")]
    pub x: Fixed,
    #[serde(serialize_with = "serialize_fixed")]
    pub y: Fixed,
    #[serde(serialize_with = "serialize_fixed")]
    pub radius: Fixed,
    pub color: String,
}

impl Sprite {
    pub(super) fn load(value: Value, path: &str) -> Result<PreparedComponent, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            position: Position,
            radius: f64,
            color: String,
        }

        let fields: Fields = deserialize(value, path)?;
        Self::validated(fields.position, fields.radius, fields.color, path).map(prepare)
    }

    pub(crate) fn validated(
        position: Position,
        radius: f64,
        color: String,
        path: &str,
    ) -> Result<Self, String> {
        let x = fixed(&format!("{path}.position.x"), position.x)?;
        let y = fixed(&format!("{path}.position.y"), position.y)?;
        let radius = fixed(&format!("{path}.radius"), radius)?;
        if radius.raw() <= 0 {
            return Err(format!("{path}.radius: ожидается число больше нуля"));
        }
        validate_color(&format!("{path}.color"), &color)?;
        Ok(Self {
            x,
            y,
            radius,
            color,
        })
    }
}

fn serialize_fixed<S: Serializer>(value: &Fixed, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_f64(value.to_f64())
}
