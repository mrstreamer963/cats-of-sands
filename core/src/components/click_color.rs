use super::{PreparedComponent, deserialize, prepare, validate_color};
use bevy_ecs::prelude::Component;
use serde::Deserialize;
use serde_yaml_ng::Value;

#[derive(Clone, Component)]
pub(crate) struct ClickColor(pub String);

impl ClickColor {
    pub(super) fn load(value: Value, path: &str) -> Result<PreparedComponent, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            color: String,
        }

        let fields: Fields = deserialize(value, path)?;
        validate_color(&format!("{path}.color"), &fields.color)?;
        Ok(prepare(Self(fields.color)))
    }
}
