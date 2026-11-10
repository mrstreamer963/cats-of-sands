use super::{PreparedComponent, deserialize, finite, prepare, validate_color};
use bevy_ecs::prelude::Component;
use serde::Deserialize;
use serde_yaml_ng::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Position {
    x: f64,
    y: f64,
}

#[derive(Component)]
pub(crate) struct Sprite {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
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
        finite(&format!("{path}.position.x"), position.x)?;
        finite(&format!("{path}.position.y"), position.y)?;
        finite(&format!("{path}.radius"), radius)?;
        if radius <= 0.0 {
            return Err(format!("{path}.radius: ожидается число больше нуля"));
        }
        validate_color(&format!("{path}.color"), &color)?;
        Ok(Self {
            x: position.x,
            y: position.y,
            radius,
            color,
        })
    }
}
