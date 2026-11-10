use super::{PreparedComponent, deserialize, finite, prepare};
use bevy_ecs::prelude::Component;
use serde::Deserialize;
use serde_yaml_ng::Value;

#[derive(Component)]
pub(crate) struct Speed(pub f64);

impl Speed {
    pub(super) fn load(value: Value, path: &str) -> Result<PreparedComponent, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            value: f64,
        }

        let fields: Fields = deserialize(value, path)?;
        Self::validated(fields.value, &format!("{path}.value")).map(prepare)
    }

    pub(crate) fn validated(value: f64, path: &str) -> Result<Self, String> {
        finite(path, value)?;
        Ok(Self(value))
    }
}
