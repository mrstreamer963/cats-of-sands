import init, { Session } from '../core/pkg/sand_simulation.js';
import {
  addonCatalogPath, addonRoot, checkAddonId, orderAddons, resourceManifest,
  validateAddonCatalog, validateAddonManifest, validateBaseManifest,
} from './addons.js';
import './style.css';

const dataRoot = 'data/';
const storageKey = 'cats-of-sands.addons';
const form = document.querySelector('#launch');
const mapChoice = form.querySelector('#map');
const choices = document.querySelector('#addons');
const fields = form.querySelector('fieldset');
const launch = form.querySelector('button');
const errorMessage = document.querySelector('#error');
const canvas = document.querySelector('canvas');
const context = canvas.getContext('2d');

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
      option.textContent = path.slice('maps/'.length).replace(/\.ya?ml$/i, '');
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

async function loadLayer(name) {
  try {
    const path = name.split('/').map(encodeURIComponent).join('/');
    const response = await fetch(import.meta.env.BASE_URL + path);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    return { name, text: await response.text() };
  } catch (error) {
    throw new Error(`${name}: ${error.message}`);
  }
}

let session;
let frame;
let previousTime;
let renderTick;
let renderEntities = [];
let drawnEntities = [];
const selectedIds = new Set();

canvas.addEventListener('click', event => {
  const bounds = canvas.getBoundingClientRect();
  const x = (event.clientX - bounds.left) * canvas.width / bounds.width;
  const y = (event.clientY - bounds.top) * canvas.height / bounds.height;
  const target = drawnEntities.findLast(({ position, sprite }) => Math.hypot(x - position.x, y - position.y) <= sprite.radius);
  if (target) {
    if (event.shiftKey) {
      if (!selectedIds.delete(target.id)) selectedIds.add(target.id);
    } else {
      selectedIds.clear();
      selectedIds.add(target.id);
      session.click(target.id);
    }
  } else if (selectedIds.size > 0) {
    session.move_group_to(Uint32Array.from(selectedIds), x, y);
  }
});

function draw(time) {
  if (previousTime !== undefined) {
    session.advance((time - previousTime) / 1000);
    session.drain_events();
  }
  previousTime = time;

  const tick = session.current_tick();
  if (tick !== renderTick) {
    renderEntities = session.render_snapshot();
    renderTick = tick;
  }
  const alpha = session.interpolation_alpha();
  drawnEntities = renderEntities.map(({ id, position, previous_position: previous, sprite }) => ({
    id,
    sprite,
    position: {
      // A wrap across the scene edge should not sweep through the canvas.
      x: Math.abs(position.x - previous.x) > canvas.width / 2
        ? position.x
        : previous.x + (position.x - previous.x) * alpha,
      y: previous.y + (position.y - previous.y) * alpha,
    },
  }));

  context.clearRect(0, 0, canvas.width, canvas.height);
  for (const { id, position, sprite } of drawnEntities) {
    context.fillStyle = sprite.color;
    context.beginPath();
    context.arc(position.x, position.y, sprite.radius, 0, Math.PI * 2);
    context.fill();
    if (selectedIds.has(id)) {
      context.strokeStyle = '#ffffff';
      context.stroke();
    }
  }
  frame = requestAnimationFrame(draw);
}

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
    const next = new Session(layers);
    cancelAnimationFrame(frame);
    session?.free();
    session = next;
    drawnEntities = [];
    selectedIds.clear();
    renderTick = undefined;
    previousTime = undefined;
    canvas.hidden = false;
    frame = requestAnimationFrame(draw);
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
