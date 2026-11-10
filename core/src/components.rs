mod click_color;
mod position;
mod speed;
mod sprite;

pub(crate) use click_color::ClickColor;
pub use position::Position;
pub(crate) use speed::Speed;
pub use sprite::Sprite;

use bevy_ecs::prelude::*;
use serde::Serializer;
use serde::de::DeserializeOwned;
use serde_yaml_ng::Value;
use std::collections::HashMap;

pub(crate) type PreparedComponent = Box<dyn Fn(&mut EntityWorldMut<'_>)>;
type Loader = fn(Value, &str) -> Result<PreparedComponent, String>;
pub(crate) type Registry = HashMap<&'static str, Loader>;

pub(crate) fn registry() -> Registry {
    HashMap::from([
        ("Position", Position::load as Loader),
        ("Sprite", Sprite::load as Loader),
        ("Speed", Speed::load as Loader),
        ("ClickColor", ClickColor::load as Loader),
    ])
}

pub(crate) fn prepare(component: impl Component + Clone) -> PreparedComponent {
    Box::new(move |entity| {
        entity.insert(component.clone());
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

fn fixed(path: &str, value: f64) -> Result<crate::Fixed, String> {
    crate::Fixed::try_from_f64(value)
        .ok_or_else(|| format!("{path}: ожидается конечное число в диапазоне Q32.32"))
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

fn serialize_fixed<S: Serializer>(value: &crate::Fixed, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_f64(value.to_f64())
}
