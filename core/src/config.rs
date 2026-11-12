use crate::components::{self, PreparedComponent, Registry};
use crate::tile_map::{Ground, Wall};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::BTreeMap;

#[derive(Clone, Deserialize, Serialize)]
pub struct Layer {
    pub name: String,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    entities: Option<BTreeMap<String, EntityConfig>>,
    units: Option<BTreeMap<String, EntityConfig>>,
    placements: Option<Vec<Placement>>,
    tiled: Option<String>,
    #[serde(default)]
    ground: BTreeMap<String, Ground>,
    #[serde(default)]
    walls: BTreeMap<String, Wall>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Placement {
    #[serde(rename = "type")]
    type_name: String,
    position: Value,
}

pub(crate) struct LoadedConfig {
    pub tiled: Option<String>,
    pub ground: BTreeMap<String, Ground>,
    pub walls: BTreeMap<String, Wall>,
    pub types: BTreeMap<String, Vec<PreparedComponent>>,
    pub placements: Vec<(String, Option<PreparedComponent>)>,
}

#[derive(Deserialize)]
#[serde(transparent)]
pub struct EntityConfig(BTreeMap<String, Option<Value>>);

pub fn load(layers: &[Layer]) -> Result<LoadedConfig, String> {
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
        for section in ["entities", "units"] {
            if let Some(value) = mapping.get_mut(section) {
                merge_type_section(&mut merged, section, value)
                    .map_err(|error| format!("{}: {error}", layer.name))?;
            }
        }
        if let Some(placements) = mapping.get_mut("placements") {
            let entries = placements
                .as_sequence_mut()
                .ok_or_else(|| format!("{}: placements: ожидается список", layer.name))?;
            let previous = merged
                .as_mapping_mut()
                .unwrap()
                .entry("placements".into())
                .or_insert_with(|| Value::Sequence(Vec::new()));
            previous.as_sequence_mut().unwrap().append(entries);
            *placements = std::mem::take(previous);
        }
        merge(&mut merged, document);
    }

    let config: Config = serde_path_to_error::deserialize(merged)
        .map_err(|error| format!("{}: {}", error.path(), error.inner()))?;
    for (name, ground) in &config.ground {
        if ground.cost == 0 {
            return Err(format!("ground.{name}.cost: ожидается положительное число"));
        }
    }
    for (name, wall) in &config.walls {
        if wall.durability == 0 {
            return Err(format!(
                "walls.{name}.durability: ожидается положительное число"
            ));
        }
    }
    let (section, definitions) = match (config.entities, config.units) {
        (Some(entities), None) if config.placements.is_none() => ("entities", entities),
        (None, Some(units)) => ("units", units),
        (None, None) if config.placements.is_some() => ("units", BTreeMap::new()),
        (None, None) => return Err("Ожидается каталог units или описание entities".into()),
        _ => return Err("Используйте entities отдельно от units и placements".into()),
    };
    let registry = components::registry();
    let types = definitions
        .into_iter()
        .map(|(id, entity)| {
            let components = entity.into_components(section, &id, &registry)?;
            Ok((id, components))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let placements = if section == "entities" {
        types.keys().map(|name| (name.clone(), None)).collect()
    } else {
        config
            .placements
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, placement)| {
                let name = placement.type_name;
                if !types.contains_key(&name) {
                    return Err(format!("placements[{index}].type: неизвестный тип {name}"));
                }
                let path = format!("placements[{index}].position");
                let position = registry["Position"](placement.position, &path)?;
                Ok((name, Some(position)))
            })
            .collect::<Result<Vec<_>, String>>()?
    };
    Ok(LoadedConfig {
        types,
        placements,
        tiled: config.tiled,
        ground: config.ground,
        walls: config.walls,
    })
}

fn merge_type_section(merged: &mut Value, section: &str, value: &mut Value) -> Result<(), String> {
    match value {
        Value::Sequence(entries) => {
            let mut rules = merged
                .as_mapping_mut()
                .unwrap()
                .remove(section)
                .unwrap_or_else(|| Value::Mapping(Mapping::new()));
            let rules_map = rules
                .as_mapping_mut()
                .ok_or_else(|| format!("{section}: ожидается словарь типов"))?;
            apply_entity_rules(section, rules_map, std::mem::take(entries))?;
            *value = rules;
        }
        Value::Mapping(entities) => {
            for (_, entity) in entities {
                if let Value::Mapping(entity) = entity {
                    normalize_speed(entity);
                }
            }
        }
        _ => return Err(format!("{section}: ожидается словарь или список типов")),
    }
    Ok(())
}

fn apply_entity_rules(
    section: &str,
    entities: &mut Mapping,
    entries: Vec<Value>,
) -> Result<(), String> {
    for (index, entry) in entries.into_iter().enumerate() {
        let path = format!("{section}[{index}]");
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
        normalize_speed(&mut entry);
        let value = Value::Mapping(entry);
        if let Some(previous) = entities.get_mut(&id) {
            merge(previous, value);
        } else {
            entities.insert(id, value);
        }
    }
    Ok(())
}

fn normalize_speed(entity: &mut Mapping) {
    if let Some(speed @ Value::Number(_)) = entity.get_mut("Speed") {
        let mut fields = Mapping::new();
        fields.insert(Value::String("value".into()), std::mem::take(speed));
        *speed = Value::Mapping(fields);
    }
}

impl EntityConfig {
    fn into_components(
        self,
        section: &str,
        id: &str,
        registry: &Registry,
    ) -> Result<Vec<PreparedComponent>, String> {
        if id.trim().is_empty() {
            return Err(format!("{section}: ID сущности должен быть непустым"));
        }

        let path = format!("{section}.{id}");
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

    fn base() -> Layer {
        layer(
            "legacy.yaml",
            "entities: {cat: {Position: {x: 80, y: 150}, Sprite: {radius: 18, color: '#f9c86d'}, Speed: 60}}",
        )
    }

    pub(crate) fn shipped_scene() -> [Layer; 2] {
        [
            layer("units.yaml", include_str!("../../public/data/units.yaml")),
            layer(
                "maps/map1.yaml",
                include_str!("../../public/data/maps/map1.yaml"),
            ),
        ]
    }

    pub(crate) fn shipped_addon() -> Layer {
        layer(
            "desert-companion/units.yaml",
            include_str!("../../public/addons/desert-companion/units.yaml"),
        )
    }

    pub(crate) fn layer(name: &str, text: &str) -> Layer {
        Layer {
            name: name.into(),
            text: text.into(),
        }
    }

    #[test]
    fn shipped_addon_overrides_speed_and_keeps_position() {
        let [units, scene] = shipped_scene();
        let config = load(&[units, scene, shipped_addon()]).unwrap();
        assert_eq!(config.types.len(), 2);
        let session = crate::Simulation::from_config(config, None).unwrap();
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
    fn placements_create_independent_instances_and_leave_unused_types_available() {
        let [units, scene] = shipped_scene();
        let extra = layer(
            "extra.yaml",
            "units: [{type: UNUSED}]\nplacements: [{type: CAT, position: {x: 240, y: 180}}]",
        );
        let mut simulation = crate::Simulation::from_layers(&[units, scene, extra]).unwrap();
        let snapshot = simulation.snapshot();
        assert_eq!(snapshot.len(), 2);
        for (entry, (id, x, y)) in snapshot.iter().zip([(1, 80, 150), (2, 240, 180)]) {
            assert_eq!(entry.id, SimId(id));
            assert_eq!(entry.type_name, "CAT");
            let position = entry.position.as_ref().unwrap();
            assert_eq!(position.x, Fixed::from_int(x));
            assert_eq!(position.y, Fixed::from_int(y));
            assert_eq!(entry.sprite.as_ref().unwrap().color, "#f9c86d");
        }
        assert_eq!(simulation.spawn("UNUSED").unwrap(), SimId(3));
    }

    #[test]
    fn placement_with_unknown_type_reports_its_index() {
        let [units, scene] = shipped_scene();
        let extra = layer(
            "extra.yaml",
            "placements: [{type: cat, position: {x: 0, y: 0}}]",
        );
        let error = load(&[units, scene, extra]).err().unwrap();
        assert_eq!(error, "placements[1].type: неизвестный тип cat");
    }

    #[test]
    fn scalar_and_mapping_speeds_merge_across_layers() {
        let base = layer("base.yaml", "entities: {cat: {Speed: {value: 60}}}");
        let scalar = layer("scalar.yaml", "entities: [{override: cat, Speed: -22.5}]");
        let empty = layer("empty.yaml", "entities: {cat: {Speed: {}}}");
        let mapping = layer("mapping.yaml", "entities: {cat: {Speed: {value: 90}}}");
        let layers = [base, scalar, empty, mapping];

        for (count, expected) in [(3, -22.5), (4, 90.0)] {
            let simulation = crate::Simulation::from_layers(&layers[..count]).unwrap();
            let cat = simulation.entities[&SimId(1)].0;
            assert_eq!(
                simulation.world.get::<Speed>(cat).unwrap().0.to_f64(),
                expected
            );
        }
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
        let session = crate::Simulation::from_config(entities, None).unwrap();
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
        assert!(config.types["cat"].is_empty());
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
        assert_eq!(config.types.len(), 1);
        let session = crate::Simulation::from_config(config, None).unwrap();
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
