use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct Layer {
    pub name: String,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub entities: BTreeMap<String, EntityConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityConfig {
    pub position: Position,
    pub speed: f64,
    pub radius: f64,
    pub color: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

pub fn load(layers: &[Layer]) -> Result<Config, String> {
    let mut merged = Value::Mapping(Mapping::new());
    for layer in layers {
        let document: Value = serde_yaml_ng::from_str(&layer.text)
            .map_err(|error| format!("{}: {error}", layer.name))?;
        if !document.is_mapping() {
            return Err(format!("{}: ожидается объект YAML", layer.name));
        }
        merge(&mut merged, document);
    }

    let config: Config = serde_path_to_error::deserialize(merged)
        .map_err(|error| format!("{}: {}", error.path(), error.inner()))?;
    for (id, entity) in &config.entities {
        if id.trim().is_empty() {
            return Err("entities: ID сущности должен быть непустым".into());
        }
        for (field, value) in [
            ("position.x", entity.position.x),
            ("position.y", entity.position.y),
            ("speed", entity.speed),
            ("radius", entity.radius),
        ] {
            if !value.is_finite() {
                return Err(format!("entities.{id}.{field}: ожидается конечное число"));
            }
        }
        if entity.radius <= 0.0 {
            return Err(format!("entities.{id}.radius: ожидается число больше нуля"));
        }
        let hex_color = entity
            .color
            .strip_prefix('#')
            .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
        if !hex_color {
            return Err(format!("entities.{id}.color: ожидается цвет #RRGGBB"));
        }
    }
    Ok(config)
}

fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Mapping(base), Value::Mapping(overlay)) => {
            for (key, value) in overlay {
                if let Some(previous) = base.get_mut(&key) {
                    merge(previous, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn base() -> Layer {
        Layer {
            name: "base.yaml".into(),
            text: include_str!("../../public/config/base.yaml").into(),
        }
    }

    pub(crate) fn layer(name: &str, text: &str) -> Layer {
        Layer {
            name: name.into(),
            text: text.into(),
        }
    }

    #[test]
    fn shipped_addon_overrides_speed_and_keeps_position() {
        let addon = layer(
            "desert-companion.yaml",
            include_str!("../../public/config/addons/desert-companion.yaml"),
        );
        let config = load(&[base(), addon]).unwrap();
        assert_eq!(config.entities.len(), 2);
        let cat = &config.entities["cat"];
        assert_eq!(cat.position.x, 80.0);
        assert_eq!(cat.position.y, 150.0);
        assert_eq!(cat.speed, 150.0);
    }

    #[test]
    fn last_layer_wins_and_replaces_lists() {
        let mut merged = serde_yaml_ng::from_str("cat: {speed: 60, tags: [a, b]}").unwrap();
        for overlay in [
            "cat: {speed: 120, tags: [c]}",
            "cat: {speed: 150, tags: [d]}",
        ] {
            merge(&mut merged, serde_yaml_ng::from_str(overlay).unwrap());
        }
        let expected: Value = serde_yaml_ng::from_str("cat: {speed: 150, tags: [d]}").unwrap();
        assert_eq!(merged, expected);
    }

    #[test]
    fn parse_error_identifies_source() {
        let result = load(&[base(), layer("broken.yaml", "entities: [")]);
        assert!(result.err().unwrap().starts_with("broken.yaml:"));
    }

    #[test]
    fn validation_errors_identify_field() {
        for (overlay, path) in [
            ("entities: {cat: {speed: fast}}", "entities.cat.speed"),
            ("entities: {cat: {radius: -1}}", "entities.cat.radius"),
        ] {
            let result = load(&[base(), layer("invalid.yaml", overlay)]);
            assert!(result.err().unwrap().starts_with(path));
        }
    }
}
