use crate::components::{self, PreparedComponent, Registry};
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
#[serde(transparent)]
pub struct EntityConfig(BTreeMap<String, Option<Value>>);

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
        if let Some(entities @ Value::Sequence(_)) = mapping.get_mut("entities") {
            let Value::Sequence(entries) = std::mem::take(entities) else {
                unreachable!();
            };
            let mut rules = merged
                .as_mapping_mut()
                .unwrap()
                .remove("entities")
                .unwrap_or_else(|| Value::Mapping(Mapping::new()));
            let rules_map = rules
                .as_mapping_mut()
                .ok_or_else(|| format!("{}: entities: ожидается словарь сущностей", layer.name))?;
            apply_entity_rules(rules_map, entries)
                .map_err(|error| format!("{}: {error}", layer.name))?;
            *entities = rules;
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
        let value = Value::Mapping(entry);
        if let Some(previous) = entities.get_mut(&id) {
            merge(previous, value);
        } else {
            entities.insert(id, value);
        }
    }
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
        let mut prepared = Vec::new();
        for (name, value) in self.0 {
            if name == "components" {
                return Err(format!(
                    "{path}.components: перенесите компоненты прямо в описание типа"
                ));
            }
            let path = format!("{path}.{name}");
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
    use crate::components::{Position, Speed, Sprite};
    use crate::{Fixed, SimId};

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
        let session = crate::Simulation::from_config(config);
        let cat = session.entities[&SimId(1)].0;
        let position = session.world.get::<Position>(cat).unwrap();
        assert_eq!(position.x, Fixed::from_int(80));
        assert_eq!(position.y, Fixed::from_int(150));
        assert_eq!(
            session.world.get::<Speed>(cat).unwrap().0,
            Fixed::from_int(150)
        );
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
            (
                "entities: {cat: {Speed: {value: fast}}}",
                "entities.cat.Speed.value",
            ),
            (
                "entities: {cat: {Sprite: {radius: -1}}}",
                "entities.cat.Sprite.radius",
            ),
        ] {
            let result = load(&[base(), layer("invalid.yaml", overlay)]);
            assert!(result.err().unwrap().starts_with(path));
        }
    }

    #[test]
    fn component_fields_merge_without_replacing_the_component() {
        let scene = layer(
            "scene.yaml",
            "entities: {cat: {Position: {x: 8, y: 15}, Sprite: {radius: 3, color: '#ffffff'}}}",
        );
        let addon = layer(
            "color.yaml",
            "entities: [{override: cat, Sprite: {color: '#ff0000'}}]",
        );
        let entities = load(&[scene, addon]).unwrap();
        let session = crate::Simulation::from_config(entities);
        let cat = session.entities[&SimId(1)].0;
        let sprite = session.world.get::<Sprite>(cat).unwrap();
        let position = session.world.get::<Position>(cat).unwrap();
        assert_eq!(
            (position.x, position.y, sprite.radius),
            (Fixed::from_int(8), Fixed::from_int(15), Fixed::from_int(3))
        );
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
                &format!("entities: {{cat: {{{component}}}}}"),
            );
            let error = load(&[base(), addon]).err().unwrap();
            assert!(
                error.starts_with("entities.cat.") && error.contains(expected),
                "{error}"
            );
        }
    }

    #[test]
    fn implicit_component_fields_are_rejected() {
        for field in ["position", "radius", "color", "speed"] {
            let addon = layer(
                "implicit.yaml",
                &format!("entities: {{cat: {{{field}: null}}}}"),
            );
            let error = load(&[base(), addon]).err().unwrap();
            assert!(
                error.contains("entities.cat") && error.contains(field),
                "{error}"
            );
        }
    }

    #[test]
    fn rejects_the_components_wrapper_and_accepts_an_empty_type() {
        for text in [
            "entities: {cat: {components: {Speed: {value: 60}}}}",
            "entities: [{override: cat, components: [{type: Speed, value: 60}]}]",
        ] {
            let error = load(&[base(), layer("wrapped.yaml", text)]).err().unwrap();
            assert!(
                error.starts_with("entities.cat.components: перенесите"),
                "{error}"
            );
        }
        let config = load(&[layer("empty.yaml", "entities: {cat: {}}")]).unwrap();
        assert!(config["cat"].is_empty());
    }

    #[test]
    fn entity_rules_apply_in_order() {
        let addon = layer(
            "rules.yaml",
            "entities:
  - update: absent
    Speed:
      value: 100
  - delete: cat
  - id: cat
    Speed:
      value: 120",
        );
        let config = load(&[base(), addon]).unwrap();
        assert_eq!(config.len(), 1);
        let session = crate::Simulation::from_config(config);
        let cat = session.entities[&SimId(1)].0;
        let snapshot = session.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].id, SimId(1));
        assert_eq!(snapshot[0].type_name, "cat");
        assert!(snapshot[0].sprite.is_none());
        assert!(snapshot[0].position.is_none());
        assert_eq!(
            session.world.get::<Speed>(cat).unwrap().0,
            Fixed::from_int(120)
        );
    }

    #[test]
    fn conflicting_rules_report_the_source_and_entry() {
        for (rule, expected) in [
            ("new: cat", "уже существует"),
            ("override: absent", "не найдена"),
            ("id: cat, override: cat", "укажите один"),
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
