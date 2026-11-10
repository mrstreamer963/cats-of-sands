// Pick the earliest catalog entry whose dependencies have all been loaded.
export function orderAddons(selected) {
  const byId = new Map(selected.map(addon => [addon.id, addon]));
  if (byId.size !== selected.length) {
    throw new Error('Идентификаторы выбранных аддонов должны быть уникальными');
  }

  const dependencies = new Map();
  for (const addon of selected) {
    const depends = addon.depends === undefined ? [] : addon.depends;
    if (!Array.isArray(depends) || depends.some(id => typeof id !== 'string' || !id)) {
      throw new Error(`Аддон «${addon.id}»: depends должен быть списком ID`);
    }
    for (const id of depends) {
      if (!byId.has(id)) {
        throw new Error(`Аддон «${addon.id}» требует включить «${id}»`);
      }
    }
    dependencies.set(addon.id, depends);
  }

  const pending = [...selected];
  const loaded = new Set();
  const ordered = [];
  while (pending.length > 0) {
    const index = pending.findIndex(addon => dependencies.get(addon.id).every(id => loaded.has(id)));
    if (index === -1) {
      const blocked = pending.map(addon => addon.id).join(', ');
      throw new Error(`Цикл зависимостей аддонов блокирует загрузку: ${blocked}`);
    }
    const [addon] = pending.splice(index, 1);
    ordered.push(addon);
    loaded.add(addon.id);
  }
  return ordered;
}
