import init, { Session, tiled_reference } from '../core/pkg/sand_simulation.js';
import {
  addonCatalogPath, addonRoot, checkAddonId, orderAddons, resourceManifest,
  validateAddonCatalog, validateAddonManifest, validateBaseManifest,
} from './addons.js';
import { loadTiled, relativeResource } from './tiled.js';
import { loadLayer, loadImage } from './resources.js';
import { setupMapPreview } from './map-preview.js';

async function loadMap(layer) {
  let reference;
  try {
    reference = tiled_reference(layer.text);
    if (reference) reference = relativeResource(layer.name, reference);
  } catch (error) {
    throw new Error(`${layer.name}: ${error.message}`);
  }
  return reference ? loadTiled(reference, loadLayer, loadImage) : undefined;
}

export function setupLaunch(installSession) {
  const storageKey = 'cats-of-sands.addons';
  const dataRoot = 'data/';
  const form = document.querySelector('#launch');
  const mapChoice = form.querySelector('#map');
  const choices = document.querySelector('#addons');
  const fields = form.querySelector('fieldset');
  const launch = form.querySelector('button');
  const errorMessage = document.querySelector('#error');
  const updatePreview = setupMapPreview(mapChoice, async path => {
    await init();
    return loadMap(await loadLayer(dataRoot + path));
  });

  function restoreSelection() {
    try {
      const ids = JSON.parse(localStorage.getItem(storageKey));
      return new Set(Array.isArray(ids) ? ids : []);
    } catch {
      return new Set();
    }
  }

  let addonChoices = [];
  let base;

  async function loadJson(name, validate) {
    const { text } = await loadLayer(name);
    try {
      return validate(JSON.parse(text));
    } catch (error) {
      throw new Error(`${name}: ${error.message}`);
    }
  }

  async function loadAddons() {
    const folders = await loadJson(addonCatalogPath, validateAddonCatalog);
    const addons = await Promise.all(folders.map(async folder => {
      const root = `${addonRoot}/${folder}`;
      const manifestPath = root + '/' + resourceManifest;
      const manifest = await loadJson(manifestPath, validateAddonManifest);
      return { ...manifest, layers: manifest.layers.map(path => root + '/' + path), manifestPath };
    }));
    const ids = new Set();
    for (const addon of addons) {
      try {
        checkAddonId(ids, addon.id);
      } catch (error) {
        throw new Error(`${addon.manifestPath}: ${error.message}`);
      }
    }
    return addons;
  }

  async function showLaunchOptions() {
    try {
      const [manifest, addons] = await Promise.all([
        loadJson(dataRoot + resourceManifest, validateBaseManifest),
        loadAddons(),
      ]);
      base = manifest;
      for (const path of base.maps) {
        const option = document.createElement('option');
        option.value = path;
        const name = path.slice('maps/'.length).replace(/\.ya?ml$/i, '');
        option.textContent = name === 'desert' ? 'Пустыня' : name;
        mapChoice.append(option);
      }
      const selected = restoreSelection();
      addonChoices = addons.map(addon => {
        const label = document.createElement('label');
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = selected.has(addon.id);
        label.append(checkbox, addon.name);
        choices.append(label);
        return { addon, checkbox };
      });
      mapChoice.disabled = false;
      launch.disabled = false;
      updatePreview();
    } catch (error) {
      errorMessage.textContent = error.message;
      errorMessage.hidden = false;
    }
  }

  function selectedAddons() {
    return addonChoices.filter(({ checkbox }) => checkbox.checked).map(({ addon }) => addon);
  }

  choices.addEventListener('change', () => {
    try {
      localStorage.setItem(storageKey, JSON.stringify(selectedAddons().map(addon => addon.id)));
    } catch {
      // The checkboxes retain the selection while the page is open.
    }
  });

  mapChoice.addEventListener('change', updatePreview);

  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (launch.disabled) return;

    fields.disabled = true;
    mapChoice.disabled = true;
    launch.disabled = true;
    errorMessage.hidden = true;
    try {
      const addons = orderAddons(selectedAddons());
      const files = [
        ...base.layers.map(path => dataRoot + path),
        dataRoot + mapChoice.value,
        ...addons.flatMap(addon => addon.layers),
      ];
      await init();
      const layers = await Promise.all(files.map(loadLayer));
      const selectedLayer = layers[base.layers.length];
      const nextMap = await loadMap(selectedLayer);
      let next;
      try {
        next = new Session(layers, nextMap?.grid);
      } catch (error) {
        throw new Error(`${selectedLayer.name}: ${error.message}`);
      }
      installSession(next, nextMap);
    } catch (error) {
      errorMessage.textContent = error.message;
      errorMessage.hidden = false;
    } finally {
      fields.disabled = false;
      mapChoice.disabled = false;
      launch.disabled = false;
    }
  });

  showLaunchOptions();
}
