use super::{PreparedComponent, deserialize, fixed, prepare};
use crate::Fixed;
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

#[derive(Clone, Component, Serialize, Deserialize)]
pub(crate) struct Speed(pub Fixed);

impl Speed {
    pub(super) fn load(value: Value, path: &str) -> Result<PreparedComponent, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            value: f64,
        }

        let fields: Fields = deserialize(value, path)?;
        fixed(&format!("{path}.value"), fields.value).map(|value| prepare(Self(value)))
    }
}
