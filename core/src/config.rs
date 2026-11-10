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
        if let Some(entities) = mapping.get_mut("entities") {
            if let Value::Sequence(entries) = entities {
                let mut rules = merged
                    .as_mapping_mut()
                    .unwrap()
                    .remove("entities")
                    .unwrap_or_else(|| Value::Mapping(Mapping::new()));
                let rules_map = rules.as_mapping_mut().ok_or_else(|| {
                    format!("{}: entities: ожидается словарь сущностей", layer.name)
                })?;
                apply_entity_rules(rules_map, std::mem::take(entries))
                    .map_err(|error| format!("{}: {error}", layer.name))?;
                *entities = rules;
            } else if let Some(entities) = entities.as_mapping_mut() {
                for (id, entity) in entities {
                    if let (Some(id), Some(entity)) = (id.as_str(), entity.as_mapping_mut()) {
                        normalize_components(entity, &format!("entities.{id}"))
                            .map_err(|error| format!("{}: {error}", layer.name))?;
                    }
                }
            }
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

fn apply_entity_rules(entities: &mut Mapping, entries: Vec<Value>) -> Result<(), String> {
    for (index, entry) in entries.into_iter().enumerate() {
        let path = format!("entities[{index}]");
        let Value::Mapping(mut entry) = entry else {
            return Err(format!("{path}: ожидается объект"));
        };
        let mut keys = ["id", "type", "new", "override", "update", "delete"]
            .into_iter()
            .filter(|key| entry.contains_key(*key));
        let operation = keys
            .next()
            .filter(|_| keys.next().is_none())
            .ok_or_else(|| {
                format!("{path}: укажите один из id, type, new, override, update, delete")
            })?;
        let id = entry.remove(operation).unwrap();
        let name = id
            .as_str()
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| format!("{path}.{operation}: ожидается непустой строковый ID"))?;
        let exists = entities.contains_key(&id);
        match (operation, exists) {
            ("new", true) => return Err(format!("{path}: сущность {name} уже существует")),
            ("override", false) => return Err(format!("{path}: сущность {name} не найдена")),
            ("delete", _) => {
                if !entry.is_empty() {
                    return Err(format!("{path}: delete принимает только ID сущности"));
                }
                entities.remove(&id);
                continue;
            }
            ("update", false) => continue,
            _ => {}
        }
        normalize_components(&mut entry, &format!("entities.{name}"))?;
        let value = Value::Mapping(entry);
        if let Some(previous) = entities.get_mut(&id) {
            merge(previous, value);
        } else {
            entities.insert(id, value);
        }
    }
    Ok(())
}

fn normalize_components(entity: &mut Mapping, path: &str) -> Result<(), String> {
    let Some(components @ Value::Sequence(_)) = entity.get_mut("components") else {
        return Ok(());
    };
    let Value::Sequence(entries) = std::mem::take(components) else {
        unreachable!();
    };
    let mut by_type = Mapping::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let path = format!("{path}.components[{index}]");
        let Value::Mapping(mut entry) = entry else {
            return Err(format!("{path}: ожидается объект"));
        };
        let name = entry.remove("type").unwrap_or(Value::Null);
        let kind = name
            .as_str()
            .filter(|kind| !kind.trim().is_empty())
            .ok_or_else(|| format!("{path}.type: ожидается непустое имя компонента"))?;
        if by_type.contains_key(&name) {
            return Err(format!("{path}.type: компонент {kind} указан повторно"));
        }
        by_type.insert(name, Value::Mapping(entry));
    }
    *components = Value::Mapping(by_type);
    Ok(())
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
            "entities: [{override: cat, components: [{type: Sprite, color: '#ff0000'}]}]",
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

    #[test]
    fn entity_rules_apply_in_order() {
        let addon = layer(
            "rules.yaml",
            "entities:
  - update: absent
    speed: 100
  - delete: cat
  - id: cat
    components:
      - type: Speed
        value: 120",
        );
        let config = load(&[base(), addon]).unwrap();
        assert_eq!(config.len(), 1);
        let session = crate::Session::from_config(config);
        let cat = session.entities["cat"];
        assert!(session.world.get::<Sprite>(cat).is_none());
        assert_eq!(session.world.get::<Speed>(cat).unwrap().0, 120.0);
    }

    #[test]
    fn conflicting_rules_report_the_source_and_entry() {
        for (rule, expected) in [
            ("new: cat", "уже существует"),
            ("override: absent", "не найдена"),
            (
                "id: cat, components: [{type: Speed}, {type: Speed}]",
                "указан повторно",
            ),
        ] {
            let addon = layer("rules.yaml", &format!("entities: [{{{rule}}}]"));
            let error = load(&[base(), addon]).err().unwrap();
            assert!(
                error.starts_with("rules.yaml: entities") && error.contains(expected),
                "{error}"
            );
        }
    }
}
