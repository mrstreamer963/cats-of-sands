use crate::components::{self, Position, PreparedComponent, Registry, Speed, Sprite};
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
    components: Option<BTreeMap<String, Option<Value>>>,
}

pub fn load(layers: &[Layer]) -> Result<BTreeMap<String, Vec<PreparedComponent>>, String> {
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
    let registry = components::registry();
    config
        .entities
        .into_iter()
        .map(|(id, entity)| {
            let components = entity.into_components(&id, &registry)?;
            Ok((id, components))
        })
        .collect()
}

impl EntityConfig {
    fn into_components(
        self,
        id: &str,
        registry: &Registry,
    ) -> Result<Vec<PreparedComponent>, String> {
        if id.trim().is_empty() {
            return Err("entities: ID сущности должен быть непустым".into());
        }

        let path = format!("entities.{id}");
        let missing = |field| format!("{path}.{field}: обязательное поле");
        let legacy = self.components.is_none();
        let explicit = self.components.unwrap_or_default();
        let has = |name| explicit.get(name).is_some_and(Option::is_some);
        let mut prepared = Vec::new();

        if !has("Sprite")
            && (legacy || self.position.is_some() || self.radius.is_some() || self.color.is_some())
        {
            let sprite = Sprite::validated(
                self.position.ok_or_else(|| missing("position"))?,
                self.radius.ok_or_else(|| missing("radius"))?,
                self.color.ok_or_else(|| missing("color"))?,
                &path,
            )?;
            prepared.push(components::prepare(sprite));
        }
        if !has("Speed") {
            if let Some(value) = self.speed {
                let speed = Speed::validated(value, &format!("{path}.speed"))?;
                prepared.push(components::prepare(speed));
            } else if legacy {
                return Err(missing("speed"));
            }
        }

        for (name, value) in explicit {
            let path = format!("{path}.components.{name}");
            let loader = registry
                .get(name.as_str())
                .ok_or_else(|| format!("{path}: неизвестный компонент"))?;
            if let Some(value) = value {
                prepared.push(loader(value, &path)?);
            }
        }
        Ok(prepared)
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
        let session = crate::Session::from_config(config);
        let cat = session.entities["cat"];
        let sprite = session.world.get::<Sprite>(cat).unwrap();
        assert_eq!(sprite.x, 80.0);
        assert_eq!(sprite.y, 150.0);
        assert_eq!(session.world.get::<Speed>(cat).unwrap().0, 150.0);
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
        let session = crate::Session::from_config(entities);
        let cat = session.entities["cat"];
        let sprite = session.world.get::<Sprite>(cat).unwrap();
        assert_eq!((sprite.x, sprite.y, sprite.radius), (8.0, 15.0, 3.0));
        assert_eq!(sprite.color, "#ff0000");
        assert!(session.world.get::<Speed>(cat).is_none());
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

    #[test]
    fn registered_speed_overrides_legacy_speed() {
        let addon = layer(
            "speed.yaml",
            "entities: {cat: {speed: 90, components: {Speed: {value: 120}}}}",
        );
        let session = crate::Session::from_config(load(&[base(), addon]).unwrap());
        let cat = session.entities["cat"];
        assert_eq!(session.world.get::<Speed>(cat).unwrap().0, 120.0);
    }
}
