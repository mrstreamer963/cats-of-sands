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
    position: Option<Position>,
    speed: Option<f64>,
    radius: Option<f64>,
    color: Option<String>,
    components: Option<Components>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
pub struct Components {
    pub sprite: Option<SpriteConfig>,
    pub speed: Option<SpeedConfig>,
    pub click_color: Option<ClickColorConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpriteConfig {
    pub position: Position,
    pub radius: f64,
    pub color: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeedConfig {
    pub value: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickColorConfig {
    pub color: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

pub fn load(layers: &[Layer]) -> Result<BTreeMap<String, Components>, String> {
    let mut merged = Value::Mapping(Mapping::new());
    for (index, layer) in layers.iter().enumerate() {
        let mut document: Value = serde_yaml_ng::from_str(&layer.text)
            .map_err(|error| format!("{}: {error}", layer.name))?;
        let Some(mapping) = document.as_mapping_mut() else {
            return Err(format!("{}: ожидается объект YAML", layer.name));
        };
        if index > 0 {
            mapping.remove("addon");
        }
        merge(&mut merged, document);
    }

    let config: Config = serde_path_to_error::deserialize(merged)
        .map_err(|error| format!("{}: {}", error.path(), error.inner()))?;
    config
        .entities
        .into_iter()
        .map(|(id, entity)| {
            let components = entity.into_components(&id)?;
            Ok((id, components))
        })
        .collect()
}

impl EntityConfig {
    fn into_components(self, id: &str) -> Result<Components, String> {
        if id.trim().is_empty() {
            return Err("entities: ID сущности должен быть непустым".into());
        }

        let path = format!("entities.{id}");
        let missing = |field| format!("{path}.{field}: обязательное поле");
        let legacy = self.components.is_none();
        let mut components = self.components.unwrap_or_default();
        let mut sprite_path = format!("{path}.components.Sprite");
        let mut speed_path = format!("{path}.components.Speed.value");

        if components.sprite.is_none()
            && (legacy || self.position.is_some() || self.radius.is_some() || self.color.is_some())
        {
            sprite_path = path.clone();
            components.sprite = Some(SpriteConfig {
                position: self.position.ok_or_else(|| missing("position"))?,
                radius: self.radius.ok_or_else(|| missing("radius"))?,
                color: self.color.ok_or_else(|| missing("color"))?,
            });
        }
        if components.speed.is_none() {
            if let Some(value) = self.speed {
                speed_path = format!("{path}.speed");
                components.speed = Some(SpeedConfig { value });
            } else if legacy {
                return Err(missing("speed"));
            }
        }
        if let Some(sprite) = &components.sprite {
            finite(&format!("{sprite_path}.position.x"), sprite.position.x)?;
            finite(&format!("{sprite_path}.position.y"), sprite.position.y)?;
            finite(&format!("{sprite_path}.radius"), sprite.radius)?;
            if sprite.radius <= 0.0 {
                return Err(format!("{sprite_path}.radius: ожидается число больше нуля"));
            }
            color(&format!("{sprite_path}.color"), &sprite.color)?;
        }
        if let Some(speed) = &components.speed {
            finite(&speed_path, speed.value)?;
        }
        if let Some(click_color) = &components.click_color {
            color(
                &format!("{path}.components.ClickColor.color"),
                &click_color.color,
            )?;
        }
        Ok(components)
    }
}

fn finite(path: &str, value: f64) -> Result<(), String> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("{path}: ожидается конечное число"))
    }
}

fn color(path: &str, value: &str) -> Result<(), String> {
    if value
        .strip_prefix('#')
        .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        Ok(())
    } else {
        Err(format!("{path}: ожидается цвет #RRGGBB"))
    }
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
        assert_eq!(config.len(), 2);
        let cat = &config["cat"];
        let sprite = cat.sprite.as_ref().unwrap();
        assert_eq!(sprite.position.x, 80.0);
        assert_eq!(sprite.position.y, 150.0);
        assert_eq!(cat.speed.as_ref().unwrap().value, 150.0);
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

    #[test]
    fn component_fields_merge_without_replacing_the_component() {
        let scene = layer(
            "scene.yaml",
            "entities: {cat: {components: {Sprite: {position: {x: 8, y: 15}, radius: 3, color: '#ffffff'}}}}",
        );
        let addon = layer(
            "color.yaml",
            "entities: {cat: {components: {Sprite: {color: '#ff0000'}}}}",
        );
        let entities = load(&[scene, addon]).unwrap();
        let cat = &entities["cat"];
        let sprite = cat.sprite.as_ref().unwrap();
        assert_eq!(
            (sprite.position.x, sprite.position.y, sprite.radius),
            (8.0, 15.0, 3.0)
        );
        assert_eq!(sprite.color, "#ff0000");
        assert!(cat.speed.is_none());
    }

    #[test]
    fn invalid_components_report_their_location() {
        for (component, expected) in [
            ("Unknown: {}", "Unknown"),
            ("ClickColor: {color: red}", "ClickColor.color"),
        ] {
            let addon = layer(
                "invalid.yaml",
                &format!("entities: {{cat: {{components: {{{component}}}}}}}"),
            );
            let error = load(&[base(), addon]).err().unwrap();
            assert!(
                error.starts_with("entities.cat.components") && error.contains(expected),
                "{error}"
            );
        }
    }
}
