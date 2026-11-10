mod click_color;
mod speed;
mod sprite;

pub(crate) use click_color::ClickColor;
pub(crate) use speed::Speed;
pub(crate) use sprite::{Position, Sprite};

use bevy_ecs::prelude::*;
use serde::de::DeserializeOwned;
use serde_yaml_ng::Value;
use std::collections::HashMap;

pub(crate) type PreparedComponent = Box<dyn FnOnce(&mut EntityWorldMut<'_>)>;
type Loader = fn(Value, &str) -> Result<PreparedComponent, String>;
pub(crate) type Registry = HashMap<&'static str, Loader>;

pub(crate) fn registry() -> Registry {
    HashMap::from([
        ("Sprite", Sprite::load as Loader),
        ("Speed", Speed::load as Loader),
        ("ClickColor", ClickColor::load as Loader),
    ])
}

pub(crate) fn prepare(component: impl Component) -> PreparedComponent {
    Box::new(move |entity| {
        entity.insert(component);
    })
}

fn deserialize<T: DeserializeOwned>(value: Value, path: &str) -> Result<T, String> {
    serde_path_to_error::deserialize(value).map_err(|error| {
        if error.path().iter().next().is_none() {
            format!("{path}: {}", error.inner())
        } else {
            format!("{path}.{}: {}", error.path(), error.inner())
        }
    })
}

fn finite(path: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("{path}: ожидается конечное число"))
    }
}

fn validate_color(path: &str, value: &str) -> Result<(), String> {
    if value
        .strip_prefix('#')
        .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        Ok(())
    } else {
        Err(format!("{path}: ожидается цвет #RRGGBB"))
    }
}
