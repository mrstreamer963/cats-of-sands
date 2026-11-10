export const addonCatalogPath = 'config/addons.json';
export const addonRoot = 'config/addons';
export const resourceManifest = 'resource.json';

function isPathSegment(segment) {
  return segment.trim().length > 0 && segment !== '.' && segment !== '..'
    && !/[\\/:?%#\u0000-\u001f\u007f]/u.test(segment);
}

export function validateAddonCatalog(folders) {
  if (!Array.isArray(folders) || folders.some(folder => typeof folder !== 'string' || !isPathSegment(folder))) {
    throw new Error('ожидается массив имён папок аддонов');
  }
  if (new Set(folders).size !== folders.length) {
    throw new Error('имена папок аддонов должны быть уникальными');
  }
  return folders;
}

export function validateResourceManifest(manifest) {
  if (!manifest || typeof manifest !== 'object' || Array.isArray(manifest)) {
    throw new Error('ожидается объект манифеста');
  }
  const { layers } = manifest;
  if (!Array.isArray(layers) || layers.length === 0 || layers.some(path =>
    typeof path !== 'string' || !path.split('/').every(isPathSegment) || !/\.ya?ml$/i.test(path))) {
    throw new Error('поле layers должно содержать список относительных путей к YAML');
  }
  if (new Set(layers).size !== layers.length) {
    throw new Error('пути в layers должны быть уникальными');
  }
  return manifest;
}

export function validateAddonManifest(manifest, { requireHash = true } = {}) {
  validateResourceManifest(manifest);
  for (const field of ['id', 'name']) {
    if (typeof manifest[field] !== 'string' || !manifest[field].trim()) {
      throw new Error(`поле ${field} должно быть непустой строкой`);
    }
  }
  const { depends, sha256 } = manifest;
  if (depends !== undefined && (!Array.isArray(depends)
      || depends.some(id => typeof id !== 'string' || !id.trim()))) {
    throw new Error('поле depends должно быть массивом непустых ID');
  }
  if (requireHash && (typeof sha256 !== 'string' || sha256.length !== 64 || !/^[0-9a-f]+$/.test(sha256))) {
    throw new Error('поле sha256 должно содержать 64 строчных шестнадцатеричных символа');
  }
  return manifest;
}

export function checkAddonId(ids, id) {
  if (ids.has(id)) {
    throw new Error(`повторяющийся ID аддона «${id}»`);
  }
  ids.add(id);
}

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
